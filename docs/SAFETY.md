# v0.8 update

The visible Cooling page reuses a read-only WMI helper and checks fan mode and controller ownership every five seconds. It exits on hide, page change, lost permission or read failure. Input accepts only bounded read requests; replies have sequence numbers and size limits. The helper remains in a kill-on-close job. Hardware writes still use separate helpers with fresh fingerprint, ownership, capability and readback checks.

Boot initialization allows 20 seconds per hardware request and at most three attempts, with 10- and 30-second waits. Recovery runs while hidden and stops on success, exhaustion or a user action. A fresh allowed status confirming light-bar off ends recovery even after a late acknowledgement. Each off request checks pending pulses and current state before writing; it never blindly repeats a toggle. A hidden-window regression test simulates a failed connection followed by recovery. A real reboot of this release still needs owner validation.

# v0.7 update

No additional fan writes are enabled. Office minimum/fixed/table modes still require independent protection and calibration evidence. Dynamic laptop naming does not broaden the hardware allowlist. GPU temperature/clock/tachometer requests use public read-only NVAPI interfaces through the same bounded helper; no continuous GPU polling or new driver. Zero/unavailable tach readings remain unknown. CPU clock uses read-only Windows power information and is labelled OS, not instantaneous per-core turbo.

The dashboard and firmware observation timers stop when hidden. Reopening reads current state; every hardware write still checks ownership, capabilities and readback. Firmware Auto/Boost operates independently of the GUI. The owner confirmed real restart/startup and physical light-bar-off on v0.6.2.

# Safety boundary — v0.6

The backend permits firmware automatic/boost, three convenience switches, supported keyboard HID effects, and documented Windows controls. It excludes fixed speeds, fan-off, arbitrary EC writes, custom low-speed tables, power-limit/voltage changes, firmware flashing and driver installation. Portable startup changes only the current-user `Run/MinimalistControlCenter` value and applies no hardware settings.

The optional administrator installer protects versioned binaries under Program Files and registers a task for the current administrator user's interactive logon, with highest available privileges and no stored password. The task's owner/DACL permits ordinary read/run access but not action replacement; binaries and working directory allow ordinary read/execute only. Startup requests connection and light-bar off, never a saved fan/power/keyboard profile. Existing OEM service handover still restores Auto when required. Failed interactive requests are followed by readback without replay. Boot initialization uses the bounded desired-state recovery described above and warns only after its attempts are exhausted.

Installation removes the old Minimalist Run entry, adds a fixed per-user Installed Apps record, and backs up/removes only the exact reviewed HKLM Monster launcher startup entry. Touchpad/OSD entries and OEM service startup type are preserved. Uninstall restores the backed-up exact launcher command and preserves saved profiles. Elevated startup is never registered against a user-writable portable directory. Task/real-registry status is checked through bounded helpers rather than a private package hive.

No software guarantees a PC can never fail. Evidence covers one short physical cooling test on one BIOS, not all workloads or power transitions. Keep the OEM app installed.

## Guards

- Exact model, BIOS, CPU and GPU fingerprint; reviewed ACPI instance only.
- Separate read/write encoding; verified low-byte decoding of word returns. Invalid capability data refuses writes.
- Named transaction mutex. Running OEM controllers/services block hardware writes; external software need not honor this mutex.
- Fan changes check capability, idempotence and readback. Failure attempts auto once and reports its readback. Broken WMI/firmware can prevent recovery.
- Switches wait for preceding toggle pulses, re-read current state after the wait, preserve unrelated bits, submit one pulse and poll acknowledgement. A late successful request is never blindly toggled back. Pending pulses that do not clear refuse writes.
- Handover only stops/starts the existing exact-path GamingCenter user-mode launcher, in automatic fan mode. No driver service or startup configuration changes. Exit its already-launched GUI separately.
- The running UI holds read-only/no-delete leases on the helper executable and its immediate directory, preventing ordinary replacement while it can launch elevated helpers. Versioned installation avoids overwriting a running version.
- UI helpers have five-second deadlines and 64 KiB per-stream output limits. Killing a helper cannot undo an already-submitted transaction. Manual CLI operations have no supervising process timeout.
- Startup helpers use a 15-second outer deadline and a 12-second PowerShell deadline. Child processes belong to a kill-on-close Windows job, so timeout cleanup also terminates descendants. Startup metadata is not polled continuously or synchronously on the UI thread.
- After a hardware action, including failure, the UI refreshes status. Fresh readback and ownership allow further actions; unavailable status revokes access. Recovery never replays the action. Missing OEM software needs no handover service; changed/ambiguous service paths are refused.
- Profiles validate bounded JSON before use, report partial application, and never apply automatically on startup. The installed logon task makes one separate desired-state light-bar-off request. Office/Balanced/High workload presets select firmware Auto before changing the Windows power plan. Turbo preserves the power plan. These modes do not pin fan speed low.

## Cooling and physical button

Automatic and boost are firmware modes. There is no periodic forced speed or host temperature loop. Exit/crash leaves the requested firmware mode; it does not reset hardware. The owner reports the physical turbo button changes real cooling with the original app paused. v0.4 observes firmware mode periodically rather than continuously overriding it. Refresh failure clears state; bounded retries remain reads only. End-to-end physical display synchronization, sleep/resume and cold boot still need owner validation.

The offline simulator is disconnected. Its illustrative temperatures and unitless demand are not approved settings.

## Actual evidence

Administrator firmware reads succeeded. Capability flags advertise boost, Win-key lock, USB charging and light bar; initial mode was auto. Newer fan-table defaults read zero and cannot establish safe curves.

First fan test refused writes while the OEM service ran. Later test paused it, set BOOST, checked persistence for five seconds, restored AUTO with readback, and restarted the service. The owner confirmed audible acceleration/return. No power limit or table changed.

NVAPI read GPU temperature 38°C. CPU/RPM candidates remain uncalibrated; plausible numbers are insufficient proof. Physical lighting/switch behavior, sustained thermals/noise, AC changes, resume, reboot and physical turbo interaction remain pending.

PE audit excludes known driver-installation/configuration, raw EC library, remote-memory and power-limit APIs. HID, standard power-plan selection and fixed OEM launcher handover are expected. Startup registry access uses the embedded, fixed-scope Windows provider script in src/startup.ps1. The import audit cannot enforce the PowerShell script boundary; that source is reviewed separately. Disabling startup disables only the logon trigger, preserving protected demand launch. This is defense in depth, not proof of every side effect.

OEM binaries/decompilation, tools, credentials and raw local reports are excluded from publication. Diagnostics omit serial numbers, usernames, hostname and MAC addresses.

## Why minimum-noise and custom fan tables remain locked

Static inspection identifies old MyFan2 fixed-level values 128–133 and table mode 160, but not a proven autonomous fail-safe for this exact EC. The OEM also has an over-temperature event path that performs application-side fan writes. This is not evidence that a stopped/crashed application leaves a low setting thermally safe. Community implementations describe different temperature bands across generations. A successful register write or a passing mock cannot establish cooling behavior.

The new calibration-probe command reads only fixed candidate addresses and labels all output unvalidated. It does not grant permission for custom cooling. Independent CPU/GPU temperature correlation, fan response/scaling, thermal override, sensor-loss handling, and process/OS failure behavior remain required. No test should deliberately heat the laptop to 100 °C to discover these limits.
