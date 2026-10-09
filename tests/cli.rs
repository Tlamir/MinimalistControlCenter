use std::{
    process::Command,
    time::{Duration, Instant},
};

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mcc"))
}

#[cfg(windows)]
#[test]
fn protected_helper_can_still_be_launched() {
    let _lease =
        mcc::process::lock_helper(std::path::Path::new(env!("CARGO_BIN_EXE_mcc"))).unwrap();
    let output = binary().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .starts_with("mcc ")
    );
}

#[test]
fn actual_binary_reports_read_only_and_unknown_temperatures() {
    let output = binary().args(["status", "--json"]).output().unwrap();
    assert!(output.status.success());
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["mode"], "read_only");
    assert_eq!(data["hardware_control_available"], false);
    for field in ["cpu_temperature_c", "gpu_temperature_c", "fan_rpm"] {
        assert!(data["snapshot"][field]["value"].is_null());
        assert!(data["snapshot"][field]["unavailable_reason"].is_string());
    }
    let text = String::from_utf8(output.stdout).unwrap();
    for private_field in ["serial_number", "computer_name", "user_name", "mac_address"] {
        assert!(!text.contains(private_field));
    }
}

#[test]
fn actual_binary_refuses_control_even_with_force() {
    for args in [
        vec!["fan", "0", "--force"],
        vec!["write", "1873", "0"],
        vec!["install"],
        vec!["status", "--enable-control"],
    ] {
        let output = binary().args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn actual_watch_is_bounded_and_uses_null_first_cpu_sample() {
    let started = Instant::now();
    let output = binary()
        .args(["watch", "--seconds", "1", "--interval-ms", "250", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    // Windows may scan a newly linked executable before its own bounded watch clock starts.
    assert!(started.elapsed() < Duration::from_secs(30));
    let text = String::from_utf8(output.stdout).unwrap();
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect();
    assert!(rows[0]["system_cpu_percent"].is_null());
    let summary = rows.last().unwrap();
    assert_eq!(summary["type"], "watch_summary");
    assert!((1..=4).contains(&summary["samples"].as_u64().unwrap()));
    assert_eq!(rows.len() as u64, summary["samples"].as_u64().unwrap() + 1);
    assert!(summary["wall_ms"].as_f64().unwrap() >= 1000.0);
    assert!(summary["wall_ms"].as_f64().unwrap() < 5000.0);
}

#[test]
fn simulation_cannot_be_mistaken_for_a_hardware_command() {
    let output = binary().arg("simulate").output().unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("NO hardware commands"));
    assert!(text.contains("emergency_cooling_required"));
    assert!(text.contains("firmware_automatic_required"));
}

#[test]
fn benchmark_explicitly_identifies_its_limited_scope() {
    let output = binary()
        .args(["bench", "--iterations", "1000"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(data["benchmark"], "offline_policy_only");
    assert_eq!(data["rounds"], 7);
    assert!(data["median_ns_per_evaluation"].as_f64().unwrap() > 0.0);
}

#[cfg(windows)]
#[test]
fn actual_native_ui_remains_responsive_during_a_slow_worker() {
    let path = std::env::temp_dir().join(format!("mcc-ui-test-{}.json", std::process::id()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_mcc-ui"))
        .args(["--self-test", path.to_str().unwrap()])
        .spawn()
        .unwrap();
    let start = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("Native UI failed to exit after its bounded self-test");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let _ = std::fs::remove_file(path);
    assert_eq!(report["ok"], true);
    assert_eq!(report["hardware_accessed"], false);
    assert_eq!(report["worker_responsive"], true);
    assert_eq!(report["pages_isolated_and_startup_writes_disabled"], true);
    assert!(report["ticks_during_worker_delay"].as_u64().unwrap() >= 3);
}
