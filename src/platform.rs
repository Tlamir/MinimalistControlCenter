//! Auditable OS boundary. Only registry queries and documented read-only counters.
use crate::{CpuTimes, Reading};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Identity {
    pub manufacturer: Reading<String>,
    pub model: Reading<String>,
    pub bios_version: Reading<String>,
    pub cpu: Reading<String>,
    pub display_adapters: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub memory_used_percent: Reading<u32>,
    pub physical_memory_bytes: Reading<u64>,
    pub ac_connected: Reading<bool>,
    pub battery_percent: Reading<u8>,
    pub system_cpu_times: Reading<CpuTimes>,
    pub cpu_temperature_c: Reading<f64>,
    pub gpu_temperature_c: Reading<f64>,
    pub fan_rpm: Reading<u32>,
    pub process: Reading<ProcessMetrics>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ProcessMetrics {
    pub cpu_time_100ns: u64,
    pub working_set_bytes: u64,
    pub peak_working_set_bytes: u64,
    pub private_commit_bytes: u64,
}

pub fn battery_percent(raw: u8) -> Reading<u8> {
    if raw <= 100 {
        Reading::available(raw)
    } else {
        Reading::unavailable("Battery absent or percentage unavailable")
    }
}

pub fn ac_connected(raw: u8) -> Reading<bool> {
    match raw {
        0 => Reading::available(false),
        1 => Reading::available(true),
        _ => Reading::unavailable("AC status unavailable"),
    }
}

#[cfg(windows)]
mod os {
    use super::*;
    use std::{ffi::c_void, mem::size_of, path::PathBuf};

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    impl FileTime {
        fn value(&self) -> u64 {
            (u64::from(self.high) << 32) | u64::from(self.low)
        }
    }
    #[repr(C)]
    #[derive(Default)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }
    #[repr(C)]
    #[derive(Default)]
    struct PowerStatus {
        ac: u8,
        flags: u8,
        percent: u8,
        saver: u8,
        life: u32,
        full_life: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct MemoryCounters {
        cb: u32,
        page_faults: u32,
        peak_working: usize,
        working: usize,
        peak_paged: usize,
        paged: usize,
        peak_nonpaged: usize,
        nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
        private: usize,
    }
    #[link(name = "advapi32", kind = "raw-dylib")]
    unsafe extern "system" {
        fn RegGetValueW(
            key: isize,
            subkey: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            size: *mut u32,
        ) -> i32;
    }
    #[link(name = "kernel32", kind = "raw-dylib")]
    unsafe extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        fn GetSystemPowerStatus(status: *mut PowerStatus) -> i32;
        fn GetSystemTimes(idle: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GetCurrentProcess() -> isize;
        fn GetProcessTimes(
            handle: isize,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn K32GetProcessMemoryInfo(handle: isize, counters: *mut MemoryCounters, size: u32) -> i32;
        fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn registry_string(subkey: &str, value: &str) -> Reading<String> {
        let subkey = wide(subkey);
        let value = wide(value);
        let mut buffer = [0u16; 1024];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: Fixed-size writable buffer, correct byte count, NUL-terminated inputs.
        // HKLM predefined handle; RRF_RT_REG_SZ accepts only a string. No write access.
        let result = unsafe {
            RegGetValueW(
                -2147483646isize,
                subkey.as_ptr(),
                value.as_ptr(),
                2,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if result != 0 {
            return Reading::unavailable(format!(
                "Registry value unavailable (Windows status {result})"
            ));
        }
        if bytes as usize > buffer.len() * 2 || !bytes.is_multiple_of(2) {
            return Reading::unavailable("Invalid registry string length");
        }
        let count = bytes as usize / 2;
        let end = buffer[..count]
            .iter()
            .position(|&v| v == 0)
            .unwrap_or(count);
        match String::from_utf16(&buffer[..end]) {
            Ok(s) if !s.trim().is_empty() => Reading::available(s.trim().to_owned()),
            _ => Reading::unavailable("Empty or invalid registry string"),
        }
    }

    pub fn identity() -> Identity {
        let bios = "HARDWARE\\DESCRIPTION\\System\\BIOS";
        let mut adapters = Vec::new();
        for index in 0..32 {
            let key = format!(
                "SYSTEM\\CurrentControlSet\\Control\\Class\\{{4d36e968-e325-11ce-bfc1-08002be10318}}\\{index:04}"
            );
            if let Some(value) = registry_string(&key, "DriverDesc").value {
                if !adapters.contains(&value) {
                    adapters.push(value);
                }
            }
        }
        Identity {
            manufacturer: registry_string(bios, "SystemManufacturer"),
            model: registry_string(bios, "SystemProductName"),
            bios_version: registry_string(bios, "BIOSVersion"),
            cpu: registry_string(
                "HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
                "ProcessorNameString",
            ),
            display_adapters: adapters,
        }
    }

    pub fn metrics() -> Reading<ProcessMetrics> {
        let mut memory = MemoryCounters {
            cb: size_of::<MemoryCounters>() as u32,
            ..Default::default()
        };
        let (mut creation, mut exit, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        // SAFETY: Pseudo-handle for this process only; no resource ownership transferred.
        let process = unsafe { GetCurrentProcess() };
        // SAFETY: repr(C) structure and matching size, valid writable pointers.
        let mem_ok = unsafe {
            K32GetProcessMemoryInfo(process, &mut memory, size_of::<MemoryCounters>() as u32)
        };
        // SAFETY: Valid current-process handle and four writable FILETIME structures.
        let time_ok =
            unsafe { GetProcessTimes(process, &mut creation, &mut exit, &mut kernel, &mut user) };
        if mem_ok == 0 || time_ok == 0 {
            return Reading::unavailable("Process counters unavailable");
        }
        Reading::available(ProcessMetrics {
            cpu_time_100ns: kernel.value().saturating_add(user.value()),
            working_set_bytes: memory.working as u64,
            peak_working_set_bytes: memory.peak_working as u64,
            private_commit_bytes: memory.private as u64,
        })
    }

    pub fn snapshot() -> Snapshot {
        let mut mem = MemoryStatus {
            length: size_of::<MemoryStatus>() as u32,
            ..Default::default()
        };
        let mut power = PowerStatus::default();
        let (mut idle, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        // SAFETY: repr(C) buffer includes the required size field and writable storage.
        let memory_ok = unsafe { GlobalMemoryStatusEx(&mut mem) } != 0;
        // SAFETY: Valid writable SYSTEM_POWER_STATUS layout.
        let power_ok = unsafe { GetSystemPowerStatus(&mut power) } != 0;
        // SAFETY: Three valid writable FILETIME outputs.
        let cpu_ok = unsafe { GetSystemTimes(&mut idle, &mut kernel, &mut user) } != 0;
        Snapshot {
            memory_used_percent: if memory_ok && mem.load <= 100 {
                Reading::available(mem.load)
            } else {
                Reading::unavailable("Memory counter unavailable")
            },
            physical_memory_bytes: if memory_ok {
                Reading::available(mem.total_phys)
            } else {
                Reading::unavailable("Memory counter unavailable")
            },
            ac_connected: if power_ok {
                ac_connected(power.ac)
            } else {
                Reading::unavailable("Power status unavailable")
            },
            battery_percent: if power_ok && power.flags != 255 && power.flags & 128 == 0 {
                battery_percent(power.percent)
            } else {
                Reading::unavailable("Battery absent or status unavailable")
            },
            system_cpu_times: if cpu_ok {
                Reading::available(CpuTimes {
                    idle: idle.value(),
                    kernel: kernel.value(),
                    user: user.value(),
                })
            } else {
                Reading::unavailable("CPU counters unavailable")
            },
            cpu_temperature_c: Reading::unavailable("CPU temperature protocol not verified"),
            gpu_temperature_c: Reading::unavailable("GPU telemetry backend not enabled"),
            fan_rpm: Reading::unavailable("EC fan telemetry protocol not verified"),
            process: metrics(),
        }
    }

    pub fn powershell_path() -> Result<PathBuf, String> {
        let mut buffer = [0u16; 32768];
        // SAFETY: Writable UTF-16 buffer and its actual capacity in characters.
        let size =
            unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
        if size == 0 || size >= buffer.len() {
            return Err("Cannot locate trusted Windows system directory".into());
        }
        let directory =
            String::from_utf16(&buffer[..size]).map_err(|_| "Invalid system directory")?;
        Ok(PathBuf::from(directory).join("WindowsPowerShell\\v1.0\\powershell.exe"))
    }
}

#[cfg(not(windows))]
mod os {
    use super::*;
    pub fn identity() -> Identity {
        Identity {
            manufacturer: Reading::unavailable("Windows only"),
            model: Reading::unavailable("Windows only"),
            bios_version: Reading::unavailable("Windows only"),
            cpu: Reading::unavailable("Windows only"),
            display_adapters: vec![],
        }
    }
    pub fn metrics() -> Reading<ProcessMetrics> {
        Reading::unavailable("Windows only")
    }
    pub fn snapshot() -> Snapshot {
        Snapshot {
            memory_used_percent: Reading::unavailable("Windows only"),
            physical_memory_bytes: Reading::unavailable("Windows only"),
            ac_connected: Reading::unavailable("Windows only"),
            battery_percent: Reading::unavailable("Windows only"),
            system_cpu_times: Reading::unavailable("Windows only"),
            cpu_temperature_c: Reading::unavailable("Not verified"),
            gpu_temperature_c: Reading::unavailable("Not verified"),
            fan_rpm: Reading::unavailable("Not verified"),
            process: metrics(),
        }
    }
    pub fn powershell_path() -> Result<std::path::PathBuf, String> {
        Err("WMI discovery requires Windows".into())
    }
}
pub use os::{identity, metrics, powershell_path, snapshot};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_battery_never_becomes_zero() {
        for n in 101..=255 {
            assert!(battery_percent(n).value.is_none());
        }
    }
    #[test]
    fn known_battery_boundaries() {
        assert_eq!(battery_percent(0).value, Some(0));
        assert_eq!(battery_percent(100).value, Some(100));
    }
    #[test]
    fn unknown_ac_never_becomes_offline() {
        assert_eq!(ac_connected(0).value, Some(false));
        assert_eq!(ac_connected(1).value, Some(true));
        for n in 2..=255 {
            assert!(ac_connected(n).value.is_none());
        }
    }
}
