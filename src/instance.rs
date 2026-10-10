//! One real UI per privilege level; test/demo processes remain isolated.
use std::ptr;
use windows_sys::Win32::{
    Foundation::*,
    System::Threading::*,
    UI::{Shell::IsUserAnAdmin, WindowsAndMessaging::*},
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
pub struct Instance(HANDLE);
impl Drop for Instance {
    fn drop(&mut self) {
        // SAFETY: Owned mutex handle closed once. Mutex was not acquired/owned.
        unsafe {
            CloseHandle(self.0);
        }
    }
}
pub fn class() -> &'static str {
    // SAFETY: Queries only this process's token.
    if unsafe { IsUserAnAdmin() } != 0 {
        "MinimalistControlCenter.Native.Admin"
    } else {
        "MinimalistControlCenter.Native.User"
    }
}
pub fn show_existing() -> bool {
    // SAFETY: Restore only this application's existing admin UI, with no pointer messages.
    unsafe {
        let hwnd = FindWindowW(
            wide("MinimalistControlCenter.Native.Admin").as_ptr(),
            ptr::null(),
        );
        if hwnd.is_null() {
            return false;
        }
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
        true
    }
}
pub fn acquire(hidden: bool) -> Result<Option<Instance>, String> {
    let class = class();
    // SAFETY: Queries an existing own window class; no pointers or hardware commands sent.
    unsafe {
        let admin = FindWindowW(
            wide("MinimalistControlCenter.Native.Admin").as_ptr(),
            ptr::null(),
        );
        if !admin.is_null() {
            if !hidden {
                ShowWindow(admin, SW_RESTORE);
                SetForegroundWindow(admin);
            }
            return Ok(None);
        }
    }
    let name = format!("Local\\{class}");
    // SAFETY: Fixed local mutex name, default security; handle retained for UI lifetime.
    let handle = unsafe { CreateMutexW(ptr::null(), 0, wide(&name).as_ptr()) };
    // SAFETY: Read last error immediately after CreateMutexW.
    let error = unsafe { GetLastError() };
    if handle.is_null() {
        return Err(format!("Cannot open app instance lock ({error})"));
    }
    let guard = Instance(handle);
    if error == ERROR_ALREADY_EXISTS {
        if !hidden {
            // SAFETY: Restore existing UI only; no cross-process pointer payload.
            unsafe {
                let hwnd = FindWindowW(wide(class).as_ptr(), ptr::null());
                if !hwnd.is_null() {
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                }
            }
        }
        Ok(None)
    } else {
        Ok(Some(guard))
    }
}
