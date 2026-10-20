# v0.5.1 — verified keyboard variant and Solid protocol correction

Read-only Windows PnP inventory on the target reports USB VID048d/PIDce00 revision0002, with control collection usage pageff12 / usage1. This is the OEM four-zone variant, not the ff02 per-key variant.

Local OEM protocol inspection: four-zone Solid loads color slots1,2,3,4 using feature opcode0x14, then effect opcode0x08 with palette selector8. The previous implementation loaded only slot1 and selected1, matching the owner's left-side-only observation. The corrected uniform-color command fills all four slots. All reports retain nonvolatile-save0. Off remains unchanged. Regression tests cover every zone, mode selection, and variant isolation.

An earlier six-row transfer candidate applied only to ff02 and could not fix this machine. That candidate was reverted; the original per-key picture path is unchanged. Physical acceptance of the four-zone fix is pending.

---

# v0.4 investigation update — 10 October 2026

The owner confirmed that the physical turbo button changes cooling but the v0.3 display stayed stale. v0.4 adds bounded read-only observation and derives highlighted presets from observed mode and active Windows plan. It never replays the old fan request to fight a physical-button change.

Independent static findings from the installed controller: mode 1873 uses 0 for adaptive and 64 for boost; older fixed levels occupy 128–133 and custom table mode is 160. Old MyFan2 table entries are 1859–1863; generation byte 1922 bit 3 selects MyFan3. Prior read of 1922 was zero on this laptop. The OEM has an application-side over-temperature event path. **Autonomous thermal override for reduced fan settings remains unverified.** No new fan-write value is enabled.

Read-only calibration diagnostics add raw candidates 1086/6148, generation 1922, table 1859–1863, and defaults 1929–1933. They are not presented as Celsius, RPM or validated percentages. [Community transport/control observations](https://gitlab.com/siphomateke/tongfang-control/-/raw/master/tongfang_control/fan.py) corroborate mode locations but describe generation-dependent temperature bands; they cannot prove this laptop's thermal limits.

The historical notes below retain the evidence chronology. Statements about an earlier release lacking a backend do not describe the current v0.4 executable.

# Protocol investigation

## Runtime update — v0.2

Administrator probes and the native adapter successfully invoked reviewed reads. Return contains a word; the requested byte is the low eight bits, corroborated by OEM byte casts and [community transport](https://gitlab.com/siphomateke/tongfang-control/-/raw/master/tongfang_control/acpi.py). Tests pin actual values to this decoding.

Initial mode byte1873=0. Support1 byte1893=224, support2 byte1894=131. Newer rule1922=0; newer defaults1929..1933 were zero. No low/fixed custom curves are exposed.

Actual AUTO -> BOOST64 -> AUTO0 passed with five-second persistence, readback and owner-observed acceleration/return. The existing launcher was temporarily stopped/restored. No table or power-limit writes occurred. NVAPI read38°C; CPU/RPM candidates uncalibrated. Physical lighting/switch validation remains pending.

See SAFETY.md for the current boundary and MISSING_FEATURES.md for unfinished work. Findings below describe the earlier diagnostic stage, not the current executable.

Investigated 9 October 2026. This document contains independently written observations, not vendor source. Installed `GamingCenter.exe` and `MySettingDLL.dll` were statically decompiled with ILSpy's decompiler library. Their hashes are recorded in PLAN.md. No vendor assembly was executed. Decompiler output remains in an ignored local directory.

The firmware model is still unverified beyond the reseller identification TULPAR T5 V17.1 and BIOS N.1.06. A multi-model application can include code paths that do not apply to this unit.

Local read-only metadata discovery subsequently confirmed both `AcpiTest_MULong` and `AcpiTest_EventULong`. `GetSetULong` exposes `Data: UInt64` and an output parameter named `Return: UInt32`; its method return type is reported separately as Boolean. No instance method was invoked. Raw output remains local.

| Area | Static observation | Runtime status |
|---|---|---|
| Transport | `root/WMI`, class `AcpiTest_MULong`, `GetSetULong`; OEM selects a particular ACPI instance | Metadata discovery only; no method invocation |
| Read encoding | The OEM read wrapper adds a high-order read discriminator to an address before invoking the combined method | Not implemented: malformed encoding could become a write |
| Write encoding | The same combined method packs a value and address without that read discriminator | Deliberately absent from executable |
| Fan mode | Static fan-mode code distinguishes normal, boost, fixed level and table modes | Not enabled; no physical response or recovery validation |
| MyFan2 table | Five adjacent entries; scaling depends on OEM application type and firmware capability | Raw values must not be treated as percentages |
| Default table | Different MyFan2 and MyFan3 default-table regions overlap other generation-specific meanings | Exact firmware generation must precede any register interpretation |
| Capability checks | Several flags control the new MyFan2 rule, boost support and UI availability | Not read from EC |
| Power side effects | Another fan-setting page changes PL1/PL2/PL4/Tau along with fan modes | A fan-only UI action may alter power settings; don't port blindly |
| CPU temperature | Dashboard calls a native CPU-info component | No trustworthy standalone sensor backend established |
| GPU temperature | Dashboard calls a native NVIDIA information wrapper | Candidate: documented NVAPI; not enabled in v0.1 |
| RGB and convenience controls | Present in OEM app but not fully traced | Deferred |

One older customization path appears to perform a mode write without consulting its caller's write-enable argument. This is a reason not to execute OEM UI initialization paths merely to discover settings. It is not evidence that this path is active on this laptop or that it caused the reported freeze.

The old app also rebuilds WMI objects in its read wrapper. That is a potential optimization target, not a measured explanation for its performance.

## Required evidence before enabling control

1. Confirm exact hardware/EC revision and active firmware generation, including capability bits through reviewed, narrowly allowlisted reads.
2. Verify read payload encoding independently against static analysis and local method schema. Keep read and write APIs structurally separate even though firmware multiplexes them.
3. Establish valid temperatures, RPM scaling, sample age, and CPU/GPU thermal coverage. Compare to a trusted measurement source.
4. Determine the OEM service's ownership and settings persistence without stopping it until a tested handover exists.
5. Establish firmware-auto restore, readback and observed physical response, including power-limit side effects.
6. Validate service termination, OS failure assumptions, AC transitions, sleep/resume and cold boot. A software watchdog is not a firmware watchdog.
7. Enable only the specific validated capability for the verified firmware identity. A successful WMI return alone is insufficient.

Until those conditions are met, the executable has no EC transaction implementation, no fan-control backend and no command to unlock one. Tests passing on a simulator do not satisfy these hardware gates.
