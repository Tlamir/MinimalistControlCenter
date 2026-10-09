# Benchmark methodology

## Native v0.2 preview — source 88805bf

Final release artifacts: mcc.exe **818,176 bytes**, SHA-256 `6f18daec6dcd0c68b466a5eef7d963a375e3c8c41b9c5227b67c05dc69d3ded3`; mcc-ui.exe **629,248 bytes**, SHA-256 `efbee97e1f422a7afa7d4f618bc19dc45c3de7225a11dd10e5638405d52a1d0a`.

- Native visible UI: **30,325.87 ms**, process CPU **375 ms**, working/peak set **13,180,928 bytes (12.57 MiB)**, private commit **2,129,920 bytes**.
- Native hidden UI: **30,263.54 ms**, process CPU **109.375 ms**, working set **10,321,920 bytes (9.84 MiB)**, peak **10,350,592 bytes**, private commit **1,630,208 bytes**.
- These two short runs occurred concurrently during development activity and include initialization. Benchmark mode adds a measurement timer; ordinary hidden mode has no dashboard timer. They are not isolated steady-state, power, game FPS or OEM comparisons. Windows CPU counters are quantized.
- Final release **59 library + 6 executable integration tests passed**, strict Clippy passed, both PE import audits passed. Real native UI self-test continued processing timer events while its synthetic worker slept. No hardware is accessed by the UI benchmark or self-test.
- The separate real cooling test confirmed AUTO -> BOOST -> AUTO and restored the OEM launcher; the owner confirmed audible acceleration and return. This establishes basic control on this unit, not sustained thermal/noise improvement or all power transitions.

Earlier v0.2 CLI build (before launcher handover was added): 30 warm launches after 3 warmups, median **30.23 ms**, p95 **66.32 ms**; 120-second watch, 120 samples, max sampled working set **7,086,080 bytes**, private **1,310,720 bytes**, measured CPU **0 ms** at counter resolution. These earlier CLI numbers do not identify the final binaries above.

## Historical v0.1 diagnostic measurements

Verified local results for release executable SHA-256 `c4edf4c9834d8544a9c664a30c2b1c0a024cb754fad519d645b29e91e77cec72` (commit `3fd8ca7`):

- Warm `status --json`: 30 runs after 3 warmups; median **20.04 ms**, p95 **36.40 ms**, executable **573,952 bytes**.
- Offline policy: 7 rounds of 1,000,000 evaluations; median **18.80 ns/evaluation**.
- Ten-minute `watch`: 600 samples over **600,000.97 ms**; maximum sampled working set **5.39 MiB**, maximum private commit **0.84 MiB**, measured process CPU time **0 ms** at this sample resolution.

These figures are diagnostic-only measurements. They do not establish fan performance, noise, cooling safety, game frame rates, or improvement over Monster Control Center. The watch process reports no thermal control activity.

Use a release build. The local reference machine is an i7-8750H Monster Tulpar T5 V17.1 with GTX 1060. The diagnostic executable does not query GPU sensors or control hardware.

- **Warm status latency:** 30 complete `status --json` launches after three warmups. Includes process creation, registry queries, OS-counter queries, serialization and PowerShell output capture. This is not a cold boot or GUI measurement.
- **Offline policy:** seven rounds of one million synthetic evaluations. Includes sensor validity checks, curve lookup, hysteresis and state transitions; uses `black_box` and an observable checksum. No EC, WMI, GPU or CPU sensor access occurs in the measured loop.
- **Monitoring:** ten minutes at one sample per second. Record process CPU time, sampled working set and private commit. CPU percentage is normalized to one logical core, not total system capacity. Windows CPU counters are quantized, so small values can be zero at short durations.
- **Binary:** record exact file size and SHA-256. Inspect imports for excluded hardware-write interfaces and dependencies.

These tests establish the diagnostic and offline-computation overhead only. They do not establish noise, cooling performance, game frame times, idle package power, firmware safety or improvement over the OEM application. A future hardware backend and GUI need their own measurements.

Raw reports remain local and ignored. The public summary must report whether other activity occurred during measurements and which commit/binary produced them.
