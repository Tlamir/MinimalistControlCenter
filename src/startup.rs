//! Explicit logon startup. Elevated startup is installed only in protected Program Files.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub enabled: bool,
    pub managed: bool,
}

/// Finite boot recovery, independent of dashboard polling or window visibility.
#[derive(Default)]
pub struct Recovery {
    failures: u8,
    next_ms: Option<u64>,
}
pub enum RecoveryOutcome {
    Recovered,
    RetryAfter(u32),
    Exhausted,
}
impl Recovery {
    pub fn completed(&mut self, now_ms: u64, verified: bool) -> RecoveryOutcome {
        self.next_ms = None;
        if verified {
            return RecoveryOutcome::Recovered;
        }
        self.failures += 1;
        if self.failures >= 3 {
            return RecoveryOutcome::Exhausted;
        }
        let delay = if self.failures == 1 { 10_000 } else { 30_000 };
        self.next_ms = Some(now_ms.saturating_add(delay as u64));
        RecoveryOutcome::RetryAfter(delay)
    }
    pub fn due(&mut self, now_ms: u64, busy: bool) -> bool {
        if busy || !self.next_ms.is_some_and(|time| now_ms >= time) {
            return false;
        }
        self.next_ms = None;
        true
    }
}

pub fn command(executable: &str) -> Result<String, String> {
    if executable.contains(['"', '\0', '\r', '\n'])
        || !executable
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
        || !executable.to_ascii_lowercase().ends_with("\\mcc-ui.exe")
        || executable.as_bytes().get(1) != Some(&b':')
        || executable.as_bytes().get(2) != Some(&b'\\')
    {
        return Err("Startup requires an absolute local path to mcc-ui.exe".into());
    }
    let result = format!("\"{executable}\" --tray");
    if result.encode_utf16().count() > 260 {
        return Err("Startup command exceeds Windows Run-key limit".into());
    }
    Ok(result)
}

#[cfg(windows)]
mod native {
    use super::Status;
    use std::{os::windows::process::CommandExt, process::Command, time::Duration};

    fn request(action: &str) -> Result<Status, String> {
        let path = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("mcc-ui.exe");
        let physical = crate::elevation::physical_shell_path(&path).map_err(|e| e.to_string())?;
        let expected = super::command(&physical.to_string_lossy())?;
        let mut cmd = Command::new(crate::platform::powershell_path()?);
        cmd.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            include_str!("startup.ps1"),
        ])
        .env("MCC_STARTUP_ACTION", action)
        .env("MCC_STARTUP_EXECUTABLE", physical)
        .env("MCC_STARTUP_COMMAND", expected)
        .creation_flags(0x08000000);
        let bytes = crate::process::run(&mut cmd, Duration::from_secs(12))
            .map_err(|e| format!("Windows startup request failed: {e:?}"))?;
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Reply {
            ok: bool,
            enabled: bool,
            managed: bool,
            error: Option<String>,
        }
        let reply: Reply =
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid startup reply: {e}"))?;
        if !reply.ok {
            return Err(reply.error.unwrap_or("Windows startup unavailable".into()));
        }
        Ok(Status {
            enabled: reply.enabled,
            managed: reply.managed,
        })
    }
    pub fn status() -> Result<Status, String> {
        request("status")
    }
    pub fn set(enabled: bool) -> Result<Status, String> {
        request(if enabled { "on" } else { "off" })
    }
    pub fn launch() -> Result<Status, String> {
        request("launch")
    }
}
#[cfg(windows)]
pub use native::{launch, set, status};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boot_recovery_is_bounded_and_never_overlaps_requests() {
        let mut r = Recovery::default();
        assert!(matches!(
            r.completed(1000, false),
            RecoveryOutcome::RetryAfter(10_000)
        ));
        assert!(!r.due(10_999, false));
        assert!(!r.due(11_000, true));
        assert!(r.due(11_000, false));
        assert!(!r.due(12_000, false));
        assert!(matches!(
            r.completed(12_000, false),
            RecoveryOutcome::RetryAfter(30_000)
        ));
        assert!(r.due(42_000, false));
        assert!(matches!(
            r.completed(43_000, false),
            RecoveryOutcome::Exhausted
        ));
        assert!(!r.due(u64::MAX, false));
        assert!(matches!(
            r.completed(44_000, true),
            RecoveryOutcome::Recovered
        ));
        assert!(!r.due(u64::MAX, false));
    }
    #[test]
    fn quoted_command_starts_hidden_without_elevation_or_control_arguments() {
        assert_eq!(
            command(r"C:\Program Files\Minimalist\mcc-ui.exe").unwrap(),
            "\"C:\\Program Files\\Minimalist\\mcc-ui.exe\" --tray"
        );
    }
    #[test]
    fn invalid_and_injected_paths_are_refused() {
        for p in [
            r"mcc-ui.exe",
            r"\\server\mcc-ui.exe",
            r"C:\other.exe",
            "C:\\bad\" --connect \\mcc-ui.exe",
            "C:\\bad\0\\mcc-ui.exe",
        ] {
            assert!(command(p).is_err(), "{p}");
        }
        assert!(command(&format!("C:\\{}\\mcc-ui.exe", "x".repeat(260))).is_err());
    }
}
