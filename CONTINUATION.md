# Continuation — native v0.2, 9 October 2026

User wants a complete, genuinely lightweight OEM replacement, especially reliable fans. Latest instruction: finish within ten minutes or save an exact Markdown continuation. Safety and honest evidence outrank feature count. Full parity/extended thermal validation is UNFINISHED; do not call this checkpoint a completed replacement.

Repo: https://github.com/Tlamir/MinimalistControlCenter

## Implemented

Native Rust/Win32 GUI mcc-ui.exe + mcc.exe helper. Real auto/boost fan control, reversible OEM launcher handover, Windows power plans, finite HID effects, strict saved profiles, convenience switches, opt-in GPU temperature, screen off and GPU-settings link. Visible OS counters update at 1 Hz; hidden dashboard polling stops. Startup applies nothing. No new driver, OEM DLL, voltage/PL write, low-speed override or startup modification.

Run binaries together. Exit OEM GUI; Connect as admin; Take control if its launcher service runs. Auto/boost and profiles require fresh capability/ownership checks. Restore OEM requires automatic mode first. A UI helper timeout can leave a submitted action applied; reconnect.

## Verified hardware facts

- MONSTER / TULPAR T5 V17.1 / BIOS N.1.06 / i7-8750H / GTX1060 + UHD630. GK5CN6Z chassis and EC revision still user/reseller identified.
- root\\WMI, AcpiTest_MULong, ACPI\\PNP0C14\\1_1, GetSetULong: UInt64 Data, UInt32 Return.
- Read: (1 << 40) | address. Write: (value << 16) | address.
- Read returns a WORD, requested byte in LOW 8 bits. Real regression values: 33760 -> 224 (support1), 27648 -> 0 (fan mode), 15362 -> 2 (support5). OEM casts and community code corroborate.
- Support1=224, support2=131, support5=2, newer rule byte1922=0; MyFan2 defaults1929..1933 all zero. Do not invent percent scaling/curve limits.
- Admin native reads succeeded. Initial UAC canceled; owner explicitly approved retry.
- NVAPI temperature38°C verified. Candidate EC1086=48 and speed6148=0 are UNCALIBRATED.
- GamingCenter service: C:\Program Files\OEM\Monster Kontrol Merkezi\LaunchServGM.exe, OwnProcess, automatic startup. Native85392-byte launcher, not .NET; static imports include service APIs/CreateProcessAsUserW and strings GamingCenter.exe/ShowGamingCenter.exe. No vendor code loaded for execution.
- First fan test made zero writes because service was active after UI closed.
- Later actual test temporarily paused service; AUTO->BOOST64; persistence5s; AUTO0 confirmed; restarted service. Owner confirmed audible fan acceleration and return. private/fan-validation-handover.json records success. OEM service was restored.
- Owner reports a physical turbo button. App does not continuously fight it, BUT autonomous operation with OEM paused is not yet proven.

## Finish next

1. Check pushed head, CI and final benchmark hashes; do not redo passing checks without new changes.
2. Test physical turbo with OEM paused. If software must handle its WMI event, trace exact event semantics; distinguish request vs firmware-state notification; fresh reads, no periodic overrides.
3. Independently calibrate CPU sensor/RPM scaling/freshness against trusted readings. No unverified CPU Celsius/RPM UI.
4. Physically validate ITE048d:ce00 lighting (present in local HID registry), usage1, pagesff02/ff12. Solid ff02: mode51 + picture12 + eight65-byte output packets. Four-zone solid: firmware1. Other enabled firmware effects finite9-byte reports. No nonvolatile saves. Reactive explicitly blocked until key-event routing; per-key editor still absent.
5. Test Win-key lock, USB charging, light bar, power-plan selection, screen-off/wake and restoration physically.
6. Power transitions, sleep/resume, cold boot, termination and sustained thermal/noise workload tests.
7. Custom quiet curves ONLY after firmware family, safe limits, calibrated thermals and failover. Newer defaults zero. No fan-off/fixed low speed. Current Quiet is Windows Power Saver + firmware auto.
8. Complete display color presets, original OSD handling and per-key editor through validated APIs.
9. OEM resource baseline and game frame times before promising cooling/FPS improvements. No million-fold or zero-impact claim.

## Local environment

Workspace C:\Users\yigit\Documents\GitHub\MinimalistControlCenter.
Ignored .tools/rustup/toolchains/stable-x86_64-pc-windows-gnullvm/bin: Rust1.99. Direct tools work; rustup shims had sandbox path issues. scripts/build-local.ps1 sets local Cargo/Rustup/Rustc/PATH and absolute import-library flags. scripts/local_import_libraries.py generates .tools/windows-imports from installed Windows export metadata, without loading implementation. Windows/MSVC CI uses the SDK.

UI --self-test OUTPUT.json creates real widgets and proves event processing during a synthetic500ms worker delay; no hardware. --benchmark SECONDS OUTPUT.json / --benchmark-hidden SECONDS OUTPUT.json are bounded1..3600. Hidden benchmark adds a measurement timer; ordinary hidden mode has none.

Before packaging: format, strict Clippy, debug/release tests, both PE audits. Record hashes and actual counts/results in docs/BENCHMARKS.md.

.tools/gh/bin/gh.exe authenticated Tlamir with workflow scope. Use per-command gh credential helper, no global credential changes. User authorized public push/checkpoints. No subagents requested.

private/: OEM static decompilation, probes, raw results/audits, metadata. reports/local/: benchmarks. Never publish proprietary implementation, drivers, credentials or user inventories. Source notices regenerate via scripts/dependency-notices.py from locked cargo metadata. Include Rust runtime notices with binaries.

