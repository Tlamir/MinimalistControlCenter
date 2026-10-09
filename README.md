# Minimalist Control Center

Native Rust + Win32 controls for **MONSTER TULPAR T5 V17.1 / BIOS N.1.06 / i7-8750H / GTX 1060**. No browser runtime, proprietary DLL, new driver, or continuously running fan algorithm.

**v0.2 controls real automatic/boost modes.** The target laptop passed a five-second AUTO → BOOST → AUTO test, and its owner confirmed the audible response and return. Full feature parity and extended thermal validation remain unfinished. No software can guarantee that hardware never fails.

## Run

Keep mcc-ui.exe and mcc.exe together. Launch **mcc-ui.exe**. Startup reads OS counters and applies nothing.

1. Exit Monster Kontrol Merkezi, including its tray icon.
2. Click **Connect as admin** and accept UAC to read capabilities.
3. Click **Take control** if the OEM launcher service is active. Its startup configuration stays unchanged. Close any OEM window it already launched.
4. Select **Automatic fans**, **Cooling boost**, or a profile.
5. Quiet uses Windows Power Saver with automatic fans. This reduces available performance; it is not an FPS optimization.

Closing the window hides it to the tray and stops dashboard polling. Quit exits. Modes run in firmware without the app. To return to the original app, select Automatic fans, Restore OEM controller, then Quit.

The physical turbo button is not periodically overridden. Refresh hardware to reread its mode. Its behavior with the OEM service paused still needs testing.

## Features

| Feature | Status |
|---|---|
| Native dashboard / tray | CPU usage, memory, battery, AC; 1 Hz visible updates, no hidden dashboard polling |
| Automatic / boost fans | Real hardware test, mode readback, owner-observed response and return |
| OEM handover | Existing exact-path user-mode launcher temporarily stopped and restored in the test |
| Power plans / profiles | Standard Windows plans, three presets, saved lighting; strict bounded JSON, no startup application |
| Keyboard | Off, solid, breathing, rainbow, wave; brightness, speed, direction, RGB; finite HID transfers, no flash-save or host animation; physical validation pending |
| Win-key lock / USB charging / light bar | Capability-gated desired-state commands with readback; physical switch validation pending |
| Display / GPU | Screen off, Windows per-app GPU preference link, opt-in NVAPI temperature; actual read returned 38°C |
| CPU temperature / fan RPM / custom curves | Uncalibrated; no invented readings or unsafe low-speed curve |
| Reactive / per-key editor / OSD / display color presets | Remaining parity work |

The GUI runs hardware helpers outside its UI thread with five-second deadlines and bounded output. A timed-out write may already have taken effect: reconnect and check the mode. Keep the original app installed for recovery. Builds are unsigned Windows x64 development builds.

## CLI

```powershell
.\mcc.exe status --json                 # OS counters only
.\mcc.exe hardware status              # explicit firmware reads
.\mcc.exe hardware fan-auto
.\mcc.exe hardware fan-boost
.\mcc.exe gpu                          # may wake the discrete GPU
.\mcc.exe power saver
.\mcc.exe lighting rainbow 3 2 46A0FF 1
.\mcc.exe watch --seconds 10 --json
```

No raw-address, fan-off, fixed-speed, force, driver-install or firmware-flash entry point exists. The simulator's fictional policy never reaches hardware.

## Build / verify

Stable Rust; normal Windows/MSVC builds need Visual Studio C++ build tools. Linux tests the portable core and mocks, without a hardware backend.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo test --all-targets --release --locked
cargo build --release --locked
python scripts/audit_binary.py target/release/mcc.exe
python scripts/audit_binary.py target/release/mcc-ui.exe
```

Local SDK-less GNU/LLVM builds use ignored project-local tools and generated OS import metadata: scripts/local_import_libraries.py, then scripts/build-local.ps1 -Release -Test. No global PATH changes.

Tests include actual native UI responsiveness during a delayed worker, subprocess hangs/overflow, malformed settings, missing/stale synthetic sensors, payload encoding, actual probe word decoding, readback/recovery failures, idempotence and unrelated-bit preservation.

See [safety](docs/SAFETY.md), [protocol evidence](docs/PROTOCOL.md), [benchmarks](docs/BENCHMARKS.md), and [unfinished work / exact continuation](CONTINUATION.md). No million-fold speedup, zero impact, cooling improvement or FPS gain is claimed without measurements.

Independent of Monster/Tongfang. Original implementation MIT. OEM code, drivers, tools and raw reports are not redistributed; dependency notices accompany binaries.

