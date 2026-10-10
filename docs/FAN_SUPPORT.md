# Additional fan controls — assessment, 10 October 2026

The reviewed protocol has more than two encodable modes: firmware Auto (0), Boost (64), fixed levels (128–133), and a custom table mode (160). Protocol availability alone does not establish safe cooling.

Local static analysis of the owner's original installation identifies an application-side overheating event handler that raises cooling to a fixed level or restores Auto. This is an independent reason not to assume that firmware will override a low custom setting when the controller crashes or Windows stops. The original implementation is not copied into this project or distributed.

Existing read-only candidates have not established trustworthy CPU temperature, fan mapping, RPM scale, minimum running speed, or independent CPU/GPU control. The candidate speed byte was zero while fans were running. The previous short physical test establishes Auto/Boost response only, not sustained cooling or autonomous thermal failover.

The app therefore continues to expose Auto/Boost, independent Windows power plans, Office/Balanced/High/Turbo combinations, and a saved fan + power combination. It does not encode fixed-level or custom-table writes. The offline curve simulator is disconnected from the hardware backend.

Before additional curves can become hardware controls, validation must establish:

- Independently calibrated CPU/GPU temperature and fan response across idle, load, and cooldown.
- A firmware cooling override that survives process failure, missing/stale sensors, and OS failure.
- Reliable running/startup limits, register units, CPU/GPU fan mapping, and readback.
- Recovery during sleep/resume, AC/battery changes, and controller conflicts.

No deliberate overheating test is part of this work. A host application that normally monitors temperature is insufficient evidence of an autonomous firmware fail-safe. Until that evidence exists, extra speed controls cannot meet the owner's requirement to remain safe when software fails.

## OEM Office mode

Further static review on 10 October identifies Office Basic as a fixed level (128 + level) or table mode (160) on some variants; Office Advanced writes a table and changes CPU power limits. The older path also exposes Office/custom alongside an application-side overheating handler. None is equivalent to the existing Windows Power Saver preset, and none establishes autonomous protection on this exact BIOS. The v0.7 Office button is therefore explicit Auto + Saver. It is more accessible, but minimum-speed OEM parity remains unavailable.
