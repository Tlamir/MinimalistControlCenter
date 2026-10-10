# Missing features and known issues

Updated 10 October 2026 against v0.4.2. This is a checklist only; recording an item does not mean it has been implemented or tested on hardware.

## v0.5 work in progress

- Implemented separate Adaptive/Turbo fan buttons and Power Saver/Balanced/Performance buttons; each preserves the other setting.
- Kept regular Office/Balanced/High/Turbo combinations in a presets menu.
- Added Save current as Custom / Apply saved Custom for a supported fan + Windows plan combination. This is not a custom fan curve.
- v0.5.1 identifies this laptop as four-zone (ff12) and updates all four Solid color slots. Physical acceptance is pending; the keyboard defect below is not yet closed. The unrelated per-key row candidate was reverted.

## Priority 1 — reported problems and requested everyday behavior

- [ ] **Fix incomplete keyboard lighting.** Owner reports that changing lighting changes only one side of the keyboard. Full-keyboard control is not working correctly. Which keys/zones and effects are affected, and the protocol cause, have not been established. Verify whole-keyboard coverage for every supported effect, color, brightness and off command before calling lighting complete.
- [ ] **Keep the light bar off after restart.** Remember the desired state and reliably apply it at Windows sign-in. Before-Windows/boot lighting persistence is a separate, unverified firmware capability.
- [ ] **Verify reliable light-bar ON → OFF switching.** Earlier owner report: ON worked, but OFF required reconnecting. Acknowledgement/recovery logic was changed, but a conclusive physical retest is still missing.
- [ ] **Make startup settings reliable from packaged launch contexts.** The toggle can read/write a private redirected registry instead of Windows' real startup entry. This laptop's real entry was separately corrected and independently verified; automatic handling in the app remains unfinished.
- [ ] **Complete a real restart test.** Confirm one tray instance starts, can be opened, and connects correctly after sign-in. Registry readback and process-start tests do not prove reboot behavior.
- [ ] **Persistent privileged control and preferences.** Current startup opens a monitor-only tray app; Enable controls still needs administrator approval after sign-in. A protected, narrowly scoped background component and reliable preference restoration are not implemented.
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

- [ ] **Finish installation/update/uninstall behavior.** Reliable shortcuts, version migration, startup cleanup and clear diagnostics remain product work. Builds are currently unsigned prereleases.
- [ ] **Verify operation after OEM removal/disablement.** Code supports an absent OEM service and does not load OEM DLLs, but a complete real uninstall/reboot/hotkey test has not been performed. Do not claim full replacement yet.
- [ ] **Resolve original OEM startup interference.** The original launcher still has its own Windows startup entry. Its InstallServGM prompt belongs to the OEM software; coexistence and removal guidance need validation. Do not remove it automatically as part of this checklist.

## Confirmed working — do not reopen as missing without new evidence

- Administrator handoff/Enable controls: owner confirmed working in v0.4.2 after the physical-path fix.
- Firmware Automatic and Turbo/Boost: short physical fan response was confirmed previously.
- Physical turbo button: owner confirmed that the app's mode display updates correctly in v0.4.2.
- Real Windows startup entry: independently verified for the installed v0.4.2 path; actual reboot and the packaged-context toggle remain outstanding above.
- Native UI, bounded helper execution, automated tests and measured UI resource use exist. They do not establish full feature parity or a guarantee against hardware failure.
