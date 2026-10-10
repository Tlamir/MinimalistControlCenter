# Active v0.4.2 correction — 10 October 2026

Owner's screenshot overrides the earlier mistaken Controls ready reply: v0.4.1 failed with Windows error 3. Root cause identified by GetFinalPathNameByHandle: apparent AppData/Local/MinimalistControlCenter install was redirected into the Codex package LocalCache/Local folder. Shell runas against the apparent path failed, while the same bytes in the build folder elevated successfully. Exiting the UI did not fix it, ruling out the directory lease hypothesis; retained original lease behavior.

Fix: resolve current executable through canonicalize (GetFinalPathNameByHandle) before ShellExecuteEx, remove only local-drive extended prefix while retaining UTF-16, supply physical parent working directory. Startup registration also uses the physical path. Installer now uses %USERPROFILE%/Applications/MinimalistControlCenter/<version>, outside the redirected AppData directory. Error dialog preserves Windows error codes; only 1223 is cancellation. COM initialization and synchronous launch retained.

Build target remains target/v041 for v0.4.2. 95 release tests, strict Clippy and PE audits pass. Installed in C:/Users/yigit/Applications/MinimalistControlCenter/0.4.2 and startup readback passes. Owner confirmed Enable controls and physical turbo-button/UI synchronization both work. GitHub source fix pushed as 4a035df; v0.4.2 publication is being finalized. Exact old v0.4.1 installed bytes elevated successfully through their physical package-cache path (read-only self-test), confirming the path-redirection diagnosis. Automatic light-bar-off on restart remains requested and unfinished. Older notes below are historical, not proof of a working v0.4.1 handoff.

---

# Active v0.4.1 work — 10 October 2026

Owner reported Enable controls says administrator access canceled. v0.4.0 collapsed every ShellExecute failure into that message, so actual cause is not established. Changed to ShellExecuteExW with COM initialization, synchronous shell completion, Unicode-native executable path, exact GetLastError diagnostics and a full error dialog. Do not claim the UAC problem is resolved until owner verifies the installed build.

Owner also requires the light bar to be off after Windows restart. NOT implemented yet. Current HKCU tray startup is deliberately unelevated and cannot write ACPI. A protected, narrowly scoped privileged startup component and persistent desired lighting state are needed for automatic sign-in application; do not create a highest-privilege scheduled task pointing to user-writable LocalAppData binaries. Before-Windows lighting persistence is not established. No firmware nonvolatile writes are authorized by an inferred protocol.

InstallServGM.exe is present in the original OEM installation and is not installed or called by our app. Original GamingCenter service remains Automatic; observed Stopped. No matching scheduled task/startup command was found in the scoped read-only checks. Exact OEM installer startup trigger remains unknown.

Build target for this patch: target/v041. v0.4.0 public prerelease is already published. Pending installation, owner UAC test, publishing and persistent lighting work. Older checkpoint follows.

---

# v0.4 continuation — 10 October 2026

Latest owner request: real minimum-noise/Office/Balanced/High/Turbo/custom fan controls with thermal override, Windows startup, and UI synchronization after the physical turbo button. Owner explicitly prefers doing visual/physical checks themselves; do not spend tokens on computer-use inspection.

## Implemented in this update

- Four clear workload cards: Office = firmware Auto + Windows Power Saver; Balanced = Auto + Balanced; High = Auto + High Performance; Turbo = firmware Boost, preserving the power plan. These are **not** the OEM Office table or calibrated speed levels. Removed misleading independent Quiet power-plan label.
- Read-only fan-mode/ownership observation after explicit connection, with 2-second visible / 15-second hidden delays plus timer/helper latency. Read failures discard stale state and back off 30 seconds. Narrow observation cannot grant access; full status is used for recovery. Pending user actions remain pending when a background result arrives. No periodic fan writes, no fighting the physical button. Restore OEM stops monitoring.
- Start with Windows: fixed current-user Run entry, quoted installed mcc-ui.exe path and --tray. No automatic privilege escalation or profile application. User still clicks Enable controls after login. Installer scripts/install-user.ps1 copies a versioned release into LocalAppData, verifies hashes, then registers startup. Reboot remains an owner check.
- One real UI per privilege level; normal-to-admin handoff remains explicit. Old v0.3 uses a different class and must be exited by the owner. Explorer tray recreation supported. A file/directory lease prevents ordinary helper replacement while the elevated UI can launch it; tests cover blocked replacement and allowed helper execution.
- Raw calibration-probe reads only a fixed list of known candidate addresses. It is NOT a Celsius/RPM calibration and never grants manual fan-write access.

## Real read-only evidence this session

Both administrator probes succeeded with zero hardware writes. Fan mode 0, ownership available; generation1922=0, support1858=2; active MyFan2 entries1859..1863 all60; defaults1929..1933 all0; candidate1086 varied69/67; candidate6148 remained0 despite operating fans. No valid RPM source established. Full status samples took about1.16s; optimized one-mode observation about0.51s. Helper CPU counters are quantized and omit WMI provider CPU; don't claim whole-system performance from them.

## Still unfinished — do not claim complete OEM replacement

Minimum-noise/fixed/custom fan speeds remain locked. OEM static mapping: fixed128..133 and table160 at1873, old table1859..1863; generation bit3 at1922. There is an application-side over-temperature event path. No proof of autonomous EC thermal override under low custom cooling or after process/OS failure. Independent CPU/GPU sensor correlation, RPM/scaling, autonomous thermal recovery and failure behavior are required before enabling reduced cooling. The owner has not yet answered the HWiNFO/readings question. Do not force the laptop to100°C to discover limits.

Physical turbo display synchronization and light-bar ON/OFF need owner observation; automated tests are not that evidence. See docs/OWNER_CHECKS.md. No new real fan/lighting/power writes were made in this update. Startup registration is the intended non-hardware mutation.

Local build now uses CARGO_TARGET_DIR=target/v04 because an old target/release/mcc-ui.exe was still running. Cargo fmt/clippy need unsandboxed project access in this session; normal sandbox returns filesystem access denied. GitHub credentials work outside network-restricted sandbox; do not ask for re-login based on sandbox auth failure.

Final local checks: 93 release tests, strict Clippy/fmt, both PE audits pass. Installed in C:\Users\yigit\AppData\Local\MinimalistControlCenter\0.4.0; startup enable/off/off/on readback and actual tray/duplicate-instance smoke tests passed, leaving startup enabled. Test UI processes were stopped; the owner can launch the installed UI. See docs/BENCHMARKS.md and ignored reports/local/v04-delivery.md for publication status. Earlier v0.3 history follows.

---

# Continuation — native v0.3, 9 October 2026

v0.3 checkpoint: public portable preview at https://github.com/Tlamir/MinimalistControlCenter/releases/tag/v0.3.0. Local tests: 69 library + 4 native worker + 6 integration = 79 passing. Strict Clippy passes; binary audits and measurements are recorded in docs/BENCHMARKS.md. All previously pending v0.2 CI runs completed successfully. Check the v0.3 CI outcome when continuing.

User wants a complete, lightweight OEM replacement, especially reliable fans. Latest request: simplify the ugly/confusing UI, fix light-bar OFF requiring reconnect, preserve performance, and clarify OEM removal. This update addresses those UI/error-flow issues. Full parity and extended thermal validation are UNFINISHED; do not call this a completed replacement.

Repo: https://github.com/Tlamir/MinimalistControlCenter

## Implemented

Native Rust/Win32 GUI mcc-ui.exe + mcc.exe helper. Real auto/boost fan control, reversible OEM launcher handover, Windows power plans, finite HID effects, strict saved profiles, convenience switches, opt-in GPU temperature, screen off and GPU-settings link. Visible OS counters update at 1 Hz; hidden dashboard polling stops. Startup applies nothing. No new driver, OEM DLL, voltage/PL write, low-speed override or startup modification.

Run binaries together. Exit OEM GUI/tray; click Enable controls (UAC + reviewed launcher handover). Cooling has Auto/Boost and three Windows power choices. Quiet explicitly exits boost. Lighting has native selectors, swatches, Apply keyboard and one light-bar switch. Settings contains other switches and restoration. Use original controller selects auto before restarting the launcher. Startup applies no settings.

Switch commands wait for earlier toggle pulses, re-read desired/current state, send one pulse and poll acknowledgement. Pending or late-success requests are not replayed. The worker reads fresh status even after failure and preserves controls when that read succeeds. Four injected worker tests verify this routing; real UI preview reproduced an acknowledgement failure followed by successful OFF without reconnect. Native self-test checks all three pages, hidden animation controls and disabled startup writes. Physical light-bar timing still needs retesting.

No proprietary DLL/executable is needed for direct controls, and missing OEM service is supported by a portable test. The actual OEM uninstall has not been performed; verify provider availability after any future uninstall. Keep the original installed/paused for recovery meanwhile. Similar folders are not shared implementation.

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

1. Check pushed v0.3 head, CI and final benchmark hashes; do not redo passing checks without new changes.
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

UI --self-test OUTPUT.json creates real widgets, checks page isolation and disabled startup controls, and proves event processing during a synthetic500ms worker delay; no hardware. --demo / --demo-failure use an isolated simulated backend for all hardware actions. --benchmark SECONDS OUTPUT.json / --benchmark-hidden SECONDS OUTPUT.json are bounded1..3600. Hidden benchmark adds a measurement timer; ordinary hidden mode has none.

Before packaging: format, strict Clippy, debug/release tests, both PE audits. Record hashes and actual counts/results in docs/BENCHMARKS.md.

.tools/gh/bin/gh.exe authenticated Tlamir with workflow scope. Use per-command gh credential helper, no global credential changes. User authorized public push/checkpoints. No subagents requested.

private/: OEM static decompilation, probes, raw results/audits, metadata. reports/local/: benchmarks. Never publish proprietary implementation, drivers, credentials or user inventories. Source notices regenerate via scripts/dependency-notices.py from locked cargo metadata. Include Rust runtime notices with binaries.
