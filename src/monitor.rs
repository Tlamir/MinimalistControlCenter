//! Read-only refresh scheduling. Never restores a preferred fan mode over a physical button.
#[derive(Default)]
pub struct Monitor {
    pub enabled: bool,
    pending: bool,
    next_ms: u64,
}
impl Monitor {
    pub fn due(&mut self, now_ms: u64, busy: bool) -> bool {
        if !self.enabled || self.pending || busy || now_ms < self.next_ms {
            return false;
        }
        self.pending = true;
        true
    }
    pub fn completed(&mut self, now_ms: u64, visible: bool, failed: bool) {
        self.pending = false;
        self.next_ms = now_ms.saturating_add(if failed {
            30_000
        } else if visible {
            2_000
        } else {
            15_000
        });
    }
    pub fn wake(&mut self) {
        self.next_ms = 0;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_startup_access_no_overlapping_or_busy_refreshes() {
        let mut m = Monitor::default();
        assert!(!m.due(100, false));
        m.enabled = true;
        assert!(!m.due(100, true));
        assert!(m.due(100, false));
        assert!(!m.due(50_000, false));
    }
    #[test]
    fn hidden_polling_backs_off_and_failure_retries_without_enabling_writes() {
        let mut m = Monitor {
            enabled: true,
            ..Default::default()
        };
        m.completed(0, true, false);
        assert!(!m.due(1999, false));
        assert!(m.due(2000, false));
        m.completed(2000, false, false);
        assert!(!m.due(16999, false));
        assert!(m.due(17000, false));
        m.completed(17000, true, true);
        assert!(!m.due(46999, false));
        assert!(m.due(47000, false));
        m.completed(47000, false, false);
        m.wake();
        assert!(m.due(47001, false));
    }
}
