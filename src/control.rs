//! Typed EC operations. No arbitrary address/value entry point is public.
// Protocol code is tested on Linux too, but only the Windows adapter uses it in production.
#![cfg_attr(not(windows), allow(dead_code))]
use serde::{Deserialize, Serialize};

pub(crate) const MODE: u16 = 1873;
pub(crate) const SUPPORT1: u16 = 1893;
pub(crate) const SUPPORT2: u16 = 1894;
pub(crate) const STATUS: u16 = 1896;
pub(crate) const TRIGGER: u16 = 1895;
pub(crate) const BOOST2: u16 = 1858;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FanMode {
    Auto,
    Boost,
}

impl FanMode {
    pub fn byte(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::Boost => 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Switch {
    WindowsKeyLock,
    UsbCharging,
    LightBar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub fan_mode_raw: u8,
    pub boost: bool,
    pub windows_key_lock: bool,
    pub usb_charging: bool,
    pub light_bar: bool,
    pub windows_key_locked: bool,
    pub usb_charging_enabled: bool,
    pub light_bar_enabled: bool,
}

/// Implementations are private to the hardware adapter; tests inject a fake EC.
pub(crate) trait Ec {
    fn read(&mut self, address: u16) -> Result<u8, String>;
    fn write(&mut self, address: u16, value: u8) -> Result<(), String>;
    fn settle(&mut self) {}
}

pub(crate) fn read_payload(address: u16) -> u64 {
    (1u64 << 40) | u64::from(address)
}
/// This firmware returns a word starting at the requested byte. The OEM casts the low byte.
pub fn decode_ec_byte(value: u32) -> Result<u8, String> {
    if value > 65535 {
        return Err("EC read exceeds the verified 16-bit return width".into());
    }
    Ok((value & 255) as u8)
}
pub(crate) fn write_payload(address: u16, value: u8) -> u64 {
    (u64::from(value) << 16) | u64::from(address)
}

pub fn supported_identity(
    manufacturer: &str,
    model: &str,
    bios: &str,
    cpu: &str,
    gpu: &[String],
) -> bool {
    manufacturer == "MONSTER"
        && model == "TULPAR T5 V17.1"
        && bios == "N.1.06"
        && cpu.contains("i7-8750H")
        && gpu.iter().any(|s| s == "NVIDIA GeForce GTX 1060")
}

/// Missing OEM software is a valid standalone installation. Ambiguous/changed
/// services are never controlled, and restoring an absent service is an error.
pub(crate) fn reviewed_launcher(paths: &[&str], restoring: bool) -> Result<bool, String> {
    if paths.is_empty() {
        return if restoring {
            Err("The original controller is not installed".into())
        } else {
            Ok(false)
        };
    }
    if paths.len() != 1
        || !paths[0].trim_matches('"').eq_ignore_ascii_case(
            "C:\\Program Files\\OEM\\Monster Kontrol Merkezi\\LaunchServGM.exe",
        )
    {
        return Err(
            "Original controller service differs from the reviewed launcher; handover refused"
                .into(),
        );
    }
    Ok(true)
}

pub(crate) fn capabilities(ec: &mut impl Ec) -> Result<Capabilities, String> {
    let s1 = ec.read(SUPPORT1)?;
    let s2 = ec.read(SUPPORT2)?;
    let b2 = ec.read(BOOST2)?;
    let mode = ec.read(MODE)?;
    let status = ec.read(STATUS)?;
    let trigger = ec.read(TRIGGER)?;
    if s1 == 255
        || s2 == 255
        || b2 == 255
        || status == 255
        || trigger == 255
        || !matches!(mode, 0 | 16 | 64 | 128..=133 | 160)
    {
        return Err("Firmware capability/mode bytes are implausible; controls refused".into());
    }
    Ok(Capabilities {
        fan_mode_raw: mode,
        boost: s1 & 128 != 0 || b2 & 4 != 0,
        windows_key_lock: s1 & 32 != 0,
        usb_charging: s2 & 2 != 0,
        light_bar: s1 & 64 != 0,
        windows_key_locked: status & 1 != 0,
        usb_charging_enabled: trigger & 16 != 0,
        light_bar_enabled: status & 2 != 0,
    })
}
pub(crate) fn observed_mode(ec: &mut impl Ec) -> Result<u8, String> {
    let mode = ec.read(MODE)?;
    if !matches!(mode, 0 | 16 | 64 | 128..=133 | 160) {
        return Err("Invalid fan observation; stale state discarded".into());
    }
    Ok(mode)
}

pub(crate) fn set_fan(ec: &mut impl Ec, mode: FanMode) -> Result<(), String> {
    let caps = capabilities(ec)?;
    if mode == FanMode::Boost && !caps.boost {
        return Err("Firmware does not advertise boost".into());
    }
    if caps.fan_mode_raw == mode.byte() {
        return Ok(());
    }
    let result = ec.write(MODE, mode.byte()).and_then(|()| {
        ec.settle();
        if ec.read(MODE)? == mode.byte() {
            Ok(())
        } else {
            Err("Fan mode readback mismatch".into())
        }
    });
    if let Err(error) = result {
        let recovery = ec.write(MODE, 0).and_then(|()| {
            ec.settle();
            if ec.read(MODE)? == 0 {
                Ok(())
            } else {
                Err("auto readback mismatch".into())
            }
        });
        return Err(format!(
            "{error}; automatic-mode recovery: {}",
            match recovery {
                Ok(()) =>
                    "register confirmed (physical fan response still needs observation)".into(),
                Err(e) => format!("FAILED: {e}; use the OEM controls"),
            }
        ));
    }
    Ok(())
}

pub(crate) fn set_switch(ec: &mut impl Ec, switch: Switch, enabled: bool) -> Result<(), String> {
    let caps = capabilities(ec)?;
    let (supported, mask) = match switch {
        Switch::WindowsKeyLock => (caps.windows_key_lock, 1),
        Switch::UsbCharging => (caps.usb_charging, 16),
        Switch::LightBar => (caps.light_bar, 2),
    };
    if !supported {
        return Err("This firmware does not advertise that switch".into());
    }
    // Toggle bits are requests, not persistent settings. Wait for the preceding
    // request to be consumed before submitting another (e.g. light bar ON -> OFF).
    // Never replay a pending pulse, including one belonging to another switch.
    let mut trigger = ec.read(TRIGGER)?;
    for _ in 0..10 {
        if trigger & 3 == 0 || trigger == 255 {
            break;
        }
        ec.settle();
        trigger = ec.read(TRIGGER)?;
    }
    if trigger == 255 {
        return Err("Invalid trigger byte; no write performed".into());
    }
    if trigger & 3 != 0 {
        return Err("Previous switch request is still pending; no write performed".into());
    }
    // Re-read after pending requests were consumed. An earlier timed-out request
    // may already have reached the desired state; another pulse would undo it.
    let state = if switch == Switch::UsbCharging {
        trigger
    } else {
        ec.read(STATUS)?
    };
    if state == 255 {
        return Err("Invalid switch state; no write performed".into());
    }
    if (state & mask != 0) == enabled {
        return Ok(());
    }
    let value = if switch == Switch::UsbCharging && !enabled {
        trigger & !mask
    } else {
        trigger | mask
    };
    ec.write(TRIGGER, value)?;
    // Firmware can acknowledge asynchronously. Poll reads only: a second toggle
    // would undo a successful first one whose acknowledgement arrived late.
    for _ in 0..10 {
        ec.settle();
        let value = ec.read(if switch == Switch::UsbCharging {
            TRIGGER
        } else {
            STATUS
        })?;
        if value == 255 {
            return Err("Invalid switch readback; no repeated toggle attempted".into());
        }
        if (value & mask != 0) == enabled {
            return Ok(());
        }
    }
    Err("Switch acknowledgement timed out; no repeated toggle attempted".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn real_probe_word_values_decode_the_low_byte() {
        assert_eq!(decode_ec_byte(33760).unwrap(), 224);
        assert_eq!(decode_ec_byte(27648).unwrap(), 0);
        assert_eq!(decode_ec_byte(15362).unwrap(), 2);
        assert!(decode_ec_byte(65536).is_err());
        assert!(decode_ec_byte(u32::MAX).is_err());
    }
    #[test]
    fn standalone_installation_does_not_require_original_files_or_service() {
        assert!(!reviewed_launcher(&[], false).unwrap());
        assert!(reviewed_launcher(&[], true).is_err());
        let exact = "C:\\Program Files\\OEM\\Monster Kontrol Merkezi\\LaunchServGM.exe";
        assert!(reviewed_launcher(&[exact], false).unwrap());
        assert!(reviewed_launcher(&[exact, exact], false).is_err());
        assert!(reviewed_launcher(&["C:\\unexpected.exe"], false).is_err());
        assert!(reviewed_launcher(&[""], false).is_err());
    }
    struct Mock {
        values: BTreeMap<u16, u8>,
        writes: Vec<(u16, u8)>,
        fail: bool,
        ignore: bool,
        delay: usize,
        pending: Option<u8>,
    }
    impl Mock {
        fn new() -> Self {
            Self {
                values: [
                    (SUPPORT1, 224),
                    (SUPPORT2, 2),
                    (BOOST2, 4),
                    (MODE, 0),
                    (STATUS, 0),
                    (TRIGGER, 0),
                ]
                .into(),
                writes: vec![],
                fail: false,
                ignore: false,
                delay: 0,
                pending: None,
            }
        }
    }
    impl Ec for Mock {
        fn read(&mut self, a: u16) -> Result<u8, String> {
            if self.fail {
                Err("sensor failed".into())
            } else {
                self.values.get(&a).copied().ok_or("missing".into())
            }
        }
        fn write(&mut self, a: u16, v: u8) -> Result<(), String> {
            self.writes.push((a, v));
            if !self.ignore {
                self.values.insert(a, v);
                if a == TRIGGER {
                    self.pending = Some(v & 3);
                }
            }
            Ok(())
        }
        fn settle(&mut self) {
            if self.delay > 0 {
                self.delay -= 1;
                return;
            }
            if let Some(pulse) = self.pending.take() {
                self.values.insert(STATUS, self.values[&STATUS] ^ pulse);
                self.values.insert(TRIGGER, self.values[&TRIGGER] & !3);
            }
        }
    }
    #[test]
    fn all_byte_payloads_remain_disjoint() {
        for a in [MODE, SUPPORT1, SUPPORT2, STATUS, TRIGGER, BOOST2] {
            assert_eq!(read_payload(a), 1099511627776 + u64::from(a));
            for v in 0..=255 {
                assert_eq!(write_payload(a, v) & (1 << 40), 0);
                assert_eq!(write_payload(a, v) & 65535, u64::from(a));
            }
        }
    }
    #[test]
    fn wrong_firmware_and_gpu_never_match() {
        let gpu = vec!["NVIDIA GeForce GTX 1060".into()];
        assert!(supported_identity(
            "MONSTER",
            "TULPAR T5 V17.1",
            "N.1.06",
            "i7-8750H",
            &gpu
        ));
        assert!(!supported_identity(
            "MONSTER",
            "TULPAR T5 V17.1",
            "N.1.07",
            "i7-8750H",
            &gpu
        ));
        assert!(!supported_identity(
            "MONSTER",
            "TULPAR T5 V17.1",
            "N.1.06",
            "i7-8750H",
            &[]
        ));
    }
    #[test]
    fn automatic_is_idempotent() {
        let mut m = Mock::new();
        set_fan(&mut m, FanMode::Auto).unwrap();
        assert!(m.writes.is_empty());
    }
    #[test]
    fn observation_never_writes_and_rejects_invalid_state() {
        let mut m = Mock::new();
        m.values.insert(MODE, 64);
        assert_eq!(observed_mode(&mut m).unwrap(), 64);
        m.values.insert(MODE, 0);
        assert_eq!(observed_mode(&mut m).unwrap(), 0);
        m.values.insert(MODE, 255);
        assert!(observed_mode(&mut m).is_err());
        assert!(m.writes.is_empty());
    }
    #[test]
    fn boost_and_restore_only_touch_mode() {
        let mut m = Mock::new();
        set_fan(&mut m, FanMode::Boost).unwrap();
        set_fan(&mut m, FanMode::Auto).unwrap();
        assert_eq!(m.writes, [(MODE, 64), (MODE, 0)]);
    }
    #[test]
    fn invalid_or_missing_reads_never_write() {
        for a in [SUPPORT1, SUPPORT2, BOOST2, MODE, STATUS, TRIGGER] {
            let mut m = Mock::new();
            m.values.insert(a, 255);
            assert!(set_fan(&mut m, FanMode::Boost).is_err());
            assert!(m.writes.is_empty());
        }
        let mut m = Mock::new();
        m.fail = true;
        assert!(set_fan(&mut m, FanMode::Boost).is_err());
        assert!(m.writes.is_empty());
    }
    #[test]
    fn absent_boost_capability_never_writes() {
        let mut m = Mock::new();
        m.values.insert(SUPPORT1, 0);
        m.values.insert(BOOST2, 0);
        assert!(set_fan(&mut m, FanMode::Boost).is_err());
        assert!(m.writes.is_empty());
    }
    #[test]
    fn readback_failure_attempts_firmware_auto_once() {
        let mut m = Mock::new();
        m.ignore = true;
        assert!(
            set_fan(&mut m, FanMode::Boost)
                .unwrap_err()
                .contains("recovery")
        );
        assert_eq!(m.writes, [(MODE, 64), (MODE, 0)]);
    }
    #[test]
    fn desired_switch_state_is_idempotent() {
        let mut m = Mock::new();
        set_switch(&mut m, Switch::WindowsKeyLock, true).unwrap();
        set_switch(&mut m, Switch::WindowsKeyLock, true).unwrap();
        assert_eq!(m.writes, [(TRIGGER, 1)]);
    }
    #[test]
    fn usb_bit_preserves_other_bits() {
        let mut m = Mock::new();
        m.values.insert(TRIGGER, 128);
        set_switch(&mut m, Switch::UsbCharging, true).unwrap();
        set_switch(&mut m, Switch::UsbCharging, false).unwrap();
        assert_eq!(m.writes, [(TRIGGER, 144), (TRIGGER, 128)]);
    }
    #[test]
    fn failed_toggle_is_not_repeated() {
        let mut m = Mock::new();
        m.ignore = true;
        assert!(set_switch(&mut m, Switch::WindowsKeyLock, true).is_err());
        assert_eq!(m.writes.len(), 1);
    }
    #[test]
    fn light_bar_on_then_off_waits_for_late_acknowledgement() {
        let mut m = Mock::new();
        m.delay = 3;
        set_switch(&mut m, Switch::LightBar, true).unwrap();
        m.delay = 4;
        set_switch(&mut m, Switch::LightBar, false).unwrap();
        assert_eq!(m.writes, [(TRIGGER, 2), (TRIGGER, 2)]);
        assert_eq!(m.values[&STATUS] & 2, 0);
    }
    #[test]
    fn pending_pulse_is_never_reissued_or_overwritten() {
        let mut m = Mock::new();
        m.values.insert(TRIGGER, 1);
        assert!(set_switch(&mut m, Switch::LightBar, true).is_err());
        assert!(m.writes.is_empty());
        m.values.insert(TRIGGER, 2);
        assert!(set_switch(&mut m, Switch::UsbCharging, true).is_err());
        assert!(m.writes.is_empty());
    }
    #[test]
    fn delayed_timeout_submits_exactly_one_pulse() {
        let mut m = Mock::new();
        m.delay = 50;
        assert!(set_switch(&mut m, Switch::LightBar, true).is_err());
        assert_eq!(m.writes, [(TRIGGER, 2)]);
    }
    #[test]
    fn late_success_of_previous_request_is_not_toggled_back() {
        let mut m = Mock::new();
        m.values.insert(TRIGGER, 2);
        m.pending = Some(2);
        m.delay = 3;
        set_switch(&mut m, Switch::LightBar, true).unwrap();
        assert_eq!(m.values[&STATUS] & 2, 2);
        assert!(m.writes.is_empty());
    }
    #[test]
    fn boot_off_retry_consumes_a_late_pulse_without_turning_the_bar_back_on() {
        let mut m = Mock::new();
        m.values.insert(STATUS, 2);
        m.delay = 50;
        assert!(set_switch(&mut m, Switch::LightBar, false).is_err());
        assert_eq!(m.writes, [(TRIGGER, 2)]);
        // Reopening a slow provider must resolve the existing request before any new write.
        assert!(set_switch(&mut m, Switch::LightBar, false).is_err());
        assert_eq!(m.writes, [(TRIGGER, 2)]);
        m.delay = 3;
        set_switch(&mut m, Switch::LightBar, false).unwrap();
        assert_eq!(m.values[&STATUS] & 2, 0);
        assert_eq!(m.writes, [(TRIGGER, 2)]);
    }
    #[test]
    fn pending_request_must_be_consumed_even_if_old_state_matches_target() {
        let mut m = Mock::new();
        m.values.insert(TRIGGER, 2);
        m.values.insert(STATUS, 2);
        m.pending = Some(2);
        m.delay = 2;
        set_switch(&mut m, Switch::LightBar, true).unwrap();
        assert_eq!(m.writes, [(TRIGGER, 2)]);
        assert_eq!(m.values[&STATUS] & 2, 2);
    }
}
