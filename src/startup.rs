//! Current-user logon startup only. No elevation, service, or automatic hardware writes.
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
    use std::ptr;
    use windows_sys::Win32::{Foundation::*, System::Registry::*};
    const KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const VALUE: &str = "MinimalistControlCenter";
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: Handle is returned by RegCreateKeyExW and closed once.
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    fn open(write: bool) -> Result<Key, String> {
        let mut key = ptr::null_mut();
        // SAFETY: Fixed current-user key; valid UTF16 and output handle; no machine policy changes.
        let code = unsafe {
            if write {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    wide(KEY).as_ptr(),
                    0,
                    ptr::null(),
                    0,
                    KEY_QUERY_VALUE | KEY_SET_VALUE,
                    ptr::null(),
                    &mut key,
                    ptr::null_mut(),
                )
            } else {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    wide(KEY).as_ptr(),
                    0,
                    KEY_QUERY_VALUE,
                    &mut key,
                )
            }
        };
        if code != ERROR_SUCCESS {
            return Err(format!("Startup registry unavailable ({code})"));
        }
        Ok(Key(key))
    }
    fn expected() -> Result<String, String> {
        let path = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("mcc-ui.exe");
        if !path.is_file() {
            return Err("Keep mcc-ui.exe alongside mcc.exe before enabling startup".into());
        }
        super::command(&path.to_string_lossy())
    }
    pub fn enabled() -> Result<bool, String> {
        let key = open(false)?;
        let mut bytes = [0u16; 512];
        let mut size = (bytes.len() * 2) as u32;
        let mut kind = 0;
        // SAFETY: Buffer has size bytes capacity; fixed value name and live read-only key.
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                wide(VALUE).as_ptr(),
                ptr::null(),
                &mut kind,
                bytes.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        if code != ERROR_SUCCESS || kind != REG_SZ || !size.is_multiple_of(2) || size < 2 {
            return Err(format!("Cannot verify startup entry ({code})"));
        }
        let count = size as usize / 2;
        if count > bytes.len() || bytes[count - 1] != 0 {
            return Err("Malformed startup entry".into());
        }
        Ok(String::from_utf16(&bytes[..count - 1]).map_err(|e| e.to_string())? == expected()?)
    }
    pub fn set(enabled: bool) -> Result<(), String> {
        let command = expected()?;
        let key = open(true)?;
        let bytes = wide(&command);
        // SAFETY: Only this app's fixed HKCU value; bounded NUL-terminated UTF16 command.
        let code = unsafe {
            if enabled {
                RegSetValueExW(
                    key.0,
                    wide(VALUE).as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr().cast(),
                    (bytes.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key.0, wide(VALUE).as_ptr())
            }
        };
        if code != ERROR_SUCCESS && !(code == ERROR_FILE_NOT_FOUND && !enabled) {
            return Err(format!("Couldn't change startup ({code})"));
        }
        if self::enabled()? != enabled {
            return Err("Startup readback mismatch".into());
        }
        Ok(())
    }
}
#[cfg(windows)]
pub use native::{enabled, set};

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
