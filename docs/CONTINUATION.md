# v0.7.1 installed and checked

Final protected0.7.1 installation passed. Startup off, demand launch with logon disabled, startup on, delayed bounded read-only firmware verification: writes_allowed=true, owner=null, fan mode0/Auto, light_bar_enabled=false. Startup left enabled.104 release tests, strict Clippy, fifteen provider tests, PE import audits, package hash/CRC and Explorer icon extraction passed. The post-install documentation correction changes README/CONTINUATION only; binaries and privileged scripts remain byte-identical. The package includes16 finite hashed files and linked flat docs plus sanitized benchmark JSON. Benchmark identifies0.7.0 accurately; final0.7.1 only clears stale GPU UI state when hidden. Owner reboot/light-bar confirmation pertains to0.6.2; new build readback passed, no forced restart or OEM removal. Remaining: physical keyboard effects, minimum fan/curve protection and calibrated CPU/EC RPM, other model validation, battery/resume/uninstall checks. No Git commit, push or publication.

# v0.7.1 final correction

v0.7.0 installed successfully, but its immediate3second firmware verification probe collided with startup hardware ownership. No write was replayed. Final read-only verification must wait for startup and retry only the status read if ownership remains busy. Source v0.7.1 fixes stale GPU caption after hiding a monitor-only window and discards GPU replies received while hidden; show refreshes control captions. Performance table explicitly identifies measured v0.7.0 and its hashes. Idle polling behavior is unchanged. Use a fresh protected0.7.1 directory, never overwrite0.7.0. Final installer/check results follow.

# v0.7 work — 10 October 2026

Owner confirmed v0.6.2 real restart, automatic startup and physical light-bar-off. Latest request: less UI copy, visible Office preset, dynamic laptop name, real icon, cheaper telemetry, performance pass then measured OEM comparison in README. v0.7 implements direct preset buttons, concise status, dynamic native window/header, original ICO/README SVG and on-demand GPU clock/temp/tach. Real NVAPI sample returned354MHz/42C, fan unknown; OS CPU frequency800MHz. GPU samples expire30sec/hide; no continuous polling. Hidden dashboard and firmware polling stop; current state refreshes on open; per-write ownership guards retained. Visible ownership queries filter only matching process names/running services. Preset power changes no longer launch an extra CLI. UI snapshot skips unused self-process counters.

Windows x64 icon resource generated in build.rs with std only; no SDK/compiler/build dependency. GNU/LLVM release, strict Clippy and104 release tests passed. Fake-provider tests15 scenarios passed. Offscreen native previews checked. OEM minimum fan mode is not safe merely because encodable: fixed/table/power-limit writes still blocked. Other OEM-sharing models are unverified; dynamic naming does not broaden allowlist. Keep original installed; full uninstall/parity untested.

Serial comparison completed: native visible16.2MiB working set/2.6MiB private/0.411-0.436% whole-PC CPU; OEM visible175.8MiB/127.1MiB/0.004-0.017%. Native tray0-15.625msCPU per30sec and zero helpers, not an absolute zero-impact claim. Eight successful30sec intervals; raw failed attempts excluded. OEM main window correctly identified after an initial hidden SQL CE window mismatch. Launcher service denied job assignment; counted separately with process CPU/memory. Normal OEM Exit, Auto, bar-off and old installed task restoration succeeded. README and docs/BENCHMARKS include methodology/tradeoffs; sanitized report reports/benchmarks/v0.7.0-2026-10-10.json. Original kept installed; uninstall still untested. Icon extraction verifies Explorer uses the embedded original resource. Prepared installer includes16 finite hashed files, flat linked docs/report and explicit desktop shortcut icon. Latest final native previews checked. Installer validation follows separately. Historical notes below preserve earlier release evidence.

# v0.6.2 installed and verified — 10 October 2026

Owner authorized retry. Final v0.6.2 installed under Program Files/current-SID/0.6.2 and opened through its desktop shortcut. All eight installed payload hashes match the verified ZIP. Protected folders are Administrators-owned with ordinary read/execute access; task owner/DACL preserves read/run-only access for the current SID. Exactly one UI instance is running. Installed Apps metadata and shortcut target/arguments match v0.6.2.

Actual elevated installation tests passed: startup off, demand launch while the logon trigger was disabled, startup on, and fresh firmware status. Final readback: writes_allowed=true, ownership_reason=null, fan mode0 (Auto), light_bar_enabled=false. Startup remains enabled. An independent unelevated caller (CallerIsAdmin=false) successfully ran the protected task without UAC; installed CLI reports enabled=true, managed=true. Old Minimalist Run and exact Monster launcher Run entries are absent by EnumValues verification. OEM startup backup exists; OSD/touchpad entries remain. A real restart and physical keyboard/light-bar observation are still owner acceptance items; no stress test or fan-curve experiment occurred.

Two Windows integration quirks were fixed before final delivery: Task Scheduler returns account names for registered SIDs, requiring SID resolution; GetStringValue returns1 for a missing value on this host, so presence is established by EnumValues before reading/deleting. Actual v0.6.0 task-verification failure rolled back and preserved old startup. v0.6.1 installed successfully, then was superseded in a fresh folder. Versioned files were never overwritten. The local verification helper initially failed to find the running window because PowerShell converted a null string title to empty; corrected its P/Invoke title to IntPtr.Zero and exited only the known v0.6.1 app through its normal Exit path. This helper is not part of the distributed package.

104 release tests, strict Clippy/fmt, fifteen Windows fake-provider/source-helper scenarios, parser checks, both PE audits, and final package hashes/CRC passed. Final ZIP: target/packages/mcc-0.6.2-windows-x64.zip; SHA256 b369e5d0741b60cf68f5f8122dd676c7c6150998a1247d43e694f479add45fd4. Built using the local toolchain with target/v060 as the build cache. Latest UI layout was visually checked via native offscreen renders. No Git commit, push or publication was performed. Custom fan curves remain locked pending independent thermal failover proof. Uninstall has fixture coverage for registry normalization but no end-to-end uninstall run yet. Historical notes below describe earlier attempts and releases.

---

# v0.6 delivery — 10 October 2026

User requests: finish the project; keep it minimal and performant; installer, Windows startup without repeated UAC, light bar off at sign-in, and investigate safer fan curves. Latest owner feedback rejects the previous UI and asks for researched UX improvements. G-Helper and Microsoft command-bar guidance were reviewed. The native UI now has global Hide/Exit, full-width navigation, clear Automatic/Boost versus independent Windows performance, compact presets, explicit keyboard-send feedback, and complete status/error Details. Offscreen native renders of all three pages were visually checked; no desktop screen capture or hardware access was used.

v0.6 implements protected versioned Program Files installation, fixed interactive elevated logon/demand task, exact OEM-launcher backup/restore, desktop shortcut and Installed Apps entry. One installer UAC prompt; desktop launch still works when automatic logon is switched off. Startup requests only light-bar-off, with fresh readback and no write replay. No saved fan, power or keyboard preset is automatically recalled. Handing over a running OEM service may restore Auto first. Startup metadata uses bounded out-of-process Windows providers; timeout kills helper descendants. Custom low fan curves remain locked: OEM software-side over-temperature handling does not establish autonomous EC protection after an app/OS failure. See docs/FAN_SUPPORT.md.

104 release tests, strict Clippy/fmt, nine PowerShell fake-provider scenarios, parser checks and both PE audits passed. The release ZIP is generated by scripts/package.py with a finite allowlist and verified hashes. Private OEM source, reports and toolchains are excluded. CI includes Windows provider checks and packaging.

Installation attempt reached Windows UAC, which reported cancellation (Windows error1223). No v0.6 installed files/task were created. Asked owner whether to retry or keep the installer ready; retry requires their answer. Do not claim installation, cold boot or physical lighting acceptance. No Git commit, push or publication in this turn. Historical entries below describe earlier versions.

---

# v0.5.1 active correction

Read-only Win32_PnPEntity established this laptop is ITE048d:ce00 REV0002, usageff12/1 (FOUR ZONE). The v0.5.0 row candidate targeted ff02 and could not fix this laptop; reverted it. Actual root cause: generic Solid populated only palette slot1. OEM FourZone Single constructs four colors, circular=true, then writes slots1..4 and selects palette8. Implemented this exact bounded non-flash protocol for ff12 Solid only, retaining other variants/effects. Tests cover all zones, selector8, Off and variant isolation. Physical test pending.

v0.5.0 was installed but not publicly released. User now has one Desktop/Minimalist Control Center.lnk, and version is shown in app. Native controls separation/regular preset menu/saved Custom combination implemented and tested. Installer now verifies real startup via StdRegProv and preserves prior profiles. Asked owner to exit app so shortcut can be updated to v0.5.1. Build target target/v041; final tests running. Do not claim lighting fixed physically until confirmed.

---

# Active v0.5 work — 10 October 2026

Latest user resumed implementation and requested separate fan/performance controls while keeping regular presets and custom choices. This supersedes the earlier documentation-only request.

Implemented: independent Adaptive/Turbo fan controls and Power Saver/Balanced/Performance plans; original workload presets retained in a menu; Save current as Custom and Apply saved Custom reuse validated settings schema and never automatically apply at startup. Applying Custom changes fan + power only, preserving lighting. Power-only jobs never request firmware status.

Lighting: compared the existing picture stream with original local OEM protocol. Added explicit row0..5 selection (feature0x16), each with 21 planar B/G/R values, for Solid on usagepageff02 only. Tests verify all126 LED positions and zero nonvolatile save byte. No firmware flash writes. Physical fix is not yet confirmed. Asked owner which effect/side fails and to exit older app before installation; replies pending.

Build target remains target/v041; Cargo package v0.5.0. 100 release tests, strict Clippy/fmt and both binary audits pass. Installed at C:/Users/yigit/Applications/MinimalistControlCenter/0.5.0, opened for owner verification. Desktop shortcut C:/Users/yigit/Desktop/Minimalist Control Center.lnk points to it. Installer uses StdRegProv for real startup registration and bounded copying of previous profiles.json; Win32_StartupCommand confirms the v0.5.0 path. Owner confirmed the original lighting symptom affects Solid colors on the left only, and could not locate any tray app; process inventory showed none running. Asked owner to test Solid now; result pending. Publication pending. Other missing features remain in docs/MISSING_FEATURES.md, especially verified low/custom cooling, startup preferences/light-bar-off, and package-context startup toggle handling.

---

# Verified v0.4.2 delivery — 10 October 2026

Latest owner request: documentation only; do not implement or run hardware tests in this turn. See [Missing features and known issues](MISSING_FEATURES.md). New reported defect: lighting changes only one side of the keyboard; cause and affected zones/effects are not established.

Installed: C:/Users/yigit/Applications/MinimalistControlCenter/0.4.2. Owner explicitly confirmed Enable controls succeeds and physical turbo changes synchronize with UI. 95 release tests, strict Clippy/fmt, both PE audits, Windows+Ubuntu CI for code commit4a035df pass. Source includes physical path resolution for runas/startup and installer outside redirected AppData.

Startup follow-up: even owner's off/on and requested Win+R CLI step did not appear in Win32_StartupCommand. Native HKCU and explicit HKU readbacks from packaged tool context were insufficient. Used Windows StdRegProv SetStringValue for ONLY current-user Run/MinimalistControlCenter = quoted installed mcc-ui.exe plus --tray. Independent StdRegProv readback and Win32_StartupCommand now BOTH confirm the exact real entry. Real restart not performed. Startup toggle when launched from a packaged process can still target a private hive: needs future automatic handling; do not infer this is fixed in code. No OEM startup entries removed.

Remaining: automatic light-bar-off after restart (owner request); reliable privileged startup/preferences architecture; minimum-noise/custom cooling blocked pending calibrated sensors and verified autonomous thermal failover. No new real fan/lighting writes in this fix except owner's own Turbo/button test. Owner prefers to do visual checks; avoid computer-use.

GitHub code fix4a035df and owner confirmationa7c13c9 pushed. v0.4.2 release being finalized; prior chronological notes follow.

---

# Active v0.4.2 correction — 10 October 2026

Owner's screenshot overrides the earlier mistaken Controls ready reply: v0.4.1 failed with Windows error 3. Root cause identified by GetFinalPathNameByHandle: apparent AppData/Local/MinimalistControlCenter install was redirected into the Codex package LocalCache/Local folder. Shell runas against the apparent path failed, while the same bytes in the build folder elevated successfully. Exiting the UI did not fix it, ruling out the directory lease hypothesis; retained original lease behavior.

Fix: resolve current executable through canonicalize (GetFinalPathNameByHandle) before ShellExecuteEx, remove only local-drive extended prefix while retaining UTF-16, supply physical parent working directory. Startup registration also uses the physical path. Installer now uses %USERPROFILE%/Applications/MinimalistControlCenter/<version>, outside the redirected AppData directory. Error dialog preserves Windows error codes; only 1223 is cancellation. COM initialization and synchronous launch retained.

Build target remains target/v041 for v0.4.2. 95 release tests, strict Clippy and PE audits pass. Installed in C:/Users/yigit/Applications/MinimalistControlCenter/0.4.2 and startup readback passes. Owner confirmed Enable controls and physical turbo-button/UI synchronization both work. GitHub source fix pushed as 4a035df; v0.4.2 publication is being finalized. Exact old v0.4.1 installed bytes elevated successfully through their physical package-cache path (read-only self-test), confirming the path-redirection diagnosis. Automatic light-bar-off on restart remains requested and unfinished. Older notes below are historical, not proof of a working v0.4.1 handoff.

Additional startup issue: packaged-process HKCU readback shows the new Applications path, but Win32_StartupCommand omits it (provider returns six other entries). Owner toggling startup from the UAC-launched app did not make it visible. Asked owner to run installed mcc.exe startup on via Win+R, which originates outside the package context. Await independent provider confirmation. Do not call startup validated based only on same-context readback. README now calls out normal Windows PowerShell installation. CI on code commit 4a035df passed Windows and Ubuntu.

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
