use mcc::{
    cli::{self, Command},
    policy::{self, Sample},
};
use serde_json::json;
use std::{
    io::{self, Write},
    time::{Duration, Instant},
};

fn output(s: &str) -> Result<(), String> {
    let mut out = io::stdout().lock();
    match writeln!(out, "{s}") {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
fn pretty<T: serde::Serialize>(v: &T) -> Result<(), String> {
    output(&serde_json::to_string_pretty(v).map_err(|e| e.to_string())?)
}
fn label<T: std::fmt::Display>(v: &mcc::Reading<T>) -> String {
    v.value
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "unavailable".into())
}

fn execute(command: Command) -> Result<(), String> {
    match command {
        Command::Startup { enabled } => {
            #[cfg(windows)]
            {
                let result = match enabled {
                    Some(on) => mcc::startup::set(on),
                    None => mcc::startup::status(),
                };
                match result {
                    Ok(status) => pretty(
                        &json!({"ok":true,"startup":status,"scope":"current_user_logon","hardware_writes":0}),
                    ),
                    Err(e) => pretty(&json!({"ok":false,"error":e})),
                }
            }
            #[cfg(not(windows))]
            {
                let _ = enabled;
                Err("Startup configuration requires Windows".into())
            }
        }
        Command::Gpu => {
            #[cfg(windows)]
            {
                match mcc::gpu::sensors() {
                    Ok(s) => {
                        pretty(&json!({"ok":true,"gpu_temperature_c":s.temperature_c,"sensors":s}))
                    }
                    Err(e) => pretty(&json!({"ok":false,"error":e})),
                }
            }
            #[cfg(not(windows))]
            {
                Err("NVIDIA telemetry requires Windows".into())
            }
        }
        Command::Help => output(cli::HELP),
        Command::Version => output(concat!("mcc ", env!("CARGO_PKG_VERSION"))),
        Command::Diagnostics => pretty(&mcc::diagnostics::collect()),
        Command::Hardware { action } => hardware(action),
        Command::Lighting { config } => {
            #[cfg(windows)]
            {
                let result = mcc::hardware::keyboard(&config);
                pretty(&json!({"ok":result.is_ok(),"error":result.err()}))
            }
            #[cfg(not(windows))]
            {
                let _ = config;
                Err("Hardware controls require Windows".into())
            }
        }
        Command::Power { plan } => {
            #[cfg(windows)]
            {
                let result = mcc::desktop::power(plan);
                pretty(&json!({"ok":result.is_ok(),"error":result.err()}))
            }
            #[cfg(not(windows))]
            {
                let _ = plan;
                Err("Power controls require Windows".into())
            }
        }
        Command::Status { json } => {
            let r = mcc::report();
            if json {
                pretty(&r)
            } else {
                output(&format!(
                    "Minimalist Control Center — READ ONLY\nModel: {} {}\nBIOS: {}\nCPU: {}\nDisplay adapters: {}\nMemory used: {}%\nBattery: {}%\nCPU/GPU temperature and fan RPM: unavailable (not verified)\nHardware control: unavailable; no changes made",
                    label(&r.identity.manufacturer),
                    label(&r.identity.model),
                    label(&r.identity.bios_version),
                    label(&r.identity.cpu),
                    r.identity.display_adapters.join(", "),
                    label(&r.snapshot.memory_used_percent),
                    label(&r.snapshot.battery_percent)
                ))
            }
        }
        Command::Schema { json } => {
            let r = mcc::schema::discover()?;
            if json {
                pretty(&r)
            } else {
                output("WMI METADATA ONLY — hardware control unavailable")?;
                for class in r.metadata.classes {
                    output(&format!(
                        "{}: {}",
                        class.name,
                        if class.available {
                            "present"
                        } else {
                            "unavailable or access denied"
                        }
                    ))?;
                    for method in class.methods {
                        output(&format!(
                            "  {} ({})",
                            method.name,
                            method
                                .parameters
                                .iter()
                                .map(|p| format!("{}: {}", p.name, p.cim_type))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))?;
                    }
                }
                output(r.note)
            }
        }
        Command::Watch {
            seconds,
            interval_ms,
            json,
        } => watch(seconds, interval_ms, json),
        Command::Bench { iterations } => bench(iterations),
        Command::Simulate => {
            let mut simulator = policy::example();
            output("OFFLINE SIMULATION — fictional curve, unitless demand; NO hardware commands")?;
            for (ms, cpu, gpu) in [
                (0, 40., 40.),
                (1000, 70., 40.),
                (2000, 45., 75.),
                (3000, 91., 40.),
                (4000, 45., 40.),
                (10_000, 45., 40.),
            ] {
                pretty(
                    &json!({"synthetic_time_ms":ms,"cpu_c":cpu,"gpu_c":gpu,"decision":simulator.evaluate(ms,Some(Sample{celsius:cpu,captured_ms:ms}),Some(Sample{celsius:gpu,captured_ms:ms}))}),
                )?;
            }
            pretty(&json!({"missing_sensor_decision":simulator.evaluate(11_000,None,None)}))
        }
    }
}

fn hardware(action: cli::HardwareAction) -> Result<(), String> {
    #[cfg(windows)]
    {
        use cli::HardwareAction;
        match action {
            HardwareAction::ObserveLoop => mcc::hardware::observe_loop(),
            HardwareAction::Observe => match mcc::hardware::observe() {
                Ok(observation) => pretty(&json!({"ok":true,"observation":observation})),
                Err(e) => pretty(&json!({"ok":false,"error":e})),
            },
            HardwareAction::CalibrationProbe => match mcc::hardware::calibration_probe() {
                Ok(probe) => pretty(&json!({"ok":true,"probe":probe})),
                Err(e) => pretty(&json!({"ok":false,"error":e})),
            },
            HardwareAction::PauseOem | HardwareAction::ResumeOem => {
                let r = mcc::hardware::handover(matches!(action, HardwareAction::ResumeOem));
                pretty(&json!({"ok":r.is_ok(),"error":r.err()}))
            }
            HardwareAction::Status => match mcc::hardware::status() {
                Ok(s) => pretty(&json!({"ok":true,"status":s})),
                Err(e) => pretty(&json!({"ok":false,"error":e})),
            },
            HardwareAction::Fan(mode) => {
                let r = mcc::hardware::fan(mode);
                pretty(&json!({"ok":r.is_ok(),"error":r.err()}))
            }
            HardwareAction::Switch(s, on) => {
                let r = mcc::hardware::switch(s, on);
                pretty(&json!({"ok":r.is_ok(),"error":r.err()}))
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = action;
        Err("Hardware controls require Windows".into())
    }
}

fn watch(seconds: u64, interval_ms: u64, json_output: bool) -> Result<(), String> {
    let start = Instant::now();
    let end = Duration::from_secs(seconds);
    let interval = Duration::from_millis(interval_ms);
    let baseline = mcc::platform::metrics().value;
    let mut tick = Duration::ZERO;
    let mut previous = None;
    let mut count = 0u64;
    let mut max_working = 0u64;
    let mut max_private = 0u64;
    loop {
        let now = start.elapsed();
        if now >= end {
            break;
        }
        if now < tick {
            std::thread::sleep((tick - now).min(end - now));
            continue;
        }
        let snapshot = mcc::platform::snapshot();
        let current = snapshot.system_cpu_times.value;
        let usage = previous
            .zip(current)
            .and_then(|(a, b)| mcc::cpu_usage(a, b));
        previous = current;
        if let Some(p) = snapshot.process.value {
            max_working = max_working.max(p.working_set_bytes);
            max_private = max_private.max(p.private_commit_bytes);
        }
        count += 1;
        if json_output {
            output(&serde_json::to_string(&json!({"type":"sample","elapsed_ms":start.elapsed().as_millis(),"system_cpu_percent":usage,"snapshot":snapshot})).map_err(|e|e.to_string())?)?;
        } else {
            output(&format!(
                "{:>6.1}s  CPU {}  memory {}%  battery {}%  fan/temperature unavailable",
                start.elapsed().as_secs_f64(),
                usage
                    .map(|v| format!("{v:.1}%"))
                    .unwrap_or_else(|| "warming up".into()),
                label(&snapshot.memory_used_percent),
                label(&snapshot.battery_percent)
            ))?;
        }
        tick =
            mcc::next_tick(tick, start.elapsed(), interval).ok_or("Invalid scheduling interval")?;
    }
    let elapsed = start.elapsed();
    let final_metrics = mcc::platform::metrics().value;
    let cpu_ms = baseline
        .zip(final_metrics)
        .and_then(|(a, b)| b.cpu_time_100ns.checked_sub(a.cpu_time_100ns))
        .map(|v| v as f64 / 10_000.0);
    let summary = json!({"type":"watch_summary","mode":mcc::MODE,"samples":count,"wall_ms":elapsed.as_secs_f64()*1000.0,"process_cpu_ms":cpu_ms,"one_core_cpu_percent":cpu_ms.map(|ms|ms/(elapsed.as_secs_f64()*1000.0)*100.0),"max_sampled_working_set_bytes":max_working,"max_sampled_private_commit_bytes":max_private,"note":"Process CPU includes reporting. One-core percentage is not total-system percentage. Provider cost and GPU power are not measured. No thermal control is active."});
    if json_output {
        output(&summary.to_string())
    } else {
        pretty(&summary)
    }
}

fn bench(iterations: u64) -> Result<(), String> {
    let mut times = Vec::with_capacity(7);
    let mut checksum = 0u64;
    // Bounded CPU-only microbenchmark. No OS sensor or WMI access in the timed loop.
    for _ in 0..7 {
        let mut simulator = policy::example();
        let start = Instant::now();
        for i in 0..iterations {
            let sample = std::hint::black_box(Sample {
                celsius: 35.0 + (i % 550) as f64 / 10.0,
                captured_ms: i * 1000,
            });
            let decision =
                std::hint::black_box(simulator.evaluate(i * 1000, Some(sample), Some(sample)));
            checksum = checksum.wrapping_add(match decision {
                policy::Decision::CoolingDemand { unitless_percent } => u64::from(unitless_percent),
                policy::Decision::EmergencyCoolingRequired => 101,
                policy::Decision::FirmwareAutomaticRequired { .. } => 102,
            });
        }
        times.push(start.elapsed().as_nanos() as f64 / iterations as f64);
    }
    times.sort_by(f64::total_cmp);
    pretty(
        &json!({"schema_version":1,"benchmark":"offline_policy_only","iterations_per_round":iterations,"rounds":7,"median_ns_per_evaluation":times[3],"min_ns_per_evaluation":times[0],"max_ns_per_evaluation":times[6],"checksum":checksum,"debug_build":cfg!(debug_assertions),"note":"Synthetic CPU-only microbenchmark. Does not measure fan behavior, sensor latency, game FPS or improvement over OEM software."}),
    )
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if let Err(error) = cli::parse(&args).and_then(execute) {
        eprintln!("mcc: {error}");
        std::process::exit(2);
    }
}
