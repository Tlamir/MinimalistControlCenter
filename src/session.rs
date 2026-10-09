//! UI connection state. Errors never imply that a write did or did not happen.
use crate::control::{Capabilities, Switch};

#[derive(Default)]
pub struct Session {
    pub capabilities: Option<Capabilities>,
    pub allowed: bool,
    pub busy: bool,
}
impl Session {
    /// A narrow mode observation can revoke access, but cannot grant it or fabricate capabilities.
    pub fn observe_mode(&mut self, mode: u8, allowed: bool) {
        if let Some(caps) = self.capabilities.as_mut() {
            caps.fan_mode_raw = mode;
        }
        self.allowed &= allowed;
    }
    pub fn observe(&mut self, fresh: Option<(Capabilities, bool)>, status_failed: bool) {
        let busy = self.busy;
        self.complete(fresh, status_failed);
        self.busy = busy;
    }
    /// Only a fresh hardware status grants access. A failed status revokes it;
    /// unrelated power/lighting errors do not invent a disconnection.
    pub fn complete(&mut self, fresh: Option<(Capabilities, bool)>, status_failed: bool) {
        self.busy = false;
        if status_failed {
            self.allowed = false;
            self.capabilities = None;
        } else if let Some((caps, allowed)) = fresh {
            self.capabilities = Some(caps);
            self.allowed = allowed;
        }
    }
    pub fn ready(&self) -> bool {
        self.allowed && !self.busy
    }
    pub fn toggle_target(&self, switch: Switch) -> Option<bool> {
        if !self.ready() {
            return None;
        }
        let c = self.capabilities.as_ref()?;
        match switch {
            Switch::WindowsKeyLock if c.windows_key_lock => Some(!c.windows_key_locked),
            Switch::UsbCharging if c.usb_charging => Some(!c.usb_charging_enabled),
            Switch::LightBar if c.light_bar => Some(!c.light_bar_enabled),
            _ => None,
        }
    }
}
pub fn fan_label(byte: Option<u8>) -> &'static str {
    match byte {
        Some(0) => "Adaptive",
        Some(64) => "Turbo",
        Some(_) => "Firmware preset",
        None => "Not connected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn caps() -> Capabilities {
        Capabilities {
            fan_mode_raw: 0,
            boost: true,
            windows_key_lock: true,
            usb_charging: true,
            light_bar: true,
            windows_key_locked: false,
            usb_charging_enabled: false,
            light_bar_enabled: true,
        }
    }
    #[test]
    fn failed_toggle_with_fresh_status_does_not_need_reconnect() {
        let mut s = Session::default();
        s.complete(Some((caps(), true)), false);
        s.busy = true;
        s.complete(Some((caps(), true)), false);
        assert!(s.ready());
        assert_eq!(s.toggle_target(Switch::LightBar), Some(false));
    }
    #[test]
    fn lost_status_and_competing_controller_disable_writes() {
        let mut s = Session::default();
        s.complete(Some((caps(), true)), false);
        s.complete(None, true);
        assert!(!s.ready());
        assert_eq!(s.toggle_target(Switch::LightBar), None);
        s.complete(Some((caps(), false)), false);
        assert!(!s.ready());
        s.complete(Some((caps(), true)), false);
        assert!(s.ready());
    }
    #[test]
    fn unrelated_failure_keeps_connection_and_busy_prevents_duplicate_actions() {
        let mut s = Session::default();
        s.complete(Some((caps(), true)), false);
        s.busy = true;
        assert_eq!(s.toggle_target(Switch::LightBar), None);
        s.complete(None, false);
        assert!(s.ready());
    }
    #[test]
    fn labels_never_imply_auto_for_unknown_firmware_mode() {
        assert_eq!(fan_label(Some(128)), "Firmware preset");
        assert_eq!(fan_label(None), "Not connected");
    }
    #[test]
    fn physical_turbo_change_updates_state_without_clearing_a_queued_user_action() {
        let mut s = Session::default();
        let mut boosted = caps();
        boosted.fan_mode_raw = 64;
        s.complete(Some((boosted, true)), false);
        s.busy = true;
        s.observe(Some((caps(), true)), false);
        assert_eq!(s.capabilities.as_ref().unwrap().fan_mode_raw, 0);
        assert!(s.busy);
        assert!(!s.ready());
        s.observe(None, true);
        assert!(s.busy);
        assert!(!s.allowed);
        assert!(s.capabilities.is_none());
    }
    #[test]
    fn narrow_observation_never_grants_ownership_or_clears_pending_work() {
        let mut s = Session::default();
        s.observe_mode(64, true);
        assert!(!s.allowed && s.capabilities.is_none());
        s.complete(Some((caps(), true)), false);
        s.busy = true;
        s.observe_mode(64, true);
        assert!(s.busy && s.allowed);
        assert_eq!(s.capabilities.as_ref().unwrap().fan_mode_raw, 64);
        s.observe_mode(0, false);
        s.observe_mode(0, true);
        assert!(!s.allowed);
    }
}
