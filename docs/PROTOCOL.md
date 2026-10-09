# Protocol investigation — no commands enabled

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
