# Owner checks for v0.4

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
