# Minimalist Control Center — implementation plan

Prepared 9 October 2026. Target: the user's Monster Tulpar T5 V17.1 / reported Tongfang GK5CN6Z, Windows, Intel Core i7-8750H and NVIDIA GeForce GTX 1060.

This is a research and implementation plan, not working control software. Investigation used file/metadata inspection, registry reads and online source review. No fan, power, driver, service or firmware settings were changed; no OEM executable or DLL was executed. Installed binaries are enough to begin interoperability research; original source would help but is not required for the first milestone.

## Recommendation

Build a small Rust Windows service plus an on-demand native Win32 interface. Use the existing firmware WMI interface where verified. Prefer firmware-managed fan curves over continuous manual control. Make reliable fan behavior the first deliverable, then add lighting and convenience settings.

C++20 with Win32 is an equally viable implementation choice. Rust is recommended for keeping policy, configuration and service logic memory-safe; Windows/COM/FFI still needs careful review. Neither language fixes incorrect firmware commands, blocking sensor calls or excessive polling by itself. Microsoft provides Windows API bindings for Rust [6].

The success criteria are measured responsiveness, resource use, temperature, noise and sustained workload performance. A literal 10,000x improvement or zero performance impact cannot be promised.

## Evidence collected locally

| Evidence | Observation | Meaning / limit |
|---|---|---|
| BIOS registry | MONSTER; TULPAR T5 V17.1; BIOS N.1.06; BIOSReleaseDate string `05/03/2018` | Confirms reseller model. Baseboard reports `Standard`, so exact GK5CN6Z identity and EC firmware remain unverified. |
| CPU registry | Intel Core i7-8750H @ 2.20GHz | Confirms local CPU identity. |
| Display driver registry | NVIDIA GeForce GTX 1060; Intel UHD Graphics 630 | Matches requested GPU generation; does not prove a display MUX exists. |
| GamingCenter.exe.config | .NET Framework 4.6.1 | Existing runtime target, not proof of why the app freezes. |
| GamingCenter.exe | Managed assembly, version 1.2.3.4; WPF-related names/resources | Good candidate for ILSpy analysis. Version may be a generic assembly version. |
| MySettingDLL.dll | Managed assembly, version 2.0.0.1 | Likely useful for WMI wrappers and events. |
| Strings in those two assemblies | `root\WMI`, `AcpiTest_MULong`, `GetSetULong` | Matches community protocol research; actual local method availability has not been queried successfully. |
| GamingCenter.exe strings | `SetFanMode`, `SetFanCustomizeMode`, `SetFanCustomize_L1_PWM` through `L5`, `GetFanTablePWMDefault`, MyFan2/MyFan3 identifiers | Suggests several firmware implementations and a five-level interface. Presence in a multi-model binary does not prove support on this laptop. |
| Installed native components | ECIO.dll, SmIoDll.dll, SmIoDriver.sys, inpoutx64.dll, CPUInfoDLL.dll, GPUInfoDLL.dll, XTU components | Potential fallback research targets. Do not assume these are all active or all needed. |

Standard CIM inventory queries were unavailable in this execution context. Registry reads supplied the identity above. No fan RPM, temperatures, live WMI capability bits, active OEM ownership or baseline resource measurements were established. This is initial static inspection, not completed decompilation or a diagnosis of the freeze.

Local reference SHA-256 hashes:

```text
GamingCenter.exe        679A2F2287059AEF2C43744BC2AD03F8610B6D1BFFAC87BD9C11803625D79130
GamingCenter.exe.config 40CAB2754C663895F74D27A793BBF656227AA5C2FC4634A87252CEA96847E221
MySettingDLL.dll        D13A9D3862392DB1EFD328AF2C9894AC19EBB1B786275B3C2B3CFB737589F9CE
ECIO.dll               8E875E5C597CFCB0C1F78622DED916A18B654357B45DF8C7012AE029A80EA9E3
SmIoDll.dll            00D3BE04F02B02CFA17FFC0C71FD9199AA0DB125B4766ABCEC8A687A83CDF55D
SmIoDriver.sys         2F8A8B3893E991830934BE8CA56905192828B4F0AC60F3EE60203440B1F30DD3
```

## What online research establishes

- Tongfang Unofficial Control Center lists GK5CN6Z among supported barebones and describes the `AcpiTest_MULong` path. It was primarily developed on another model, and explicitly describes incomplete functionality [1]. Use it as a protocol lead, not a validated replacement for this machine.
- A GK5CN6Z-specific repository contains extracted MOF definitions. Its `GetSetULong` definition has a 64-bit input and 32-bit output [2]. Match the local schema before implementing; names alone are insufficient.
- The community fan implementation contains capability checks, five PWM levels and interactions between custom PWM and an extra-speed setting. It also includes TODO/FIXME items [3]. This supplies hypotheses for ineffective controls, not proof of the cause here. Its addresses, scaling and temperature boundaries must not be copied blindly.
- ITE 8291 keyboard research lists GK5CN6Z and Monster Tulpar systems [4]. Verify the local USB/HID IDs and controller revision before selecting that backend.
- NVIDIA documents a native GPU thermal query [5]. Temperature telemetry support must be tested with this laptop's driver; it does not establish control over the laptop's EC fans.

## Architecture

```text
Native settings window / optional tray / CLI
                   |
         restricted local named pipe
                   |
        small Windows control service
          |         |           |
   Tongfang WMI   GPU telemetry  Windows power APIs
          |
   embedded controller (EC)

Keyboard lighting: separate verified HID/WMI adapter
```

- Service is the sole hardware writer, with privileges limited to those actually required. The UI runs without elevation and can close completely while control continues.
- Allowlisted, typed requests such as `SetProfile` and `RestoreAutomatic`; never expose arbitrary register writes through IPC. Restrict pipe access and validate lengths, ranges and configuration versions.
- One serialized hardware queue. Cache connection/session objects. Keep WMI and sensors off the UI thread; bound requests and retries. If a provider cannot be bounded reliably, isolate it in a restartable worker and require a safe failure strategy before allowing manual control.
- Native controls and a small graph drawn on demand. No browser runtime, permanent animation loop, telemetry network client or background RGB animation engine. Let the keyboard controller run its effects where supported.
- Small configuration file, bounded in-memory history, rotating diagnostic logs. Persist changes, not every sample. Use Windows power/session notifications instead of polling everything.
- Keep protocol, policy and presentation separate. Suggested workspace components: `policy`, `hardware`, `service`, `ui`, `cli`, plus protocol notes and recorded test fixtures.

## Fan strategy: choose based on verified capability

**Preferred: configure a firmware-managed curve.** If the EC accepts a stable table and continues regulating cooling autonomously, apply the table once and let it run. This is the best path to low CPU use and independence from UI failures. Check whether temperature breakpoints are fixed, whether both fans share a policy and whether firmware retains emergency overrides.

**Second choice: firmware profiles.** If custom tables are unavailable, provide verified Auto/Balanced/Boost modes, clear status and useful power profiles. Do not present a nonfunctional custom-curve editor.

**Optional later: host-managed curves.** Only enable after proving direct control, trustworthy sensor coverage and safe behavior when the process dies, Windows hangs, the laptop sleeps or telemetry fails. A service restart or software watchdog does not protect against an OS crash. If safe autonomous fallback cannot be demonstrated, do not ship persistent manual fan control.

For a validated host controller:

- Evaluate CPU and GPU demand separately. For a shared fan policy, use the greater required cooling output from the two curves, rather than simply treating CPU and GPU temperatures as interchangeable.
- Start policy experiments with roughly 3–5°C hysteresis and a 5–10-second delay before reducing speed. These are tuning hypotheses, not hardware settings to apply now. Allow prompt increases; emergency temperature handling bypasses smoothing.
- Establish minimum reliable running speed and start-up behavior experimentally. Do not offer zero-RPM operation merely because zero can be encoded.
- Begin with a 500–1000 ms control interval, validate thermal response and provider cost, then adjust. Closing the UI must not slow a safety-critical control loop.
- Track sensor age, failures and plausibility. Unknown/stale GPU temperature is not 0°C. A powered-down GPU needs an explicitly verified handling path.
- On faults, use the validated firmware-auto or boost recovery sequence, verify where possible and visibly report failure. A blind write cannot guarantee recovery from a stalled EC.

Separate desired state, accepted command and observed fan response in the UI. Show actual RPM only after validating its scale. PWM, fan level and RPM are different quantities.

Quiet mode should coordinate cooling with supported power settings. Sustained heavy CPU/GPU load still produces heat; lower fan noise may require lower power and performance. Do not add overclocking, voltage writes or undocumented PL settings to the first release.

## Feature scope from the screenshots

| Feature | Delivery and validation |
|---|---|
| Fan Auto / Boost / profiles | First usable release; validate return to firmware control before other writes. |
| Custom fan curves | Conditional on firmware support and observed response. Independent CPU/GPU control is not promised. |
| CPU/GPU temperatures and fan state | First release after validating each sensor. Prefer EC telemetry; use NVAPI for GPU where suitable. Missing data must be labelled. |
| CPU, memory, battery usage | Low-cost OS counters/events. CPU usage is distinct from CPU temperature. |
| Power profiles, AC/battery behavior | Standard Windows power APIs first [7]; preserve/restore previous settings. OEM fan modes may also alter power limits, so document side effects. |
| Keyboard colors, brightness, effects, profiles | Next release; verified HID/WMI commands and firmware-rendered effects. |
| Light bar / projection toggle | Map the actual installed device and OEM command. |
| Windows-key lock, USB charging in hibernation | Add after mapping capability flags and persistence. |
| Hotkeys, mode button, optional OSD | Map vendor events; keep event-driven and let OSD be disabled. |
| Display off | Small Windows integration feature; verify wake behavior. |
| Gaming / Video / Reading display presets | Later: determine whether OEM uses gamma, color profiles or vendor display APIs; restore prior settings. |
| Discrete GPU toggle | Reverse engineer its exact effect. It may express a GPU preference; do not label it a MUX switch without hardware evidence. |
| Disk temperature and network graphs | Optional, sampled slowly or only while visible; not part of the thermal control dependency chain. |
| Extra useful features | Profile import/export, readable diagnostics, fan history, hotkeys, per-app rules later, and one-click firmware-auto recovery. |

Do not rely on `Win32_TemperatureProbe` for CPU temperature: Microsoft documents that its live CurrentReading is not populated [8]. If suitable EC telemetry is absent, investigate a maintained sensor backend separately; avoid turning an old unrestricted I/O driver into the default architecture.

## Milestones and exit criteria

1. **Protocol discovery and baseline — approximately 2–5 focused working days.** Inspect managed assemblies with ILSpy; follow fan methods, capability checks, WMI payload packing, read/modify/write masks and sensor paths. Use native disassembly only where needed. Inventory actual services, hotkeys and current configuration. Record BIOS/EC identity, local WMI schema, firmware-auto behavior and baseline idle/load measurements. Deliver a command/capability matrix with evidence and confidence per operation. No guessed writes or full EC-address scans.
2. **Read-only Rust diagnostic tool — approximately 2–4 days.** Report model, versions, supported capabilities and only verified sensor reads. Timestamp samples and include unavailable/error states. Compare telemetry to a trusted measurement source. Exercise timeout and disconnected-device behavior. Exit when readings and access overhead are credible.
3. **Verified control backend — approximately 3–7 days.** Implement Auto and Boost first, then supported profiles/tables. Validate safe restore, readback and physical response. Coordinate takeover so the OEM and replacement never compete as writers; preserve the original installation and rollback configuration. Exit only after restart, sleep/resume, AC/battery and service-failure tests pass.
4. **Minimal product — approximately 3–5 days.** Add service lifecycle, native tray/window, profiles, status/history, validated curve editing, configuration migration and packaging. Keep unsafe/unverified capabilities unavailable. Exit when UI failures do not interrupt cooling and resource targets are met.
5. **Feature parity and tuning — approximately 1–3 further weeks.** Add verified lighting, hotkeys, convenience settings and optional automation. Run longer thermal/acoustic testing and installation/uninstallation checks.

Rough total: 2–4 weeks for a useful fan-focused MVP, 4–8+ weeks for a polished replacement, assuming the existing WMI interface is sufficient. These are engineering estimates, not delivery promises. Missing telemetry, firmware restrictions or a new signed driver requirement can change scope substantially; re-estimate after milestone 1.

The first implementation task should be the protocol inventory and read-only diagnostic CLI, not the dashboard.

## Performance and reliability acceptance targets

Proposed budgets to measure on this laptop, not current results:

| Metric | Initial target |
|---|---|
| Service + optional tray, UI closed | Under 0.1% total CPU averaged over 10 minutes; investigate any sustained excess. |
| Background private working set | Under 20 MiB combined, subject to measured Windows/COM overhead. |
| Total private working set with settings open | Under 50 MiB initially. |
| Window opening | Under 300 ms warm, under 1 second cold. |
| Hidden window rendering | No rendering loop. |
| Disk/network activity at steady idle | No network traffic; no continuous disk writes. |
| Visible dashboard updates | Roughly 1 Hz initially; redraw only changed content. |
| Firmware-auto monitoring | Optional slower monitoring, around 2–5 seconds initially; event-driven where possible. |
| Application overhead under load | Target under 1% difference in repeated matched workload runs, assessed against run-to-run variability. |

Measure CPU cost in the application AND any WMI provider/driver doing work on its behalf. Check wakeups and idle package power; low application CPU alone is insufficient. Verify sensor polling does not keep the GTX 1060 awake unnecessarily.

Compare three configurations: OEM control, firmware automatic without the OEM UI where safely supported, and the replacement. Keep ambient conditions, Windows/NVIDIA settings, power profile and workload consistent. Separate application-overhead tests at identical cooling settings from tests of new thermal policies. Record temperatures, RPM/level, clocks, throttling, frame times and repeatable fan noise observations. Do not infer acoustic dBA from PWM percentages.

Test the policy using recorded inputs: temperature spikes, oscillation around thresholds, stale or missing sensors, invalid curves, rejected writes and clock discontinuities. Integration tests must cover UI exit, service termination, provider hangs, sleep/hibernate/resume, charger changes, conflicting OEM startup and recovery after restart. Publish observed limitations instead of silently pretending unsupported hardware features work.

## Sources

1. [Tongfang Unofficial Control Center](https://gitlab.com/siphomateke/tongfang-control) — cross-model compatibility and WMI research.
2. [GK5CN6Z extracted WMI schema](https://raw.githubusercontent.com/ehntoo/gk5cn6z_wmi/master/gk5cn6z_wqba.mof) — exact-model reference, not local schema verification.
3. [Community fan implementation](https://gitlab.com/siphomateke/tongfang-control/-/raw/master/tongfang_control/fan.py) — five-level tables, capability checks and implementation caveats.
4. [ITE 8291 keyboard project](https://github.com/0xlsa/open-ec-control-center/blob/main/README.md) — keyboard compatibility leads.
5. [NVIDIA NVAPI thermal interface](https://docs.nvidia.com/nvapi/group__gputhermal.html) — GPU thermal query contract.
6. [Microsoft Rust for Windows](https://github.com/microsoft/windows-rs) — native API bindings.
7. [Microsoft power management functions](https://learn.microsoft.com/en-us/windows/win32/power/power-management-functions) — OS power integration.
8. [Microsoft Win32_TemperatureProbe documentation](https://learn.microsoft.com/fr-fr/windows/win32/cimwin32prov/win32-temperatureprobe) — live reading limitation.
9. [Monster T5 V17.1 support page](https://support.monsternotebook.com/tr/product/tulpar-t5-v171-156-gaming-laptop/drivers-and-downloads/) — official recovery/download reference.

Community implementations are research references, not evidence that their binaries or commands are safe for this firmware. Before incorporating source code, review its license. Keep proprietary binaries and decompiled vendor implementation out of a public repository; maintain independently written code and protocol notes.
