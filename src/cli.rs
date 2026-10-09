//! Parse and reject invalid requests before performing any OS operation.
#[derive(Debug, PartialEq)]
pub enum Command {
    Help,
    Version,
    Status {
        json: bool,
    },
    Schema {
        json: bool,
    },
    Watch {
        seconds: u64,
        interval_ms: u64,
        json: bool,
    },
    Bench {
        iterations: u64,
    },
    Simulate,
    Hardware {
        action: HardwareAction,
    },
    Lighting {
        config: crate::lighting::Lighting,
    },
    Power {
        plan: crate::settings::PowerPlan,
    },
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareAction {
    Status,
    Fan(crate::control::FanMode),
    Switch(crate::control::Switch, bool),
    PauseOem,
    ResumeOem,
}

pub const HELP: &str = "Minimalist Control Center 0.2\n\n\
Usage:\n  mcc status [--json]\n  mcc schema [--json]\n  mcc watch [--seconds 10] [--interval-ms 1000] [--json]\n  mcc bench [--iterations 1000000]\n  mcc simulate\n  mcc hardware status\n  mcc hardware fan-auto|fan-boost\n  mcc hardware key-lock-on|key-lock-off|usb-on|usb-off|light-bar-on|light-bar-off\n  mcc lighting off|solid|breathing|rainbow|wave|reactive BRIGHTNESS SPEED RRGGBB DIRECTION\n  mcc power balanced|saver|performance\n  mcc-ui.exe [--connect]\n\n\
schema: opt-in WMI metadata only, bounded to 10 seconds. No vendor methods run.\n\
watch: OS counters only; 1–3600 seconds, 250–60000 ms interval.\n\
bench/simulate: synthetic in-memory policy only; never control real fans.\n\
Hardware commands are explicit, fingerprint/capability/ownership gated.\n\
Fan modes are firmware automatic or boost. No fixed speeds, raw writes, drivers or voltage changes.\n\
Lighting brightness/speed: 0–4; direction: 1–4. Profiles never apply on startup.\n";

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some(name) = args.first().map(String::as_str) else {
        return Ok(Command::Help);
    };
    if matches!(
        name,
        "set" | "fan" | "boost" | "auto" | "control" | "write" | "install" | "service"
    ) {
        return Err(
            "Hardware control and installation are unavailable in this read-only release.".into(),
        );
    }
    match name {
        "gpu" if args.len() == 1 => Ok(Command::Gpu),
        "hardware" if args.len() == 2 => {
            use crate::control::{FanMode, Switch};
            let action = match args[1].as_str() {
                "pause-oem" => HardwareAction::PauseOem,
                "resume-oem" => HardwareAction::ResumeOem,
                "status" => HardwareAction::Status,
                "fan-auto" => HardwareAction::Fan(FanMode::Auto),
                "fan-boost" => HardwareAction::Fan(FanMode::Boost),
                "key-lock-on" => HardwareAction::Switch(Switch::WindowsKeyLock, true),
                "key-lock-off" => HardwareAction::Switch(Switch::WindowsKeyLock, false),
                "usb-on" => HardwareAction::Switch(Switch::UsbCharging, true),
                "usb-off" => HardwareAction::Switch(Switch::UsbCharging, false),
                "light-bar-on" => HardwareAction::Switch(Switch::LightBar, true),
                "light-bar-off" => HardwareAction::Switch(Switch::LightBar, false),
                _ => {
                    return Err("Unknown typed hardware action; raw values are not accepted".into());
                }
            };
            Ok(Command::Hardware { action })
        }
        "power" if args.len() == 2 => {
            use crate::settings::PowerPlan;
            let plan = match args[1].as_str() {
                "balanced" => PowerPlan::Balanced,
                "saver" => PowerPlan::PowerSaver,
                "performance" => PowerPlan::HighPerformance,
                _ => return Err("Choose balanced, saver or performance".into()),
            };
            Ok(Command::Power { plan })
        }
        "lighting" if args.len() == 6 => {
            use crate::lighting::{Effect, Lighting};
            let effect = match args[1].as_str() {
                "off" => Effect::Off,
                "solid" => Effect::Solid,
                "breathing" => Effect::Breathing,
                "rainbow" => Effect::Rainbow,
                "wave" => Effect::Wave,
                "reactive" => Effect::Reactive,
                _ => return Err("Unknown keyboard effect".into()),
            };
            let n = |i: usize| {
                args[i]
                    .parse::<u8>()
                    .map_err(|_| "Expected a bounded integer".to_owned())
            };
            let hex = &args[4];
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Color must be six hexadecimal digits, e.g. 46A0FF".into());
            }
            let rgb = [
                u8::from_str_radix(&hex[0..2], 16).unwrap(),
                u8::from_str_radix(&hex[2..4], 16).unwrap(),
                u8::from_str_radix(&hex[4..6], 16).unwrap(),
            ];
            let config = Lighting {
                effect,
                brightness: n(2)?,
                speed: n(3)?,
                rgb,
                direction: n(5)?,
            };
            crate::lighting::reports(&config)?;
            Ok(Command::Lighting { config })
        }
        "help" | "--help" | "-h" if args.len() == 1 => Ok(Command::Help),
        "--version" | "version" if args.len() == 1 => Ok(Command::Version),
        "status" | "schema" => {
            let json = match &args[1..] {
                [] => false,
                [v] if v == "--json" => true,
                _ => return Err("Only --json is accepted for this command.".into()),
            };
            Ok(if name == "status" {
                Command::Status { json }
            } else {
                Command::Schema { json }
            })
        }
        "simulate" if args.len() == 1 => Ok(Command::Simulate),
        "watch" | "bench" => {
            let mut seconds = None;
            let mut interval = None;
            let mut iterations = None;
            let mut json = false;
            let mut i = 1;
            while i < args.len() {
                let key = args[i].as_str();
                if key == "--json" && name == "watch" && !json {
                    json = true;
                    i += 1;
                    continue;
                }
                let slot = match (name, key) {
                    ("watch", "--seconds") => &mut seconds,
                    ("watch", "--interval-ms") => &mut interval,
                    ("bench", "--iterations") => &mut iterations,
                    _ => return Err(format!("Unknown or repeated option: {key}")),
                };
                if slot.is_some() {
                    return Err(format!("Repeated option: {key}"));
                }
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("Missing value for {key}"))?;
                if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(format!("Expected an unsigned integer for {key}"));
                }
                *slot = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| format!("Out-of-range value for {key}"))?,
                );
                i += 2;
            }
            if name == "bench" {
                let iterations = iterations.unwrap_or(1_000_000);
                if !(1_000..=10_000_000).contains(&iterations) {
                    return Err("Iterations must be 1000–10000000.".into());
                }
                Ok(Command::Bench { iterations })
            } else {
                let seconds = seconds.unwrap_or(10);
                let interval_ms = interval.unwrap_or(1000);
                if !(1..=3600).contains(&seconds) || !(250..=60_000).contains(&interval_ms) {
                    return Err("Seconds must be 1–3600; interval must be 250–60000 ms.".into());
                }
                Ok(Command::Watch {
                    seconds,
                    interval_ms,
                    json,
                })
            }
        }
        _ => Err(format!(
            "Unknown command or unexpected arguments: {name}. Use --help."
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(s: &str) -> Result<Command, String> {
        parse(&s.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
    }
    #[test]
    fn default_is_help_not_a_background_process() {
        assert_eq!(p("").unwrap(), Command::Help);
    }
    #[test]
    fn rejects_all_control_requests() {
        for s in [
            "set",
            "fan 0",
            "boost",
            "auto",
            "control",
            "write 1",
            "install",
            "service",
            "status --force",
        ] {
            assert!(p(s).is_err(), "{s}");
        }
    }
    #[test]
    fn rejects_zero_unbounded_and_overflow_limits() {
        for s in [
            "watch --seconds 0",
            "watch --seconds 3601",
            "watch --interval-ms 0",
            "watch --interval-ms 249",
            "watch --interval-ms 60001",
            "watch --seconds -1",
            "watch --seconds 18446744073709551616",
            "bench --iterations 0",
            "bench --iterations 10000001",
        ] {
            assert!(p(s).is_err(), "{s}");
        }
    }
    #[test]
    fn rejects_missing_duplicate_and_unknown_arguments() {
        for s in [
            "watch --seconds",
            "watch --seconds 1 --seconds 2",
            "watch --json --json",
            "bench --json",
            "schema --json --json",
            "simulate extra",
        ] {
            assert!(p(s).is_err(), "{s}");
        }
    }
    #[test]
    fn valid_watch_limits() {
        assert_eq!(
            p("watch --seconds 1 --interval-ms 250 --json").unwrap(),
            Command::Watch {
                seconds: 1,
                interval_ms: 250,
                json: true
            }
        );
    }
    #[test]
    fn typed_controls_never_accept_raw_or_force_options() {
        for s in [
            "hardware write 1873 0",
            "hardware fan-fixed",
            "hardware fan-auto --force",
            "power boost",
            "lighting solid 5 0 00FF00 1",
            "lighting solid 1 5 00FF00 1",
            "lighting solid 1 2 nope 1",
            "lighting solid 1 2 00FF00 0",
        ] {
            assert!(p(s).is_err(), "{s}");
        }
        assert_eq!(
            p("hardware fan-auto").unwrap(),
            Command::Hardware {
                action: HardwareAction::Fan(crate::control::FanMode::Auto)
            }
        );
        assert!(matches!(
            p("lighting solid 4 2 00FF00 1"),
            Ok(Command::Lighting { .. })
        ));
    }
}
