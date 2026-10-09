//! Opt-in NVIDIA temperature read using the documented NVAPI thermal interface.
use std::{ffi::c_void, ptr};
use windows_sys::Win32::{
    Foundation::FreeLibrary,
    System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
};
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Sensor {
    controller: i32,
    minimum: i32,
    maximum: i32,
    current: i32,
    target: i32,
}
#[repr(C)]
#[derive(Default)]
struct Thermal {
    version: u32,
    count: u32,
    sensors: [Sensor; 3],
}
type Query = unsafe extern "C" fn(u32) -> *mut c_void;
type Initialize = unsafe extern "C" fn() -> i32;
type Enumerate = unsafe extern "C" fn(*mut *mut c_void, *mut u32) -> i32;
type GetThermal = unsafe extern "C" fn(*mut c_void, u32, *mut Thermal) -> i32;

pub fn temperature() -> Result<i32, String> {
    let name: Vec<u16> = "nvapi64.dll".encode_utf16().chain(Some(0)).collect();
    // SAFETY: Search restricted to Windows System32, avoiding DLL loading from the app directory.
    let module =
        unsafe { LoadLibraryExW(name.as_ptr(), ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if module.is_null() {
        return Err("NVIDIA NVAPI is unavailable; no driver is installed by this app".into());
    }
    let result = (|| {
        // SAFETY: Exact public NVIDIA export; module is live for all queries below.
        let export = unsafe { GetProcAddress(module, c"nvapi_QueryInterface".as_ptr().cast()) }
            .ok_or("NVAPI query export missing")?;
        // SAFETY: NVIDIA's documented query-interface ABI returns a pointer for a fixed interface ID.
        let query: Query = unsafe { std::mem::transmute(export) };
        // SAFETY: Verified interface IDs from NVIDIA's published nvapi_interface.h; no setting APIs.
        let (init, enumerate, thermal, unload) = unsafe {
            (
                query(0x0150e828),
                query(0xe5ac921f),
                query(0xe3640a56),
                query(0xd22bdd7e),
            )
        };
        if [init, enumerate, thermal, unload]
            .iter()
            .any(|p| p.is_null())
        {
            return Err("Required NVAPI interfaces unavailable".into());
        }
        // SAFETY: Exact documented function types for these fixed interface IDs.
        let (init, enumerate, thermal, unload): (Initialize, Enumerate, GetThermal, Initialize) = unsafe {
            (
                std::mem::transmute::<*mut c_void, Initialize>(init),
                std::mem::transmute::<*mut c_void, Enumerate>(enumerate),
                std::mem::transmute::<*mut c_void, GetThermal>(thermal),
                std::mem::transmute::<*mut c_void, Initialize>(unload),
            )
        };
        // SAFETY: NVAPI initialization takes no arguments.
        let status = unsafe { init() };
        if status != 0 {
            return Err(format!("NVAPI initialization failed ({status})"));
        }
        let result = (|| {
            let mut handles = [ptr::null_mut(); 64];
            let mut count = 0;
            // SAFETY: NVAPI specifies a maximum of 64 physical GPU handles and a u32 count.
            let status = unsafe { enumerate(handles.as_mut_ptr(), &mut count) };
            if status != 0 || !(1..=64).contains(&count) || handles[0].is_null() {
                return Err(format!("NVIDIA GPU unavailable ({status})"));
            }
            let mut data = Thermal {
                version: std::mem::size_of::<Thermal>() as u32 | (2 << 16),
                ..Default::default()
            };
            // SAFETY: Live NVAPI GPU handle, sensor index zero and correctly versioned V2 structure.
            let status = unsafe { thermal(handles[0], 0, &mut data) };
            if status != 0 || !(1..=3).contains(&data.count) {
                return Err(format!("GPU temperature unavailable ({status})"));
            }
            let sensor = data.sensors[0];
            if sensor.target != 1 || !(-20..=125).contains(&sensor.current) {
                return Err("GPU sensor returned an invalid target or temperature".into());
            }
            Ok(sensor.current)
        })();
        // SAFETY: Balance successful NVAPI initialization on every result path.
        unsafe {
            unload();
        }
        result
    })();
    // SAFETY: Balance this function's successful LoadLibraryExW after calls have finished.
    unsafe {
        FreeLibrary(module);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thermal_abi_matches_nvidia_v2() {
        assert_eq!(std::mem::size_of::<Thermal>(), 68);
        assert_eq!(std::mem::size_of::<Sensor>(), 20);
    }
}
