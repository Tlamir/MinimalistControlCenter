//! ITE firmware effects: finite reports, no host animation thread, no flash/save writes.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Off,
    Solid,
    Breathing,
    Rainbow,
    Wave,
    Reactive,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lighting {
    pub effect: Effect,
    pub brightness: u8,
    pub speed: u8,
    pub rgb: [u8; 3],
    pub direction: u8,
}
impl Default for Lighting {
    fn default() -> Self {
        Self {
            effect: Effect::Solid,
            brightness: 3,
            speed: 2,
            rgb: [70, 160, 255],
            direction: 1,
        }
    }
}
pub fn reports(c: &Lighting) -> Result<Vec<[u8; 9]>, String> {
    if c.effect == Effect::Reactive {
        return Err(
            "Reactive effects need verified key-event routing and are not enabled in this build"
                .into(),
        );
    }
    if c.brightness > 4 || c.speed > 4 || !(1..=4).contains(&c.direction) {
        return Err("Brightness and speed must be 0–4, direction 1–4".into());
    }
    if c.effect == Effect::Off {
        return Ok(vec![[0, 8, 1, 0, 0, 0, 0, 0, 0]]);
    }
    let effect = match c.effect {
        Effect::Off => unreachable!(),
        Effect::Solid => 1,
        Effect::Breathing => 2,
        Effect::Rainbow => 5,
        Effect::Wave => 3,
        Effect::Reactive => 4,
    };
    let [r, g, b] = c.rgb;
    Ok(vec![
        [0, 20, 0, 1, r, g, b, 0, 0],
        [
            0,
            8,
            2,
            effect,
            [10, 7, 5, 3, 1][c.speed as usize],
            [0, 8, 22, 36, 50][c.brightness as usize],
            1,
            c.direction,
            0,
        ],
    ])
}
/// Four-zone firmware needs all four palette slots and multi-color selection,
/// even when the requested result is one uniform color. Per-key devices retain
/// their separate protocol; never send per-key rows to a four-zone collection.
pub fn reports_for_collection(c: &Lighting, usage_page: u16) -> Result<Vec<[u8; 9]>, String> {
    if !matches!(usage_page, 0xff02 | 0xff12) {
        return Err("Unsupported keyboard collection".into());
    }
    let mut base = reports(c)?;
    if usage_page == 0xff12 && c.effect == Effect::Solid {
        let [r, g, b] = c.rgb;
        let mut all: Vec<_> = (1..=4)
            .map(|zone| [0, 20, 0, zone, r, g, b, 0, 0])
            .collect();
        let mut effect = base.pop().ok_or("Missing effect report")?;
        effect[6] = 8;
        all.push(effect);
        Ok(all)
    } else {
        Ok(base)
    }
}

/// Mode feature, transfer-start feature, then one sixteen-key output packet.
pub struct SolidTransfer {
    pub effect: [u8; 9],
    pub picture: [u8; 9],
    pub pixels: [u8; 65],
}
pub fn solid_output(c: &Lighting) -> Result<SolidTransfer, String> {
    reports(c)?;
    if c.effect != Effect::Solid {
        return Err("Solid transfer requires the solid effect".into());
    }
    let mut pixels = [0u8; 65];
    for slot in pixels[1..].chunks_exact_mut(4) {
        slot[1..].copy_from_slice(&c.rgb);
    }
    Ok(SolidTransfer {
        effect: [
            0,
            8,
            2,
            51,
            0,
            [0, 8, 22, 36, 50][c.brightness as usize],
            0,
            0,
            0,
        ],
        picture: [0, 18, 0, 0, 8, 0, 0, 0, 0],
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn four_zone_solid_updates_every_zone_and_selects_zone_palette_without_flash_save() {
        let c = Lighting {
            rgb: [19, 47, 203],
            ..Default::default()
        };
        let output = reports_for_collection(&c, 0xff12).unwrap();
        assert_eq!(output.len(), 5);
        for (i, r) in output[..4].iter().enumerate() {
            assert_eq!(*r, [0, 20, 0, (i + 1) as u8, 19, 47, 203, 0, 0]);
        }
        assert_eq!(output[4][1..4], [8, 2, 1]);
        assert_eq!(output[4][6], 8);
        assert_eq!(output[4][8], 0);
        assert_eq!(
            reports_for_collection(&c, 0xff02).unwrap(),
            reports(&c).unwrap()
        );
        assert!(reports_for_collection(&c, 0xff03).is_err());
        let off = Lighting {
            effect: Effect::Off,
            ..c
        };
        assert_eq!(
            reports_for_collection(&off, 0xff12).unwrap(),
            reports(&off).unwrap()
        );
    }
    #[test]
    fn off_is_one_firmware_report() {
        let c = Lighting {
            effect: Effect::Off,
            ..Default::default()
        };
        assert_eq!(reports(&c).unwrap(), [[0, 8, 1, 0, 0, 0, 0, 0, 0]]);
    }
    #[test]
    fn no_report_writes_nonvolatile_save_byte() {
        for e in [
            Effect::Off,
            Effect::Solid,
            Effect::Breathing,
            Effect::Rainbow,
            Effect::Wave,
        ] {
            let c = Lighting {
                effect: e,
                ..Default::default()
            };
            for r in reports(&c).unwrap() {
                assert_eq!(r[0], 0);
                assert_eq!(r[8], 0);
                assert!(matches!(r[1], 8 | 20));
            }
        }
    }
    #[test]
    fn invalid_parameters_are_rejected_before_io() {
        for c in [
            Lighting {
                brightness: 5,
                ..Default::default()
            },
            Lighting {
                speed: 5,
                ..Default::default()
            },
            Lighting {
                direction: 5,
                ..Default::default()
            },
        ] {
            assert!(reports(&c).is_err());
        }
    }
    #[test]
    fn color_encoding_is_exact() {
        let c = Lighting {
            rgb: [2, 45, 200],
            ..Default::default()
        };
        assert_eq!(reports(&c).unwrap()[0], [0, 20, 0, 1, 2, 45, 200, 0, 0]);
    }
    #[test]
    fn reactive_is_not_sent_without_verified_key_events() {
        assert!(
            reports(&Lighting {
                effect: Effect::Reactive,
                ..Default::default()
            })
            .is_err()
        );
    }
}
