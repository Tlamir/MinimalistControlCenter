//! Temporary handover of the existing, path-verified OEM launcher. Startup type stays unchanged.
use std::{
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::Services::*;
struct Handle(SC_HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: Each non-null owned service-manager/service handle is closed once.
        unsafe {
            CloseServiceHandle(self.0);
        }
    }
}
pub(crate) fn change(start: bool) -> Result<(), String> {
    // SAFETY: Local service manager, minimum connect access; no service creation or configuration.
    let scm = Handle(unsafe { OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT) });
    if scm.0.is_null() {
        return Err("Administrator access is required for OEM handover".into());
    }
    let name: Vec<u16> = "GamingCenter".encode_utf16().chain(Some(0)).collect();
    // SAFETY: Only the fixed existing service name is opened; no user-supplied name is accepted.
    let service = Handle(unsafe {
        OpenServiceW(
            scm.0,
            name.as_ptr(),
            SERVICE_QUERY_STATUS | if start { SERVICE_START } else { SERVICE_STOP },
        )
    });
    if service.0.is_null() {
        return Err("The reviewed GamingCenter service is unavailable or access is denied".into());
    }
    let mut status = SERVICE_STATUS::default();
    // SAFETY: Valid service handle and writable correctly sized SERVICE_STATUS.
    if unsafe { QueryServiceStatus(service.0, &mut status) } == 0 {
        return Err("Cannot query OEM service status".into());
    }
    if status.dwServiceType != SERVICE_WIN32_OWN_PROCESS {
        return Err(
            "OEM service type changed; refusing to operate on a shared service or driver".into(),
        );
    }
    let goal = if start {
        SERVICE_RUNNING
    } else {
        SERVICE_STOPPED
    };
    if status.dwCurrentState == goal {
        return Ok(());
    }
    // SAFETY: Fixed existing user-mode service, documented start/stop operation; no configuration writes.
    let result = unsafe {
        if start {
            StartServiceW(service.0, 0, ptr::null())
        } else {
            ControlService(service.0, SERVICE_CONTROL_STOP, &mut status)
        }
    };
    if result == 0 {
        return Err("OEM service handover request failed; no forced termination attempted".into());
    }
    let begin = Instant::now();
    while begin.elapsed() < Duration::from_secs(3) {
        // SAFETY: Service handle remains owned; status output is valid.
        if unsafe { QueryServiceStatus(service.0, &mut status) } == 0 {
            return Err("Cannot confirm OEM handover".into());
        }
        if status.dwCurrentState == goal {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("OEM handover timed out; inspect Services before retrying".into())
}
