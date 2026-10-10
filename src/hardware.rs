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
#[derive(Serialize)]
struct Input {
    #[serde(rename = "Data")]
    data: u64,
}
#[derive(Deserialize)]
struct Output {
    #[serde(rename = "Return")]
    value: u32,
}

struct NativeEc {
    wmi: WMIConnection,
    path: String,
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
        Ok(Self {
            wmi,
            path: row.path,
        })
    }
    fn transact(&self, data: u64) -> Result<u32, String> {
        let result: Output = self
            .wmi
            .exec_instance_method::<Instance, Output>(&self.path, "GetSetULong", Input { data })
            .map_err(|e| format!("EC transaction failed: {e}"))?;
        Ok(result.value)
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
        control::decode_ec_byte(self.transact(control::read_payload(address))?)
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
#[serde(rename = "Win32_Process")]
struct Process {
    #[serde(rename = "Name")]
    name: String,
}
#[derive(Deserialize)]
#[serde(rename = "Win32_Service")]
struct Service {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "State")]
    state: String,
    #[serde(rename = "PathName")]
    path: Option<String>,
}

fn check_competing_controller() -> Result<(), String> {
    let c = WMIConnection::with_namespace_path("root\\CIMV2")
        .map_err(|e| format!("Cannot check OEM ownership: {e}"))?;
    let processes: Vec<Process> = c
        .raw_query("SELECT Name FROM Win32_Process")
        .map_err(|e| format!("Cannot check running controllers: {e}"))?;
    for p in processes {
        if matches!(
            p.name.to_ascii_lowercase().as_str(),
            "gamingcenter.exe" | "oemservicewinapp.exe" | "controlcenter.exe" | "gamingcenteru.exe"
        ) {
            return Err(format!(
                "Close {} before applying controls; two controllers can overwrite each other",
                p.name
            ));
        }
    }
    let services: Vec<Service> = c
        .raw_query("SELECT Name, State, PathName FROM Win32_Service")
        .map_err(|e| format!("Cannot check OEM services: {e}"))?;
    for s in services {
        let path = s.path.unwrap_or_default().to_ascii_lowercase();
        if s.state == "Running"
            && (path.contains("gamingcenter")
                || path.contains("oemservice")
                || path.contains("monster kontrol merkezi"))
        {
            return Err(format!(
                "OEM service {} is running. Hardware writes refused until ownership is resolved",
                s.name
            ));
        }
    }
    Ok(())
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
    let mut ec = NativeEc::connect()?;
    let mode = control::observed_mode(&mut ec)?;
    let ownership_reason = check_competing_controller().err();
    Ok(Observation {
        mode,
        writes_allowed: ownership_reason.is_none(),
        ownership_reason,
    })
}

pub fn status() -> Result<HardwareStatus, String> {
    let _guard = Ownership::acquire()?;
    let mut ec = NativeEc::connect()?;
    let capabilities = control::capabilities(&mut ec)?;
    let ownership_reason = check_competing_controller().err();
    Ok(HardwareStatus {
        writes_allowed: ownership_reason.is_none(),
        capabilities,
        ownership_reason,
    })
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
        for row in &transfer.row_select {
            device.send_feature_report(row).map_err(|e| e.to_string())?;
            std::thread::sleep(Duration::from_millis(2));
            if device.write(&transfer.pixels).map_err(|e| e.to_string())? != transfer.pixels.len() {
                return Err("Short keyboard output transfer".into());
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        return Ok(());
    }
    for report in lighting::reports(config)? {
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
