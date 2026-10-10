use crate::{control::FanMode, lighting::Lighting};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerPlan {
    Balanced,
    PowerSaver,
    HighPerformance,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub fan: FanMode,
    pub power: PowerPlan,
    pub lighting: Lighting,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub version: u32,
    pub profiles: Vec<Profile>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            profiles: vec![
                Profile {
                    name: "Quiet".into(),
                    fan: FanMode::Auto,
                    power: PowerPlan::PowerSaver,
                    lighting: Lighting::default(),
                },
                Profile {
                    name: "Balanced".into(),
                    fan: FanMode::Auto,
                    power: PowerPlan::Balanced,
                    lighting: Lighting::default(),
                },
                Profile {
                    name: "Cooling boost".into(),
                    fan: FanMode::Boost,
                    power: PowerPlan::Balanced,
                    lighting: Lighting::default(),
                },
            ],
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || !(1..=8).contains(&self.profiles.len()) {
            return Err("Unsupported settings version or profile count".into());
        }
        for (i, p) in self.profiles.iter().enumerate() {
            if p.name.is_empty()
                || p.name.chars().count() > 32
                || p.name.chars().any(char::is_control)
                || self.profiles[..i].iter().any(|o| o.name == p.name)
            {
                return Err("Profile names must be unique, printable and 1–32 characters".into());
            }
            crate::lighting::reports(&p.lighting)?;
        }
        Ok(())
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 16 * 1024 {
            return Err("Settings file exceeds 16 KiB".into());
        }
        let s: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        use std::io::Read;
        let mut bytes = vec![];
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Self::parse(&bytes)
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        use std::io::Write;
        self.validate()?;
        // A failed write cannot truncate the original. create_new refuses stale/symlink temp files.
        let temp = path.with_extension("json.pending");
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            f.write_all(&serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
            drop(f);
            std::fs::rename(&temp, path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_use_autonomous_fan_modes() {
        let s = Settings::default();
        s.validate().unwrap();
        assert_eq!(s.profiles[0].fan, FanMode::Auto);
        assert_eq!(s.profiles[0].power, PowerPlan::PowerSaver);
    }
    #[test]
    fn reject_unknown_fields_and_invalid_enum() {
        for b in [br#"{"version":1,"profiles":[],"write_register":123}"#.as_slice(),br#"{"version":1,"profiles":[{"name":"x","fan":"fixed_zero","power":"balanced","lighting":{}}]}"#.as_slice()] { assert!(Settings::parse(b).is_err()); }
    }
    #[test]
    fn rejects_large_files_and_duplicate_profiles() {
        assert!(Settings::parse(&vec![0; 16385]).is_err());
        let mut s = Settings::default();
        s.profiles.push(s.profiles[0].clone());
        assert!(s.validate().is_err());
    }
    #[test]
    fn profile_roundtrip() {
        let s = Settings::default();
        let p = Settings::parse(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(p.profiles[0].name, "Quiet");
    }
    #[test]
    fn custom_combinations_roundtrip_without_replacing_regular_profiles() {
        let mut settings = Settings::default();
        settings.profiles.push(Profile {
            name: "Custom".into(),
            fan: FanMode::Boost,
            power: PowerPlan::PowerSaver,
            lighting: Lighting::default(),
        });
        let restored = Settings::parse(&serde_json::to_vec(&settings).unwrap()).unwrap();
        assert_eq!(restored.profiles.len(), 4);
        assert_eq!(restored.profiles[1].name, "Balanced");
        assert_eq!(restored.profiles[3].fan, FanMode::Boost);
        assert_eq!(restored.profiles[3].power, PowerPlan::PowerSaver);
    }
}
