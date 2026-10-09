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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    let (supported, current, mask) = match switch {
        Switch::WindowsKeyLock => (caps.windows_key_lock, caps.windows_key_locked, 1),
        Switch::UsbCharging => (caps.usb_charging, caps.usb_charging_enabled, 16),
        Switch::LightBar => (caps.light_bar, caps.light_bar_enabled, 2),
    };
    if !supported {
        return Err("This firmware does not advertise that switch".into());
    }
    if current == enabled {
        return Ok(());
    }
    let trigger = ec.read(TRIGGER)?;
    if trigger == 255 {
        return Err("Invalid trigger byte; no write performed".into());
    }
    let value = if switch == Switch::UsbCharging && !enabled {
        trigger & !mask
    } else {
        trigger | mask
    };
    ec.write(TRIGGER, value)?;
    ec.settle();
    let after = capabilities(ec)?;
    let current = match switch {
        Switch::WindowsKeyLock => after.windows_key_locked,
        Switch::UsbCharging => after.usb_charging_enabled,
        Switch::LightBar => after.light_bar_enabled,
    };
    if current != enabled {
        return Err("Switch readback mismatch; no repeated toggle attempted".into());
    }
    Ok(())
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
    struct Mock {
        values: BTreeMap<u16, u8>,
        writes: Vec<(u16, u8)>,
        fail: bool,
        ignore: bool,
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
                    let old = self.values[&STATUS];
                    self.values.insert(STATUS, old ^ (v & 3));
                }
            }
            Ok(())
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
}
