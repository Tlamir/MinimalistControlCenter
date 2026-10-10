# Owner confirmation — 10 October 2026

The owner tested a normal restart on v0.6.2 and confirmed that automatic startup works and the light bar is physically off after sign-in. That check is complete. Battery sign-in, physical keyboard effects, sleep/resume and uninstall still need separate observations.

The v0.7 UI uses a dynamic model header, direct preset buttons, shorter copy, an embedded icon and on-demand GPU readings. Office remains Auto + Power Saver; it does not implement an OEM low-speed fan table.

Historical checks below are preserved as the checklist used for v0.6.

Installed v0.6.2 has passed file/permission checks, startup off/on and desktop/task launch without another UAC prompt. Firmware reports the light bar off. The remaining observations below require a normal restart or checking the physical hardware.

# Owner checks for v0.6

1. Exit old Minimalist instances and the Monster GUI. Extract the full release, run Install.cmd, and approve Windows' installation prompt once. The desktop shortcut should open the installed app without another administrator prompt.
2. At your next normal restart, wait about ten seconds after sign-in. Confirm exactly one Minimalist tray instance starts, controls are ready, and the light bar is physically off. No fan or keyboard preset should be recalled. A tray warning means the connection/write/readback failed; open Lighting and report the actual message instead of repeatedly toggling.
3. Confirm startup still works when signing in on battery. Toggle Start with Windows off, exit using the header button, and reopen from the desktop shortcut without another prompt. Re-enable startup and confirm its display follows the real task state.
4. Confirm Monster's GUI does not reopen automatically, while existing touchpad/OSD behavior remains available. Its exact launcher entry is restored by uninstall; installed OEM files and service startup type are preserved.
5. Check the three redesigned pages, header Hide/Exit buttons and complete status/error Details. Then check Solid colors across all four zones, one light-bar on/off cycle, and physical Turbo-button synchronization. Automated reports cannot establish the keyboard's actual appearance or cooling response.

No forced restart, stress test, or deliberate overheating is required. Extra fan curves remain unavailable pending the independent firmware protection described in [fan support](FAN_SUPPORT.md).

# Historical owner checks for v0.4

The app's automated tests use fake hardware. The following observations must come from the laptop owner. No stress test or intentional overheating is needed for these checks.

1. Exit every old Minimalist instance and the Monster tray app. Open the installed v0.4 UI and Enable controls. Choose Turbo, then press the laptop's physical turbo button. The selected card and fan-mode heading should follow the firmware within a few seconds. If the fans change but the display does not, record the elapsed time and displayed mode. Do not repeatedly click fan buttons to force a response.
2. Toggle the light bar on and off once. If acknowledgement times out, wait for the status check; confirm whether the next desired-state request works without Enable controls.
3. Start with Windows should show On after installation. At your next normal restart and sign-in, the app should be in the tray. It starts without changing the fan mode. Enable controls still needs administrator approval; automatic privileged takeover is not configured.
4. For future custom-fan work, report HWiNFO's CPU Package and GPU temperature readings at the same time as the read-only calibration-probe. If fan RPM is exposed there, report its exact sensor label too. One matching temperature does not establish sensor scaling or thermal override; repeated idle/load/cooldown correlation is required. Do not install an unreviewed low-level driver just for this check.

Minimum-noise and manual fan speeds are not unlocked by these basic checks. Firmware thermal override and behavior after process/OS failure need separate evidence. The candidate fan-speed byte currently reads zero even though the fans run, so it is not a usable RPM reading.

## Useful read-only command

From an administrator terminal in the installed app folder:

```powershell
.\mcc.exe hardware calibration-probe
```

This command reads a fixed list of candidates and labels them unvalidated. It has no arbitrary-address or write argument. Share the displayed readings and observations in the development chat; do not publish a complete personal sensor inventory.
