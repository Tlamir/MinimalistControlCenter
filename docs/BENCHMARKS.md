# v0.4.2 validation — 10 October 2026

95 release tests passed (83 library, 5 native-worker, 7 executable integration). Strict Clippy, formatting and both PE import audits passed. New regression coverage verifies physical executable path resolution and distinguishes cancellation from other Windows launch errors.

Actual UAC isolation: v0.4.1 at its apparent LocalAppData path failed with error 3 both while running and after exit. GetFinalPathNameByHandle exposed package filesystem redirection. Launching those exact installed bytes through the resolved package-cache path succeeded, exit 0; its elevated UI self-test reported hardware_accessed=false and worker_responsive=true. This test changed no firmware settings. It is functional verification, not a new performance comparison.

v0.4.2 installed at %USERPROFILE%/Applications/MinimalistControlCenter/0.4.2. Initial same-process startup readback passed but proved insufficient under package registry redirection. The fixed current-user entry was subsequently registered through StdRegProv and independently confirmed by both StdRegProv and Win32_StartupCommand. A real reboot remains untested. The owner confirmed Enable controls succeeds and pressing the physical turbo button correctly updates the UI. Historical v0.4 measurements below must not be read as evidence that its original installation supported UAC or reboot startup.

---

# v0.4 validation — 10 October 2026

Final local release suite: **93 passed** (81 library, 5 native-worker, 7 executable integration tests). Strict Clippy and formatting pass. Both final PE import audits pass. Native self-test verifies Segoe UI resolution, page isolation, disabled startup hardware controls and message processing during a delayed worker. Helper leases reject replacement/rename but still allow actual CLI execution.

The app was installed into the owner's versioned LocalAppData directory. Actual startup enable/disable/idempotent-disable/re-enable readbacks passed, leaving startup enabled. Installed tray launch and duplicate-instance exit passed, with no visible main window or enabled hardware controls. A real Windows reboot remains an owner check.

| Measurement | Duration | Process CPU | Working set | Paints |
|---|---:|---:|---:|---:|
| Visible UI, before final helper-lease addition | 30.224s | 62.500ms | 14.86 MiB | 30 |
| Final binary, hidden benchmark | 10.074s | 31.250ms | 14.04 MiB | 0 |

These UI-only measurements include startup and **exclude firmware polling, helper processes and WMI-provider cost**. CPU counters are quantized. The hidden benchmark has a measurement timer; ordinary disconnected tray startup has none. A later attempt at a final visible benchmark was launched with a forced-hidden Windows startup flag and produced zero paints; it is excluded as visible-rendering evidence. Visual acceptance belongs to the owner, who requested no computer-use inspection.

Actual administrator read-only firmware measurements: full status mean **1162.7ms** across8 samples; narrow observation mean **511.0ms** across4 samples. Narrow observation performs one EC mode read plus fingerprint/ownership checks instead of six EC capability/status reads. Helper CPU mean was 35.16ms, omitting delegated WMI-provider CPU. These small samples establish reduced observed latency, not a whole-system CPU, FPS, noise or temperature improvement.

Both real probes made zero hardware writes. The candidate RPM byte remained0 despite operating fans; CPU/RPM calibration and reduced-speed thermal failover remain unresolved. New workload presets use only already-reviewed autonomous Auto/Boost and standard Windows power plans. Physical synchronization/light-bar/reboot checks are pending.

## Final local binaries

- `mcc.exe`: 854016 bytes; SHA-256 `76cb8a31678c0ffc75603a92cbc4181b885a8d4764627ea479c9d1f9baab33de`.
- `mcc-ui.exe`: 669184 bytes; SHA-256 `a9e4a82445e776db4b0439ecae1e62db3f8c96e86826cf8033424b7d5c3b9be2`.

Historical measurements follow; they describe earlier versions.

---

# Benchmark methodology

## Native v0.3 — simplified UI

Final release artifacts: mcc.exe **821,248 bytes**, SHA-256 `e7fb075d1913f1161007ff9f40dc0d626073f4deb5cf847dcf97468fbca07c10`; mcc-ui.exe **641,024 bytes**, SHA-256 `d14c65d230c62541c4acaa16873f2f95b90d2ffeb18686a8e2796227816b5a3d`.

- Visible UI: **30,140.314 ms**, process CPU **218.75 ms**, working/peak set **15,728,640 bytes (15 MiB)**, private commit **2,486,272 bytes**. Thirty paints; nine initial button paints. Total measured paint wall time **106.768 ms**, OS dashboard calls **10.102 ms**.
- Hidden UI: **30,090.720 ms**, process CPU **46.875 ms**, working/peak set **12,492,800 bytes (11.91 MiB)**, private commit **1,933,312 bytes**, zero paints. Hidden benchmark adds a 1 Hz measurement timer; ordinary hidden mode has no dashboard timer.
- This averages **0.73% of one logical CPU**, including initialization. It is not a steady-state CPU percentage or game-performance result. Strict linting and a short release test run overlapped the first part of the visible measurement; no UI automation client was active.
- A separate ten-second run after resetting the UI inspection session used **125 ms CPU**, **15,642,624 bytes working set**, **47.147 ms paint wall time** and **3.676 ms OS-dashboard wall time**.
- An earlier run during the UI inspection session measured **7,953.125 ms CPU / 30,181.743 ms**, working set **19,664,896 bytes**. Subsequent instrumentation found no repaint loop (one paint per second). Periodic painting was restricted to dashboard values, dialog routing to keyboard messages, and the inspection client was reset. The final numbers reflect all those changes; this experiment does not isolate one cause for the earlier high figure. Raw reports remain ignored/local.
- Local validation: **69 library + 4 worker + 6 executable integration tests = 79 passed**. Strict Clippy and both final PE audits passed. The real native self-test checks page isolation, animation-field visibility, disabled startup writes and responsiveness during a 500 ms synthetic worker delay.
- Actual UI preview testing reproduced a failed light-bar acknowledgement, then successfully switched OFF without reconnecting. The preview backend simulates every hardware operation. Physical timing still needs retesting on the laptop. No new hardware writes occurred during this UI revision's tests or benchmarks; the unprivileged status probe correctly reported administrator access required.

All UI measurements include startup, use quantized Windows process counters and access OS metrics only. They do not establish thermal/noise improvement, game FPS, idle power, zero impact, OEM speedup or safety under every workload.

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
