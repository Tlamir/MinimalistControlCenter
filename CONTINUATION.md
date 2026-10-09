# Continuation checkpoint — 9 October 2026

User priority: a genuinely lightweight, reliable replacement for Monster Control Center, especially fan behavior. Safety takes precedence over feature count. User authorized implementation, tests, benchmarks and a public GitHub repository. User also requested a pushed checkpoint before session limits interrupt work.

## Implemented and locally verified

- Rust v0.1 diagnostic CLI: status, bounded watch, opt-in WMI metadata discovery, offline simulator, benchmark.
- 37 library tests plus 5 actual-executable integration tests passed in debug and release; strict Clippy passed.
- Standalone x64 GNU/LLVM executable runs without the local toolchain on PATH; approximately 560 KiB.
- Initial warm CLI median around 20 ms; offline policy median around 19 ns/evaluation. These are NOT OEM comparison or thermal results. See docs/BENCHMARKS.md for finalized measurements.
- Windows binary import audit passed. CI is configured for Windows/MSVC and Linux.
- Public repository: https://github.com/Tlamir/MinimalistControlCenter

## Non-negotiable current boundary

NO EC transactions, vendor WMI method invocation, fan/power writes, OEM DLL execution, driver/service installation or firmware changes. Control commands are absent. Leave OEM/firmware cooling in place. Simulator decisions never reach hardware; illustrative thresholds are not approved laptop settings. Future code that adds control requires a new review and hardware validation.

## Hardware / protocol evidence

- Local model: MONSTER TULPAR T5 V17.1; BIOS N.1.06; i7-8750H; GTX 1060 + UHD 630.
- Exact GK5CN6Z identity and EC revision still not confirmed independently.
- Local metadata confirms `AcpiTest_MULong` and `AcpiTest_EventULong`; `GetSetULong` takes UInt64 Data and exposes UInt32 Return. No invocation performed.
- Static decompilation found firmware-dependent fan scaling, multiple table variants, shared read/write transport and fan-mode side effects on CPU power limits. See docs/PROTOCOL.md.
- Native CPU sensor path, GPU sensor backend, RPM scaling, firmware failover and safe ownership transfer remain unresolved.

## Local-only investigation material

Ignored `private/` contains OEM decompiler output, WMI schema, import audit and dependency metadata. Do not publish vendor decompiled implementation. Ignored `.tools/` contains project-local Rust, GitHub CLI and downloads. Ignored `reports/local/` contains raw measurements. No proprietary binaries were copied into tracked files.

Rust is installed under `.tools/rustup/toolchains/stable-x86_64-pc-windows-gnullvm/bin`. Set CARGO_HOME to project `.tools/cargo`, RUSTUP_HOME to `.tools/rustup`, RUSTC to the direct rustc.exe, and prepend that toolchain bin directory to PATH. Rustup shims had trouble resolving paths inside the restricted runner; the direct toolchain worked with approved execution. No system PATH change was made.

## Next work

1. Confirm published head and CI status; read benchmark results before repeating tests or measurements.
2. Review narrowly allowlisted read payloads against static evidence and local schema, but do not implement writes while sensor identity and recovery remain unresolved.
3. Establish CPU/GPU temperature and fan-RPM sources with explicit freshness/validity rules. Decide whether native NVAPI can supply GPU telemetry without unnecessary GPU wakeups.
4. Verify exact firmware family, autonomous fan tables and recovery behavior before considering any replacement controller. Do not promise independent fans, arbitrary temperature breakpoints or safe manual control without evidence.
5. Build the minimal native interface only once telemetry is trustworthy; longer-term plan is in PLAN.md.

This checkpoint is a completed diagnostic milestone, not a completed fan-controller replacement. Do not describe simulator tests or low diagnostic overhead as proof that real fan control is safe or better.
