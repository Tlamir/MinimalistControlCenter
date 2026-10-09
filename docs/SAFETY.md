# Safety boundary for v0.1

This release is a diagnostic foundation, not a replacement fan controller. Keep the OEM/firmware cooling system in place. We do not claim it is impossible for software or hardware to fail.

The executable can query a small allowlist of Windows registry strings and documented OS counters. It can optionally launch the Windows system copy of PowerShell with an embedded, fixed metadata-only WMI script. It can print diagnostics and run an offline synthetic policy calculation.

It does not implement EC access, invoke vendor WMI methods, load OEM DLLs, install a driver/service, change fan speed, change power limits, edit startup settings or modify the registry. There is no `--force` or hidden control switch. It does not require administrator privileges for normal operation. In restricted environments, discovery can report unavailable/access denied instead of requesting elevation.

## Tests and evidence

- CLI integration tests execute the actual built program and verify control requests fail before diagnostics begin.
- Sensor unavailable values remain JSON null with a reason, never a fabricated zero.
- Watch intervals and duration are bounded; missed ticks do not produce polling bursts.
- WMI subprocess output is capped at 64 KiB per stream and the operation has a 10-second deadline. Failed parsing, wrong scope, unexpected fields and malformed identifiers are rejected.
- Child-process tests cover hangs, oversized stdout/stderr and nonzero exits.
- Synthetic policy tests cover missing/stale/future/non-finite readings, clock rollback, CPU/GPU demand, rapid heating, delayed cooling, oscillation, integer boundaries and 100,000 deterministic fault samples.
- Windows binary import inspection checks for known hardware/registry/service write APIs and vendor libraries. This catches regressions but does not prove all possible side effects are absent; review source and dependencies too.
- CI runs formatting, strict Clippy, debug/release tests and a Windows executable import audit.

The simulator's emergency temperatures and curves are fictional test inputs. Its unitless demand is not a PWM value and must never be connected directly to hardware. A simulated `firmware_automatic_required` result only describes a recommendation; it does not restore real firmware control.

## Measurement limits

Policy microbenchmarks establish computation cost only. OS-counter monitoring measures this diagnostic executable, not a future service, graphical interface or WMI provider overhead. None of these tests establishes cooling effectiveness, game performance, noise improvement, crash-safe hardware behavior or a speedup over the OEM application.

## Public repository hygiene

OEM binaries, native drivers, decompiled implementation, downloaded tools and local diagnostic reports are ignored. Public benchmark summaries contain only reviewed aggregate measurements. Diagnostics omit usernames, hostname, serial numbers, MAC addresses and user file paths by design. Review reports before sharing them.

Future releases that add control need a new safety review and the hardware evidence in PROTOCOL.md. This release's boundary must not be used to justify later hardware writes.
