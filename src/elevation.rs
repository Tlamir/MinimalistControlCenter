//! Administrator launch errors must distinguish consent cancellation from launch failure.
/// Resolve package filesystem redirection before handing a path to the Windows shell.
#[cfg(windows)]
pub fn physical_shell_path(path: &std::path::Path) -> std::io::Result<std::path::PathBuf> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    let resolved = std::fs::canonicalize(path)?;
    let units: Vec<u16> = resolved.as_os_str().encode_wide().collect();
    // canonicalize returns extended-length syntax, which ShellExecute does not
    // consistently accept. Strip only the local-drive prefix, never arbitrary text.
    if units.starts_with(&[92, 92, 63, 92])
        && units
            .get(4)
            .is_some_and(|c| (*c as u8).is_ascii_alphabetic() && *c < 128)
        && units.get(5) == Some(&58)
        && units.get(6) == Some(&92)
    {
        Ok(std::ffi::OsString::from_wide(&units[4..]).into())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "A local drive installation is required",
        ))
    }
}

pub fn failure_message(code: u32) -> String {
    if code == 1223 {
        "Windows reports that administrator permission was declined (1223). Click Enable controls to retry.".into()
    } else {
        format!(
            "Could not start administrator controls. Windows error {code}: {}. The current app remains open.",
            std::io::Error::from_raw_os_error(code as i32)
        )
    }
}

#[cfg(windows)]
pub fn launch(owner: windows_sys::Win32::Foundation::HWND) -> Result<(), String> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::{
        Foundation::GetLastError,
        System::Com::{
            COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
        },
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let path = std::env::current_exe()
        .and_then(|p| physical_shell_path(&p))
        .map_err(|e| format!("Cannot locate this app's physical executable: {e}"))?;
    let file: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let directory: Vec<u16> = path
        .parent()
        .ok_or("Application directory is missing")?
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let args: Vec<u16> = "--connect\0".encode_utf16().collect();
    // SAFETY: Initialize COM on this UI thread; balance only successful initialization.
    let com = unsafe {
        CoInitializeEx(
            ptr::null(),
            COINIT_APARTMENTTHREADED as u32 | COINIT_DISABLE_OLE1DDE as u32,
        )
    };
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        hwnd: owner,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: args.as_ptr(),
        lpDirectory: directory.as_ptr(),
        nShow: SW_SHOWNORMAL,
        ..Default::default()
    };
    // SAFETY: All strings outlive the synchronous shell handoff; fixed executable and argument.
    let launched = unsafe { ShellExecuteExW(&mut info) };
    let error = if launched == 0 {
        // SAFETY: Capture the error before any other Windows call can overwrite it.
        unsafe { GetLastError() }
    } else {
        0
    };
    if com >= 0 {
        // SAFETY: Balances this thread's successful CoInitializeEx, including S_FALSE.
        unsafe { CoUninitialize() };
    }
    if launched == 0 {
        Err(failure_message(error))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn resolved_shell_path_names_the_same_file_without_extended_prefix() {
        let original = std::env::current_exe().unwrap();
        let resolved = physical_shell_path(&original).unwrap();
        assert!(resolved.is_absolute());
        assert!(!resolved.as_os_str().to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(
            std::fs::canonicalize(original).unwrap(),
            std::fs::canonicalize(resolved).unwrap()
        );
        assert!(
            physical_shell_path(std::path::Path::new(r"Z:\missing-mcc-install\mcc-ui.exe"))
                .is_err()
        );
    }
    #[test]
    fn only_explicit_consent_cancellation_is_reported_as_declined() {
        assert!(failure_message(1223).contains("declined"));
        for code in [0, 2, 3, 5, 32, 740, 1155] {
            let message = failure_message(code);
            assert!(!message.contains("declined"));
            assert!(message.contains(&format!("Windows error {code}:")));
        }
    }
}
