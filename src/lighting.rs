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
/// Mode feature followed by six explicitly addressed rows of 21 LEDs.
/// The reviewed first-generation OEM row protocol uses planar B, G, R channels.
pub struct SolidTransfer {
    pub effect: [u8; 9],
    pub row_select: [[u8; 9]; 6],
    pub pixels: [u8; 65],
}
pub fn solid_output(c: &Lighting) -> Result<SolidTransfer, String> {
    reports(c)?;
    if c.effect != Effect::Solid {
        return Err("Solid transfer requires the solid effect".into());
    }
    let mut pixels = [0u8; 65];
    pixels[2..23].fill(c.rgb[2]);
    pixels[23..44].fill(c.rgb[1]);
    pixels[44..65].fill(c.rgb[0]);
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
        row_select: std::array::from_fn(|row| [0, 22, 0, row as u8, 0, 0, 0, 0, 0]),
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_addresses_every_row_and_all_126_leds_without_flash_save() {
        let transfer = solid_output(&Lighting {
            rgb: [19, 47, 203],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(transfer.effect[3], 51);
        assert_eq!(transfer.effect[8], 0);
        for (row, select) in transfer.row_select.iter().enumerate() {
            assert_eq!(*select, [0, 22, 0, row as u8, 0, 0, 0, 0, 0]);
        }
        assert_eq!(&transfer.pixels[..2], &[0, 0]);
        assert_eq!(&transfer.pixels[2..23], &[203; 21]);
        assert_eq!(&transfer.pixels[23..44], &[47; 21]);
        assert_eq!(&transfer.pixels[44..65], &[19; 21]);
        assert_eq!(transfer.row_select.len() * 21, 126);
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
