# Benchmark methodology

Verified local results for release executable SHA-256 `c4edf4c9834d8544a9c664a30c2b1c0a024cb754fad519d645b29e91e77cec72` (commit `3fd8ca7`):

- Warm `status --json`: 30 runs after 3 warmups; median **20.04 ms**, p95 **36.40 ms**, executable **573,952 bytes**.
- Offline policy: 7 rounds of 1,000,000 evaluations; median **18.80 ns/evaluation**.
- Ten-minute `watch`: 600 samples over **600,000.97 ms**; maximum sampled working set **5.65 MiB**, maximum private commit **0.84 MiB**, measured process CPU time **0 ms** at this sample resolution.

These figures are diagnostic-only measurements. They do not establish fan performance, noise, cooling safety, game frame rates, or improvement over Monster Control Center. The watch process reports no thermal control activity.

Use a release build. The local reference machine is an i7-8750H Monster Tulpar T5 V17.1 with GTX 1060. The diagnostic executable does not query GPU sensors or control hardware.

- **Warm status latency:** 30 complete `status --json` launches after three warmups. Includes process creation, registry queries, OS-counter queries, serialization and PowerShell output capture. This is not a cold boot or GUI measurement.
- **Offline policy:** seven rounds of one million synthetic evaluations. Includes sensor validity checks, curve lookup, hysteresis and state transitions; uses `black_box` and an observable checksum. No EC, WMI, GPU or CPU sensor access occurs in the measured loop.
- **Monitoring:** ten minutes at one sample per second. Record process CPU time, sampled working set and private commit. CPU percentage is normalized to one logical core, not total system capacity. Windows CPU counters are quantized, so small values can be zero at short durations.
- **Binary:** record exact file size and SHA-256. Inspect imports for excluded hardware-write interfaces and dependencies.

These tests establish the diagnostic and offline-computation overhead only. They do not establish noise, cooling performance, game frame times, idle package power, firmware safety or improvement over the OEM application. A future hardware backend and GUI need their own measurements.

Raw reports remain local and ignored. The public summary must report whether other activity occurred during measurements and which commit/binary produced them.
