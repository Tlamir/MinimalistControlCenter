# Minimalist Control Center

Native Rust/Win32 controls for **MONSTER TULPAR T5 V17.1 / BIOS N.1.06 / i7-8750H / GTX 1060**. No browser runtime, proprietary DLL, new driver or continuously running fan algorithm.

**v0.3 simplifies everyday use:** Cooling, Lighting and Settings pages; two prominent fan choices; one control-enabling action; stateful switches and a native color palette. Full feature parity and extended thermal validation remain unfinished.

## Run

Keep **mcc-ui.exe** and **mcc.exe** together. Open mcc-ui.exe. Startup reads OS counters and applies nothing.

1. Exit Monster Kontrol Merkezi, including its tray icon.
2. Click **Enable controls** and accept the Windows administrator prompt. The app reads capabilities and pauses the reviewed OEM launcher if needed. Its startup configuration stays unchanged. If the original controller is using a different preset, set automatic cooling there before exiting it.
3. In **Cooling**, choose **Automatic** for firmware-managed cooling or **Cooling boost** for faster, potentially louder fans. The highlighted choice is the last confirmed mode.
4. **Quiet** selects automatic fans and Windows Power Saver together. **Balanced** and **Performance** select Windows power plans without changing fans. Quiet reduces available performance; it is not an FPS optimization.
5. In **Lighting**, choose effect, brightness and color, then **Apply keyboard**. The light-bar switch applies immediately. Successful keyboard submissions save preferences, which never apply on startup.
6. **Settings** contains Windows-key lock, hibernation USB charging, screen-off and opt-in GPU tools.

Closing the window hides it to the tray and stops dashboard polling. **Exit app** in Settings or the tray menu exits. Firmware modes remain active when the app exits. **Use original controller** restores automatic fans and the existing OEM launcher.

The physical turbo button is not periodically overridden. **Refresh**, F5, or reopening the tray window rereads its mode. Its behavior with the OEM launcher paused still needs testing.

## Reliability

Light-bar and Windows-key commands wait for pending toggle pulses to be consumed, re-read current state, submit one pulse and poll acknowledgement with a bounded read loop. A delayed success is never blindly toggled back. After failed hardware actions, the GUI refreshes actual state: successful readback keeps controls usable; unavailable state disables writes. Failed writes are never automatically replayed.

The failure/retry flow was exercised in the actual UI using simulated hardware. Real light-bar timing still needs a physical retest on this laptop; its owner previously reported ON working and OFF succeeding after reconnecting.

Hardware helpers run outside the UI thread, with five-second deadlines and bounded output. A timed-out write may already have taken effect. The fresh read checks that state; **Enable controls** retries connection if verification failed. These are unsigned Windows x64 development builds.

## Does it need Monster Kontrol Merkezi?

The executables use the Windows ACPI/WMI interface and supported keyboard HID device directly. They do not load proprietary DLLs for controls. An absent OEM launcher is supported. Similar folder or feature names do not imply shared implementation.

Keep the original installed as a recovery option during validation. Uninstalling it has **not** been tested on this laptop; preservation of the Windows firmware interface after uninstall must be checked. Uncalibrated sensors and remaining features prevent claiming full replacement parity. No software can guarantee that hardware never fails.

## Features and evidence

| Feature | Status |
|---|---|
| Native dashboard / tray | CPU usage, memory, battery, AC; visible 1 Hz OS updates, no hidden dashboard polling |
| Automatic / boost fans | Previous five-second AUTO → BOOST → AUTO hardware test passed, with owner-observed acceleration and return |
| OEM handover | Existing exact-path user-mode launcher temporarily stopped/restored in that test |
| Power plans / saved lighting | Standard Windows plans; Quiet also exits boost; bounded preferences, no startup application |
| Keyboard | Off, solid, breathing, rainbow, wave; brightness, speed, direction, colors; finite HID transfers, no flash-save or host animation; physical validation pending |
| Win-key lock / USB charging / light bar | Capability-gated desired-state commands and acknowledgement checks; physical retest pending |
| Display / GPU | Screen-off, per-app GPU preference link, opt-in NVAPI temperature; previous real read returned 38°C |
| CPU temperature / fan RPM / custom curves | Uncalibrated; no invented readings or unsafe low-speed curve |
| Reactive / per-key editor / OSD / display color presets | Remaining parity work |

## CLI

```powershell
.\mcc.exe status --json                 # OS counters only
.\mcc.exe hardware status              # explicit firmware reads
.\mcc.exe hardware fan-auto
.\mcc.exe hardware fan-boost
.\mcc.exe gpu                          # may wake the discrete GPU
.\mcc.exe power saver                  # CLI changes power plan only
.\mcc.exe lighting rainbow 3 2 46A0FF 1
.\mcc.exe watch --seconds 10 --json
```

No raw-address, fan-off, fixed-speed, force, driver-install or firmware-flash entry point exists. The simulator's fictional policy never reaches hardware. **--demo** and **--demo-failure** UI flags simulate every hardware action and display a preview label.

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

Tests cover real native page isolation, disabled startup writes, responsiveness during delayed work, late acknowledgements and pending pulses, recovery without write replay, missing OEM services, subprocess hangs/overflow, malformed settings, synthetic sensor faults, payload encoding and unrelated-bit preservation.

See [safety](docs/SAFETY.md), [protocol evidence](docs/PROTOCOL.md), [benchmarks](docs/BENCHMARKS.md), and [remaining work](CONTINUATION.md). No million-fold speedup, zero impact, cooling improvement or FPS gain is claimed without measurements.

Layout research: [G-Helper](https://github.com/seerge/g-helper) and [Fan Control](https://github.com/Rem0o/FanControl.Releases). Their code is not incorporated, and their hardware compatibility does not establish compatibility with this laptop.

Independent of Monster/Tongfang. Original implementation MIT. OEM code, drivers, tools and raw reports are not redistributed; dependency and Rust-runtime notices accompany binaries.
