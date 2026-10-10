//! Explicit logon startup. Elevated startup is installed only in protected Program Files.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub enabled: bool,
    pub managed: bool,
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
