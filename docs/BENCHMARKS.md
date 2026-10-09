# Benchmark methodology

Measurements are in progress. Reviewed aggregate results will be added after the final executable passes verification.

Use a release build. The local reference machine is an i7-8750H Monster Tulpar T5 V17.1 with GTX 1060. The diagnostic executable does not query GPU sensors or control hardware.

- **Warm status latency:** 30 complete `status --json` launches after three warmups. Includes process creation, registry queries, OS-counter queries, serialization and PowerShell output capture. This is not a cold boot or GUI measurement.
- **Offline policy:** seven rounds of one million synthetic evaluations. Includes sensor validity checks, curve lookup, hysteresis and state transitions; uses `black_box` and an observable checksum. No EC, WMI, GPU or CPU sensor access occurs in the measured loop.
- **Monitoring:** ten minutes at one sample per second. Record process CPU time, sampled working set and private commit. CPU percentage is normalized to one logical core, not total system capacity. Windows CPU counters are quantized, so small values can be zero at short durations.
- **Binary:** record exact file size and SHA-256. Inspect imports for excluded hardware-write interfaces and dependencies.

These tests establish the diagnostic and offline-computation overhead only. They do not establish noise, cooling performance, game frame times, idle package power, firmware safety or improvement over the OEM application. A future hardware backend and GUI need their own measurements.

Raw reports remain local and ignored. The public summary must report whether other activity occurred during measurements and which commit/binary produced them.
