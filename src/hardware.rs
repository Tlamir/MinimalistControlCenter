//! Native WMI adapter. Vendor code is never loaded. Calls run in a disposable helper process.
use crate::{
    control::{self, Capabilities, Ec, FanMode, Switch},
    lighting::{self, Lighting},
};
use serde::{Deserialize, Serialize};
use std::{ptr, time::Duration};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0},
    System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
};
use wmi::WMIConnection;

#[derive(Deserialize)]
#[serde(rename = "AcpiTest_MULong")]
struct Instance {
    #[serde(rename = "__Path")]
    path: String,
    #[serde(rename = "InstanceName")]
    name: String,
}
struct NativeEc {
    wmi: WMIConnection,
    path: String,
    /// Input signature of GetSetULong, fetched once. Re-reading the class definition on every
    /// transaction added a WMI round trip to each EC byte.
    input: wmi::IWbemClassWrapper,
    /// Firmware feature-flag bytes (SUPPORT1, SUPPORT2, BOOST2). They are fixed by the BIOS, so
    /// one plausible read per connection is reused; each EC read costs about 135 ms.
    support: [Option<u8>; 3],
}
impl NativeEc {
    fn connect() -> Result<Self, String> {
        let id = crate::platform::identity();
        if !control::supported_identity(
            id.manufacturer.value.as_deref().unwrap_or(""),
            id.model.value.as_deref().unwrap_or(""),
            id.bios_version.value.as_deref().unwrap_or(""),
            id.cpu.value.as_deref().unwrap_or(""),
            &id.display_adapters,
        ) {
            return Err("Unsupported hardware fingerprint. Expected MONSTER TULPAR T5 V17.1 / N.1.06 / i7-8750H / GTX 1060".into());
        }
        let wmi = WMIConnection::with_namespace_path("root\\WMI")
            .map_err(|e| format!("WMI connection: {e}. Administrator access may be required"))?;
        let rows: Vec<Instance> = wmi
            .raw_query("SELECT __Path, InstanceName FROM AcpiTest_MULong")
            .map_err(|e| {
                format!(
                    "Cannot read laptop firmware: {e}. Enable controls with administrator access"
                )
            })?;
        let mut rows = rows.into_iter().filter(|r| r.name == "ACPI\\PNP0C14\\1_1");
        let row = rows.next().ok_or("Reviewed ACPI instance not present")?;
        if rows.next().is_some() {
            return Err("Ambiguous ACPI instance; refusing access".into());
        }
        let input = wmi
            .get_object("AcpiTest_MULong")
            .and_then(|class| class.get_method("GetSetULong"))
            .map_err(|e| format!("Cannot read firmware method: {e}"))?
            .ok_or("Reviewed firmware method has no input signature")?;
        Ok(Self {
            wmi,
            path: row.path,
            input,
            support: [None; 3],
        })
    }
    fn transact(&self, data: u64) -> Result<u32, String> {
        let fail = |e: wmi::WMIError| format!("EC transaction failed: {e}");
        // A fresh parameter instance per call; only the signature lookup is cached.
        let params = self.input.spawn_instance().map_err(fail)?;
        params.put_property("Data", data).map_err(fail)?;
        let output = self
            .wmi
            .exec_method(&self.path, "GetSetULong", Some(&params))
            .map_err(fail)?
            .ok_or("EC transaction returned no result")?;
        match output.get_property("Return").map_err(fail)? {
            wmi::Variant::UI4(v) => Ok(v),
            wmi::Variant::I4(v) => u32::try_from(v).map_err(|_| "EC result out of range".into()),
            other => Err(format!("Unexpected EC result type: {other:?}")),
        }
    }
}
impl Ec for NativeEc {
    fn read(&mut self, address: u16) -> Result<u8, String> {
        if !matches!(
            address,
            control::MODE
                | control::SUPPORT1
                | control::SUPPORT2
                | control::BOOST2
                | control::STATUS
                | control::TRIGGER
                | 1086 | 6148 | 1922 | 1859..=1863 | 1929..=1933
        ) {
            return Err("EC read is outside the allowlist".into());
        }
        let slot = [control::SUPPORT1, control::SUPPORT2, control::BOOST2]
            .iter()
            .position(|a| *a == address);
        if let Some(cached) = slot.and_then(|i| self.support[i]) {
            return Ok(cached);
        }
        let value = control::decode_ec_byte(self.transact(control::read_payload(address))?)?;
        // 255 means an unreadable EC; never cache it, so the plausibility check sees it again.
        if let (Some(i), true) = (slot, value != 255) {
            self.support[i] = Some(value);
        }
        Ok(value)
    }
    fn write(&mut self, address: u16, value: u8) -> Result<(), String> {
        if !(address == control::MODE && matches!(value, 0 | 64) || address == control::TRIGGER) {
            return Err("EC write is outside the typed allowlist".into());
        }
        self.transact(control::write_payload(address, value))?;
        Ok(())
    }
    fn settle(&mut self) {
        std::thread::sleep(Duration::from_millis(100));
    }
}

struct Ownership(HANDLE);
impl Ownership {
    fn acquire() -> Result<Self, String> {
        let name: Vec<u16> = "Global\\MinimalistControlCenter.EC.Transaction"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: NUL-terminated mutex name, default security descriptor, no initial ownership.
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err("Cannot create hardware ownership mutex".into());
        }
        // SAFETY: Handle returned by CreateMutexW; bounded wait prevents helper pileups.
        let wait = unsafe { WaitForSingleObject(handle, 1000) };
        if wait != WAIT_OBJECT_0 && wait != WAIT_ABANDONED {
            // SAFETY: Close exactly this acquired handle on failure.
            unsafe {
                CloseHandle(handle);
            }
            return Err("Another hardware transaction is active".into());
        }
        Ok(Self(handle))
    }
}
impl Drop for Ownership {
    fn drop(&mut self) {
        // SAFETY: Owned mutex on the same thread that acquired it; then close once.
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

#[derive(Deserialize)]
#[serde(rename = "Win32_Service")]
struct Service {
    #[serde(rename = "PathName")]
    path: Option<String>,
}

/// Refuse writes while the OEM controller runs. Native process/service enumeration replaces
/// the previous WMI queries (about 0.7 s per request) with the same matching rules.
fn check_competing_controller() -> Result<(), String> {
    competing(&running_processes()?, &running_services()?)
}
pub(crate) fn competing(processes: &[String], services: &[(String, String)]) -> Result<(), String> {
    for name in processes {
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "gamingcenter.exe" | "oemservicewinapp.exe" | "controlcenter.exe" | "gamingcenteru.exe"
        ) {
            return Err(format!(
                "Close {name} before applying controls; two controllers can overwrite each other"
            ));
        }
    }
    for (name, path) in services {
        let path = path.to_ascii_lowercase();
        if path.contains("gamingcenter")
            || path.contains("oemservice")
            || path.contains("monster kontrol merkezi")
        {
            return Err(format!(
                "OEM service {name} is running. Hardware writes refused until ownership is resolved"
            ));
        }
    }
    Ok(())
}
fn utf16_until_nul(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|c| *c == 0).unwrap_or(s.len())])
}
/// Executable names of all running processes.
fn running_processes() -> Result<Vec<String>, String> {
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
            TH32CS_SNAPPROCESS,
        },
    };
    // SAFETY: Snapshot handle is checked, iterated with a correctly sized entry, closed once.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("Cannot check running controllers".into());
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut names = Vec::new();
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            names.push(utf16_until_nul(&entry.szExeFile));
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
        if names.is_empty() {
            return Err("Cannot check running controllers".into());
        }
        Ok(names)
    }
}
/// (name, binary path) of every running Win32 service. A service whose configuration
/// cannot be read reports an empty path, matching the previous WMI behaviour.
fn running_services() -> Result<Vec<(String, String)>, String> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, ENUM_SERVICE_STATUS_PROCESSW, EnumServicesStatusExW, OpenSCManagerW,
        OpenServiceW, QUERY_SERVICE_CONFIGW, QueryServiceConfigW, SC_ENUM_PROCESS_INFO,
        SC_MANAGER_ENUMERATE_SERVICE, SERVICE_ACTIVE, SERVICE_QUERY_CONFIG, SERVICE_RUNNING,
        SERVICE_WIN32,
    };
    // SAFETY: SCM handles are checked and closed once. Buffers are u64-aligned, sized from the
    // API's reported requirement, and outlive every pointer read out of them.
    unsafe {
        let scm = OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_ENUMERATE_SERVICE);
        if scm.is_null() {
            return Err("Cannot check OEM services".into());
        }
        let mut buffer: Vec<u64> = vec![0; 8192];
        let mut count = 0u32;
        let mut ok = false;
        for _ in 0..4 {
            let (mut needed, mut resume) = (0u32, 0u32);
            ok = EnumServicesStatusExW(
                scm,
                SC_ENUM_PROCESS_INFO,
                SERVICE_WIN32,
                SERVICE_ACTIVE,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * 8) as u32,
                &mut needed,
                &mut count,
                &mut resume,
                ptr::null(),
            ) != 0;
            if ok || needed == 0 {
                break;
            }
            // Services can start between calls; grow with headroom and retry from the start.
            buffer = vec![0; (needed as usize + 4096) / 8 + 1];
        }
        if !ok {
            CloseServiceHandle(scm);
            return Err("Cannot check OEM services".into());
        }
        let entries = std::slice::from_raw_parts(
            buffer.as_ptr().cast::<ENUM_SERVICE_STATUS_PROCESSW>(),
            count as usize,
        );
        let mut services = Vec::new();
        let mut config: Vec<u64> = vec![0; 1024];
        for e in entries {
            if e.ServiceStatusProcess.dwCurrentState != SERVICE_RUNNING {
                continue;
            }
            let mut name_len = 0;
            while *e.lpServiceName.add(name_len) != 0 {
                name_len += 1;
            }
            let name =
                String::from_utf16_lossy(std::slice::from_raw_parts(e.lpServiceName, name_len));
            let mut path = String::new();
            let service = OpenServiceW(scm, e.lpServiceName, SERVICE_QUERY_CONFIG);
            if !service.is_null() {
                let mut needed = 0u32;
                let mut read = QueryServiceConfigW(
                    service,
                    config.as_mut_ptr().cast(),
                    (config.len() * 8) as u32,
                    &mut needed,
                ) != 0;
                if !read && needed as usize > config.len() * 8 {
                    config = vec![0; needed as usize / 8 + 1];
                    read = QueryServiceConfigW(
                        service,
                        config.as_mut_ptr().cast(),
                        (config.len() * 8) as u32,
                        &mut needed,
                    ) != 0;
                }
                if read {
                    let c = &*config.as_ptr().cast::<QUERY_SERVICE_CONFIGW>();
                    if !c.lpBinaryPathName.is_null() {
                        let mut n = 0;
                        while *c.lpBinaryPathName.add(n) != 0 {
                            n += 1;
                        }
                        path = String::from_utf16_lossy(std::slice::from_raw_parts(
                            c.lpBinaryPathName,
                            n,
                        ));
                    }
                }
                CloseServiceHandle(service);
            }
            services.push((name, path));
        }
        CloseServiceHandle(scm);
        Ok(services)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareStatus {
    pub capabilities: Capabilities,
    pub writes_allowed: bool,
    pub ownership_reason: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub mode: u8,
    pub writes_allowed: bool,
    pub ownership_reason: Option<String>,
}
pub fn observe() -> Result<Observation, String> {
    let _guard = Ownership::acquire()?;
    observe_with(&mut NativeEc::connect()?)
}

/// Persistent hardware agent: one warm firmware connection serving typed requests.
/// Ownership and write permission are re-checked on every request and never cached.
pub fn agent() -> Result<(), String> {
    use crate::agent::{Request, Response};
    let mut ec: Option<NativeEc> = None;
    crate::agent::serve(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        |request| {
            if ec.is_none() {
                ec = Some(NativeEc::connect()?);
            }
            let ec = ec.as_mut().ok_or("Firmware connection unavailable")?;
            // Hold the mutex only for this request; release it before waiting for the next one.
            let _guard = Ownership::acquire()?;
            Ok(match request {
                Request::Observe => Response::Observation(observe_with(ec)?),
                Request::Status => Response::Status(status_with(ec)?),
                Request::Fan(mode) => {
                    check_competing_controller()?;
                    control::set_fan(ec, mode)?;
                    Response::Done
                }
                Request::Switch(s, on) => {
                    check_competing_controller()?;
                    control::set_switch(ec, s, on)?;
                    Response::Done
                }
            })
        },
    )
}
fn observe_with(ec: &mut NativeEc) -> Result<Observation, String> {
    let mode = control::observed_mode(ec)?;
    let ownership_reason = check_competing_controller().err();
    Ok(Observation {
        mode,
        writes_allowed: ownership_reason.is_none(),
        ownership_reason,
    })
}
fn status_with(ec: &mut NativeEc) -> Result<HardwareStatus, String> {
    let capabilities = control::capabilities(ec)?;
    let ownership_reason = check_competing_controller().err();
    Ok(HardwareStatus {
        writes_allowed: ownership_reason.is_none(),
        capabilities,
        ownership_reason,
    })
}

pub fn status() -> Result<HardwareStatus, String> {
    let _guard = Ownership::acquire()?;
    status_with(&mut NativeEc::connect()?)
}
/// Diagnostic raw candidates, NOT validated Celsius, RPM, percentages, or permission to write.
pub fn calibration_probe() -> Result<serde_json::Value, String> {
    let _guard = Ownership::acquire()?;
    let mut ec = NativeEc::connect()?;
    let mut raw = std::collections::BTreeMap::new();
    for address in [
        1086,
        6148,
        1922,
        control::MODE,
        control::BOOST2,
        1859,
        1860,
        1861,
        1862,
        1863,
        1929,
        1930,
        1931,
        1932,
        1933,
    ] {
        raw.insert(address.to_string(), ec.read(address)?);
    }
    Ok(
        serde_json::json!({"validated":false,"hardware_writes":0,"raw_bytes":raw,
        "note":"Read-only candidates. Compare independently across idle/load/cooldown; these are not calibrated temperatures or RPM."}),
    )
}
pub fn fan(mode: FanMode) -> Result<(), String> {
    let _guard = Ownership::acquire()?;
    let mut ec = NativeEc::connect()?;
    check_competing_controller()?;
    control::set_fan(&mut ec, mode)
}
pub fn switch(s: Switch, on: bool) -> Result<(), String> {
    let _guard = Ownership::acquire()?;
    let mut ec = NativeEc::connect()?;
    check_competing_controller()?;
    control::set_switch(&mut ec, s, on)
}
pub fn keyboard(config: &Lighting) -> Result<(), String> {
    let _guard = Ownership::acquire()?;
    // Keyboard writes are also gated on the reviewed laptop, independent of WMI access.
    let id = crate::platform::identity();
    if !control::supported_identity(
        id.manufacturer.value.as_deref().unwrap_or(""),
        id.model.value.as_deref().unwrap_or(""),
        id.bios_version.value.as_deref().unwrap_or(""),
        id.cpu.value.as_deref().unwrap_or(""),
        &id.display_adapters,
    ) {
        return Err("Unsupported keyboard/laptop fingerprint".into());
    }
    check_competing_controller()?;
    let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;
    let devices: Vec<_> = api
        .device_list()
        .filter(|d| {
            d.vendor_id() == 0x048d
                && d.product_id() == 0xce00
                && d.usage() == 1
                && matches!(d.usage_page(), 0xff02 | 0xff12)
        })
        .collect();
    if devices.len() != 1 {
        return Err("Exactly one supported ITE keyboard collection is required (048d:ce00, usage 1, page ff02/ff12)".into());
    }
    let device = devices[0].open_device(&api).map_err(|e| e.to_string())?;
    if config.effect == lighting::Effect::Solid && devices[0].usage_page() == 0xff02 {
        let transfer = lighting::solid_output(config)?;
        device
            .send_feature_report(&transfer.effect)
            .map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(2));
        device
            .send_feature_report(&transfer.picture)
            .map_err(|e| e.to_string())?;
        for _ in 0..8 {
            if device.write(&transfer.pixels).map_err(|e| e.to_string())? != transfer.pixels.len() {
                return Err("Short keyboard output transfer".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        return Ok(());
    }
    for report in lighting::reports_for_collection(config, devices[0].usage_page())? {
        device
            .send_feature_report(&report)
            .map_err(|e| e.to_string())?;
        std::thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}

pub fn handover(start_oem: bool) -> Result<(), String> {
    let _guard = Ownership::acquire()?;
    let mut ec = NativeEc::connect()?;
    let caps = control::capabilities(&mut ec)?;
    if caps.fan_mode_raw != 0 {
        return Err("Set automatic cooling in the original controller, exit its tray icon, then enable controls here".into());
    }
    let c = WMIConnection::with_namespace_path("root\\CIMV2").map_err(|e| e.to_string())?;
    let services: Vec<Service> = c
        .raw_query("SELECT Name, State, PathName FROM Win32_Service WHERE Name='GamingCenter'")
        .map_err(|e| e.to_string())?;
    let paths: Vec<&str> = services
        .iter()
        .map(|s| s.path.as_deref().unwrap_or(""))
        .collect();
    if !control::reviewed_launcher(&paths, start_oem)? {
        return Ok(());
    }
    crate::oem::change(start_oem)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }
    #[test]
    fn oem_processes_and_services_block_writes_case_insensitively() {
        assert!(competing(&names(&["explorer.exe", "svchost.exe"]), &[]).is_ok());
        for p in [
            "GamingCenter.exe",
            "OEMSERVICEWINAPP.EXE",
            "controlcenter.exe",
            "GamingCenterU.exe",
        ] {
            assert!(competing(&names(&[p]), &[]).unwrap_err().contains(p));
        }
        for path in [
            r#""C:\Program Files\OEM\Monster Kontrol Merkezi\LaunchServGM.exe""#,
            r"C:\Tools\GamingCenter\svc.exe",
            r"C:\Windows\OEMService.exe -k",
        ] {
            let services = [("GamingCenter".to_string(), path.to_string())];
            assert!(
                competing(&[], &services)
                    .unwrap_err()
                    .starts_with("OEM service GamingCenter")
            );
        }
        let unrelated = [("Spooler".to_string(), String::new())];
        assert!(competing(&[], &unrelated).is_ok());
    }
    #[test]
    fn native_inventory_reads_without_wmi_or_administrator_rights() {
        let processes = running_processes().unwrap();
        let own = std::env::current_exe().unwrap();
        let own = own
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        assert!(processes.iter().any(|p| p.to_ascii_lowercase() == own));
        let services = running_services().unwrap();
        assert!(services.iter().any(|(name, path)| {
            name.eq_ignore_ascii_case("EventLog") && path.to_ascii_lowercase().contains("svchost")
        }));
    }
}
