//! Native control center. Diagnostics are read-only; controls use an explicit typed backend.
pub mod cli;
pub mod control;
#[cfg(windows)]
pub mod desktop;
#[cfg(windows)]
pub mod gpu;
#[cfg(windows)]
pub mod hardware;
pub mod lighting;
#[cfg(windows)]
mod oem;
pub mod platform;
pub mod policy;
pub mod process;
pub mod schema;
pub mod settings;

use serde::Serialize;
use std::time::Duration;

pub const MODE: &str = "read_only";

#[derive(Debug, Clone, Serialize)]
pub struct Reading<T> {
    pub value: Option<T>,
    pub unavailable_reason: Option<String>,
}

impl<T> Reading<T> {
    pub fn available(value: T) -> Self {
        Self {
            value: Some(value),
            unavailable_reason: None,
        }
    }
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            value: None,
            unavailable_reason: Some(reason.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

/// Windows kernel time includes idle time. First samples and counter resets are unknown.
pub fn cpu_usage(previous: CpuTimes, current: CpuTimes) -> Option<f64> {
    let idle = current.idle.checked_sub(previous.idle)?;
    let kernel = current.kernel.checked_sub(previous.kernel)?;
    let user = current.user.checked_sub(previous.user)?;
    let total = kernel.checked_add(user)?;
    if total == 0 || idle > total {
        return None;
    }
    Some(100.0 * (total - idle) as f64 / total as f64)
}

/// Monotonic scheduling: skip missed ticks rather than generating a catch-up burst.
pub fn next_tick(previous: Duration, now: Duration, interval: Duration) -> Option<Duration> {
    if interval.is_zero() {
        return None;
    }
    let due = previous.checked_add(interval)?;
    if due > now {
        Some(due)
    } else {
        now.checked_add(interval)
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub version: &'static str,
    pub mode: &'static str,
    pub hardware_control_available: bool,
    pub identity: platform::Identity,
    pub snapshot: platform::Snapshot,
    pub limitations: [&'static str; 3],
}

pub fn report() -> Report {
    Report {
        schema_version: 1,
        version: env!("CARGO_PKG_VERSION"),
        mode: MODE,
        hardware_control_available: false,
        identity: platform::identity(),
        snapshot: platform::snapshot(),
        limitations: [
            "This status command reads OS counters only. Explicit hardware commands are separate.",
            "CPU temperature and fan RPM require independently verified telemetry; missing values remain unknown.",
            "Hardware controls require the supported fingerprint, capabilities, administrator access and no competing OEM application.",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cpu_kernel_includes_idle() {
        assert_eq!(
            cpu_usage(
                CpuTimes {
                    idle: 10,
                    kernel: 20,
                    user: 10
                },
                CpuTimes {
                    idle: 30,
                    kernel: 60,
                    user: 30
                }
            ),
            Some(100.0 * 40.0 / 60.0)
        );
    }
    #[test]
    fn reset_and_impossible_cpu_data_are_unknown() {
        let a = CpuTimes {
            idle: 10,
            kernel: 20,
            user: 10,
        };
        assert_eq!(cpu_usage(a, a), None);
        assert_eq!(
            cpu_usage(
                a,
                CpuTimes {
                    idle: 0,
                    kernel: 0,
                    user: 0
                }
            ),
            None
        );
        assert_eq!(
            cpu_usage(
                a,
                CpuTimes {
                    idle: 100,
                    kernel: 21,
                    user: 11
                }
            ),
            None
        );
    }
    #[test]
    fn missed_ticks_never_catch_up_in_a_burst() {
        assert_eq!(
            next_tick(
                Duration::ZERO,
                Duration::from_secs(20),
                Duration::from_secs(1)
            ),
            Some(Duration::from_secs(21))
        );
        assert_eq!(
            next_tick(Duration::ZERO, Duration::ZERO, Duration::ZERO),
            None
        );
    }
    #[test]
    fn unavailable_values_serialize_as_null_not_zero() {
        let v = serde_json::to_value(Reading::<f64>::unavailable("not verified")).unwrap();
        assert!(v["value"].is_null());
        assert_eq!(v["unavailable_reason"], "not verified");
    }
    #[test]
    fn read_only_report_has_no_control_capability() {
        let r = report();
        assert!(!r.hardware_control_available);
        assert_eq!(r.mode, "read_only");
        assert!(r.snapshot.cpu_temperature_c.value.is_none());
        assert!(r.snapshot.gpu_temperature_c.value.is_none());
        assert!(r.snapshot.fan_rpm.value.is_none());
    }
}
