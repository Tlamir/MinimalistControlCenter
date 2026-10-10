# Missing features and known issues

Updated 10 October 2026 against v0.7.1. Code delivery and physical acceptance are tracked separately.

## v0.7 delivery

- Dynamic laptop header/title, embedded original Windows/README icon, direct preset buttons and shorter interface copy.
- Visible-only dashboard and firmware polling; on-demand GPU clock, temperature and supported tachometer readings, with stale samples cleared.
- Office remains Auto + Power Saver. OEM minimum-speed and custom-table modes are unverified and remain blocked.

## v0.6 delivery

- Redesigned native UI: global Hide/Exit, grouped fan/power controls, compact presets, explicit lighting-send feedback, and full status/error Details. Three native page renders verified; owner acceptance pending.
- Protected Program Files installer with file hashes, preference migration, desktop shortcut, Installed Apps uninstall, and startup rollback on installation failure.
- Current administrator user's elevated interactive logon task; one-time installation approval, no stored password or repeated sign-in prompt. Starts on AC/battery and requests light-bar off after sign-in.
- Startup controls use bounded helpers with task/real-registry verification. Portable startup remains monitor only. No continuous startup metadata polling.
- Exact reviewed OEM launcher startup is backed up/disabled to avoid competing controllers and restored on uninstall. Other OSD/touchpad entries are preserved.
- Startup write failures report a tray warning and fresh status; no write replay. Additional fan levels remain gated by the documented [fan assessment](FAN_SUPPORT.md).
- Real installation, installed hash/permission checks, startup off/on and unelevated desktop/task launch passed in v0.6.2. Fresh firmware readback reports light bar off. The owner confirmed real restart/startup and physical light-bar-off. Physical keyboard acceptance, battery sign-in and end-to-end uninstall remain separate owner checks.

## v0.5 implemented foundation

- Implemented separate Adaptive/Turbo fan buttons and Power Saver/Balanced/Performance buttons; each preserves the other setting.
- Kept regular Office/Balanced/High/Turbo combinations in a presets menu.
- Added Save current as Custom / Apply saved Custom for a supported fan + Windows plan combination. This is not a custom fan curve.
- v0.5.1 identifies this laptop as four-zone (ff12) and updates all four Solid color slots. Physical acceptance is pending; the keyboard defect below is not yet closed. The unrelated per-key row candidate was reverted.

## Priority 1 — reported problems and requested everyday behavior

- [ ] **Fix incomplete keyboard lighting.** Owner reports that changing lighting changes only one side of the keyboard. Full-keyboard control is not working correctly. The four-zone Solid palette omission was identified and corrected in v0.5.1; physical acceptance of that correction remains pending. Verify whole-keyboard coverage for every supported effect, color, brightness and off command before calling lighting complete.
- [x] **Physically verify light bar off after restart.** Owner confirmed this on v0.6.2. Before-Windows/boot lighting persistence is a separate, unverified firmware capability.
- [ ] **Verify reliable light-bar ON → OFF switching.** Earlier owner report: ON worked, but OFF required reconnecting. Acknowledgement/recovery logic was changed, but a conclusive physical retest is still missing.
- [x] **Handle startup settings from packaged launch contexts in code.** Startup now uses Task Scheduler or the real Windows registry provider through a bounded helper, rather than native reads/writes of a private redirected hive. Provider failure remains visible instead of appearing Off.
- [x] **Complete a real restart test.** Owner confirmed automatic startup and light-bar-off after a normal restart on v0.6.2. Battery sign-in is a separate check.
- [x] **Verify installed privileged launch.** Protected demand launch, ordinary-user run permission and startup off/on passed. Controls are ready and firmware reports light bar off. The real sign-in event remains covered by the separate pending restart check.
- [ ] **Finish usability polish and owner acceptance.** Confirm clear fan/power wording, lighting feedback, tray discovery, close-versus-exit behavior and readable errors. The latest working connection does not establish that the overall UI meets the owner's expectations.

## Priority 2 — actual cooling controls and trustworthy telemetry

- [ ] **Minimum-noise cooling mode.** Not available as a verified fan-speed mode.
- [ ] **Real OEM-style Office/adaptive profiles and additional cooling levels.** Current Office, Balanced and High cards combine firmware Auto with Windows power plans. They are not independently calibrated fan-speed profiles or the original OEM Office table.
- [ ] **Manual fan adjustment/custom curves.** No hardware-backed editor or low/fixed-speed control is available. A policy simulator exists, but it does not control the laptop.
- [ ] **Verified overtemperature override.** Demonstrate that dangerously low user settings cannot defeat cooling, including process failure, missing telemetry and OS failure. Mock tests cannot establish autonomous firmware protection. Do not enable reduced cooling before this is proven.
- [ ] **Calibrated CPU temperature and fan RPM.** Current raw candidates are not trustworthy displayed measurements. Identify CPU/GPU fan mapping and scaling; establish whether independent fan control is supported.
- [ ] **Complete sensor integration.** GPU temperature is opt-in and has had a successful real query; reliable combined temperature/RPM display, stale-data handling and battery-friendly sampling still need complete hardware validation.
- [ ] **Sustained thermal/noise/performance validation.** Measure representative idle, office and gaming loads against the original controller, including temperature, noise, throttling, frame-time impact, total background CPU and battery cost. Existing UI-only benchmarks do not prove better cooling or gaming performance.
- [ ] **Lifecycle/failure validation.** Test sleep/resume, AC/battery changes, controller conflicts, helper timeouts, app crashes and recovery. Verify actual firmware behavior, not just simulated policy outputs.

## Priority 3 — lighting and original control-center parity

- [ ] **Per-key/zone lighting editor and mapping.** Verify hardware capability; do not infer full support from the original app's keyboard drawing.
- [ ] **Reactive lighting.** Not implemented as a verified feature; avoid a permanent CPU-heavy animation loop.
- [ ] **Lighting startup modes and profile behavior.** Define and verify which effects/settings can be restored after sign-in, sleep and restart. Saved preferences currently do not establish hardware persistence.
- [ ] **Display color presets.** Original Standard/Gaming/Video/Reading/Customize behavior is not reproduced.
- [ ] **On-screen display and hotkey parity.** Fn-key OSD, its disable setting and remaining OEM hotkey behavior need implementation/validation. Physical turbo-button synchronization is already confirmed separately.
- [ ] **Discrete-GPU preference parity.** The Windows per-app GPU settings shortcut is not equivalent to the OEM's global discrete-GPU preference. Establish what the firmware feature actually does; do not imply a display MUX exists.
- [ ] **Convenience-feature hardware acceptance.** Windows-key lock and hibernation USB charging have implemented commands but still need full physical verification, including persistence and resume behavior.
- [ ] **Secondary dashboard parity.** Network throughput and disk/SSD information from the reference dashboard are not fully reproduced. Add only useful, inexpensive monitoring.

## Delivery and independence

- [ ] **Verify installation/update/uninstall on the target laptop.** Protected versioned installation, shortcut migration, uninstall registration, startup cleanup/rollback and package verification are implemented. Builds remain unsigned prereleases; real installer/uninstall acceptance is tracked separately.
- [ ] **Verify operation after OEM removal/disablement.** Code supports an absent OEM service and does not load OEM DLLs, but a complete real uninstall/reboot/hotkey test has not been performed. Do not claim full replacement yet.
- [ ] **Verify original OEM startup handover.** Installer now backs up/disables only the exact reviewed launcher entry and restores it on uninstall. Already-running GUIs must be exited; touchpad/OSD startup is preserved. Startup passed the owner restart test; uninstall restoration is untested.

## Confirmed working — do not reopen as missing without new evidence

- Administrator handoff/Enable controls: owner confirmed working in v0.4.2 after the physical-path fix.
- Firmware Automatic and Turbo/Boost: short physical fan response was confirmed previously.
- Physical turbo button: owner confirmed that the app's mode display updates correctly in v0.4.2.
- Real Windows startup entry: independently verified for the installed v0.4.2 path; v0.6.2 installer/task toggles and owner reboot test now pass.
- Native UI, bounded helper execution, automated tests and measured UI resource use exist. They do not establish full feature parity or a guarantee against hardware failure.
