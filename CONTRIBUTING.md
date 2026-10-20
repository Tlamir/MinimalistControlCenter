# Contributing

Start with an issue describing the behavior or proposed change. Hardware support needs evidence for the exact model and BIOS; a related chassis name is not sufficient.

## Report a problem or another laptop

Use the bug or hardware-support issue template. From the extracted release directory, run this without administrator access:

```powershell
.\mcc.exe diagnostics > diagnostics.json
```

Review the JSON before attaching it. It contains the app version, OS/architecture, manufacturer, model, BIOS, CPU and GPU names. It omits machine identifiers and paths, performs no EC transactions, and does not enable controls. Unknown EC firmware stays unknown. Older releases can provide `mcc.exe status --json` instead.

For a Windows Security block, include the exact detection name, affected path, version, SHA256 hash and time. Distinguish the installer, MCC executables and unrelated command-line detections. Do not restore a quarantined file just to gather a report. Never post private keys, serial numbers or raw OEM dumps.

## Development checks

See [build instructions](docs/BUILD.md) for the Windows toolchain and signing process.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo test --all-targets --release --locked
python -m unittest discover -s tests -p test_package.py
cargo audit --file Cargo.lock
```

Install the dependency auditor with `cargo install cargo-audit --locked`. Windows checks additionally run `tests/startup.ps1`, `tests/signatures.ps1` and the PE import audits documented in BUILD.md. Core protocol tests use the private `Ec` trait and fake implementations; tests must not write to real hardware. Use native demo/preview modes for UI changes and label simulated readings.

## Hardware changes

Read [safety](docs/SAFETY.md), [protocol](docs/PROTOCOL.md) and [fan support](docs/FAN_SUPPORT.md) first. Preserve exact identity/capability/ownership checks, bounded helpers and readback. Missing or stale readings must remain unknown. Add regression tests for failures and rejected requests when changing control logic.

Do not broaden the allowlist from a diagnostic report alone. Fixed speeds, custom tables, software fan curves, raw EC access and new drivers require independent validation. No deliberate overheating, forced restart or removal of OEM recovery software is required for a contribution.

## Releases

Sign and timestamp both executables and the install/uninstall scripts before packaging so hashes describe the signed bytes. CI ZIPs are unsigned development artifacts. Publishing and installer changes require separate release validation, including rollback, startup, upgrade and uninstall on Windows. Winget submission should follow a stable validated installer and published checksum.
