# Safety boundary — v0.2

The backend permits firmware automatic/boost, three convenience switches, supported keyboard HID effects, and documented Windows controls. It excludes fixed speeds, fan-off, arbitrary EC writes, custom low-speed tables, power-limit/voltage changes, firmware flashing, driver installation, registry writes and startup configuration changes.

No software guarantees a PC can never fail. Evidence covers one short physical cooling test on one BIOS, not all workloads or power transitions. Keep the OEM app installed.

## Guards

- Exact model, BIOS, CPU and GPU fingerprint; reviewed ACPI instance only.
- Separate read/write encoding; verified low-byte decoding of word returns. Invalid capability data refuses writes.
- Named transaction mutex. Running OEM controllers/services block hardware writes; external software need not honor this mutex.
- Fan changes check capability, idempotence and readback. Failure attempts auto once and reports its readback. Broken WMI/firmware can prevent recovery.
- Switches read desired/current state, preserve unrelated bits, check readback and avoid repeated failed toggles.
- Handover only stops/starts the existing exact-path GamingCenter user-mode launcher, in automatic fan mode. No driver service or startup configuration changes. Exit its already-launched GUI separately.
- UI helpers have five-second deadlines and 64 KiB per-stream output limits. Killing a helper cannot undo an already-submitted transaction. Manual CLI operations have no supervising process timeout.
- Profiles validate bounded JSON before use, report partial application, and never apply on startup.

## Cooling and physical button

Automatic and boost are firmware modes. There is no periodic forced speed or host temperature loop. Exit/crash leaves the requested firmware mode; it does not reset hardware. Physical turbo-button routing with OEM paused, sleep/resume and cold boot still need validation.

The offline simulator is disconnected. Its illustrative temperatures and unitless demand are not approved settings.

## Actual evidence

Administrator firmware reads succeeded. Capability flags advertise boost, Win-key lock, USB charging and light bar; initial mode was auto. Newer fan-table defaults read zero and cannot establish safe curves.

First fan test refused writes while the OEM service ran. Later test paused it, set BOOST, checked persistence for five seconds, restored AUTO with readback, and restarted the service. The owner confirmed audible acceleration/return. No power limit or table changed.

NVAPI read GPU temperature 38°C. CPU/RPM candidates remain uncalibrated; plausible numbers are insufficient proof. Physical lighting/switch behavior, sustained thermals/noise, AC changes, resume, reboot and physical turbo interaction remain pending.

PE audit excludes known driver-installation/configuration, raw EC library, remote-memory and power-limit APIs. HID, standard power-plan selection and fixed OEM launcher handover are expected. This is defense in depth, not proof of every side effect.

OEM binaries/decompilation, tools, credentials and raw local reports are excluded from publication. Diagnostics omit serial numbers, usernames, hostname and MAC addresses.

