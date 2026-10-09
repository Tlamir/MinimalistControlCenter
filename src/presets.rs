//! Workload presets use only autonomous firmware cooling and standard Windows plans.
//! They are not fixed fan speeds or replacements for the unverified OEM Office tables.
use crate::{control::FanMode, settings::PowerPlan};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Office,
    Balanced,
    High,
    Turbo,
}
impl Preset {
    pub fn fan(self) -> FanMode {
        if self == Self::Turbo {
            FanMode::Boost
        } else {
            FanMode::Auto
        }
    }
    pub fn power(self) -> Option<PowerPlan> {
        match self {
            Self::Office => Some(PowerPlan::PowerSaver),
            Self::Balanced => Some(PowerPlan::Balanced),
            Self::High => Some(PowerPlan::HighPerformance),
            Self::Turbo => None,
        }
    }
    pub fn observed(mode: Option<u8>, power: Option<PowerPlan>) -> Option<Self> {
        match (mode, power) {
            (Some(64), _) => Some(Self::Turbo),
            (Some(0), Some(PowerPlan::PowerSaver)) => Some(Self::Office),
            (Some(0), Some(PowerPlan::Balanced)) => Some(Self::Balanced),
            (Some(0), Some(PowerPlan::HighPerformance)) => Some(Self::High),
            _ => None,
        }
    }
}
pub fn apply(
    preset: Preset,
    mut fan: impl FnMut(FanMode) -> Result<(), String>,
    mut power: impl FnMut(PowerPlan) -> Result<(), String>,
) -> Result<(), String> {
    fan(preset.fan())?;
    if let Some(plan) = preset.power() {
        power(plan).map_err(|e| {
            format!("Adaptive cooling confirmed, but Windows power plan failed: {e}")
        })?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_non_turbo_preset_restores_autonomous_cooling() {
        for p in [Preset::Office, Preset::Balanced, Preset::High] {
            assert_eq!(p.fan(), FanMode::Auto);
            assert!(p.power().is_some());
        }
        assert_eq!(Preset::Turbo.power(), None);
    }
    #[test]
    fn physical_button_and_external_power_changes_determine_selection() {
        assert_eq!(
            Preset::observed(Some(64), Some(PowerPlan::Balanced)),
            Some(Preset::Turbo)
        );
        assert_eq!(
            Preset::observed(Some(0), Some(PowerPlan::Balanced)),
            Some(Preset::Balanced)
        );
        assert_eq!(
            Preset::observed(Some(0), Some(PowerPlan::HighPerformance)),
            Some(Preset::High)
        );
        assert_eq!(
            Preset::observed(Some(128), Some(PowerPlan::PowerSaver)),
            None
        );
        assert_eq!(Preset::observed(None, Some(PowerPlan::Balanced)), None);
    }
    #[test]
    fn failed_fan_transition_never_changes_power_and_failed_power_is_explicit() {
        assert!(
            apply(
                Preset::Office,
                |_| Err("fan failed".into()),
                |_| panic!("power must not change")
            )
            .is_err()
        );
        let error = apply(
            Preset::High,
            |f| {
                assert_eq!(f, FanMode::Auto);
                Ok(())
            },
            |_| Err("unavailable".into()),
        )
        .unwrap_err();
        assert!(error.contains("Adaptive cooling confirmed"));
        assert!(error.contains("unavailable"));
        apply(
            Preset::Turbo,
            |f| {
                assert_eq!(f, FanMode::Boost);
                Ok(())
            },
            |_| panic!("Turbo preserves power plan"),
        )
        .unwrap();
    }
}
