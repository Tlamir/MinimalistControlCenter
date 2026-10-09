# Minimalist Control Center

A small Rust diagnostic foundation for a future Tongfang/Monster laptop control center. Development target: Monster Tulpar T5 V17.1, reported GK5CN6Z, i7-8750H, GTX 1060.

**v0.1 is read-only. It does not control fans or replace the OEM cooling software.** Fan, power and driver-write implementations are absent, not hidden behind a setting. Keep your existing cooling controller installed.

## What works

- Native Windows identity, memory, battery, AC state and CPU-usage counters.
- Bounded monitoring with explicit unavailable sensor states and JSON output.
- Opt-in discovery of Tongfang WMI class metadata, with a deadline and output limits.
- An offline thermal-policy simulator with hysteresis, delayed decreases, immediate increases and invalid-sensor handling.
- Automated fault tests, executable integration tests, binary import inspection and repeatable benchmarks.

CPU/GPU temperature and fan RPM are intentionally unavailable until their sensor paths are validated. No GUI, background service, RGB control or hardware fan curves are shipped yet. Simulated cooling demand is not a hardware setting.

## Run

Windows x64; no administrator rights needed for normal diagnostics. Downloaded binaries are unsigned development builds and Windows may flag them accordingly.

```powershell
.\mcc.exe status
.\mcc.exe status --json
.\mcc.exe watch --seconds 10 --interval-ms 1000
.\mcc.exe schema --json
.\mcc.exe simulate
```

`status` reads only selected registry strings and documented OS counters. `watch` samples OS counters only and exits automatically. `schema` uses the Windows system copy of PowerShell once to inspect class definitions; it never invokes the listed vendor methods. If access is unavailable, it reports that rather than attempting to alter permissions.

There are no `fan`, `write`, `boost`, `install` or `--force` commands. No setting enables them.

## Build and verify

Use stable Rust (minimum source language version 1.87; the lockfile and CI use current stable). Standard Windows MSVC builds need the Visual Studio C++ build tools. A pure Rust GNU/LLVM build can use the bundled linker and `rust-mingw` component; `.cargo/config.toml` supplies that target's settings.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo test --all-targets --release --locked
cargo build --release --locked
python scripts/audit_binary.py target/release/mcc.exe
```

Linux CI runs parsing, policy, process and integration tests with Windows counters marked unavailable. Linux is not a supported hardware backend.

## Measure

```powershell
.\target\release\mcc.exe bench --iterations 1000000
.\scripts\benchmark.ps1 -Seconds 600
```

The benchmark script records 30 warm process launches after three discarded warmups, seven offline-policy rounds and a low-rate monitoring session. Reports default to ignored `reports/local/`. The policy loop makes no hardware calls. Read [benchmark results and limitations](docs/BENCHMARKS.md) before interpreting the numbers. No OEM speedup or FPS improvement is claimed.

## Safety and development

[Safety boundary](docs/SAFETY.md) describes what the current executable can and cannot do. [Protocol findings](docs/PROTOCOL.md) explain why static register discovery is insufficient to enable fan control. [The implementation plan](PLAN.md) records the longer-term scope and hardware validation gates.

The first hardware investigation confirmed that the local WMI metadata contains `AcpiTest_MULong.GetSetULong`, with `Data: UInt64` and `Return: UInt32`. **Metadata presence does not establish safe fan control.** Static inspection also found model-dependent fan scaling and power-limit side effects. No EC transactions have been attempted by this project.

OEM binaries, decompiled code, downloaded tools and raw local reports are excluded from the repository. This project is independent of Monster and Tongfang. MIT-licensed original implementation; vendor software is not redistributed.
