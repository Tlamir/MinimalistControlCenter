# Build

Install stable Rust and the Windows C++ build tools:

```powershell
cargo build --release --locked
```

Keep `target/release/mcc-ui.exe` and `mcc.exe` together. The installer is the recommended way to enable startup controls.

## Checks

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --release --all-targets --locked
powershell -NoProfile -ExecutionPolicy Bypass -File tests/startup.ps1
python scripts/audit_binary.py target/release/mcc.exe
python scripts/audit_binary.py target/release/mcc-ui.exe
```

Core tests also run on Linux. The Windows icon resource uses a dependency-free COFF builder; Pillow is only needed to regenerate the ICO with `scripts/make_icon.py`.

## Package

```powershell
python scripts/package.py --runtime-notices <rust-sysroot>/share/doc/rust/COPYRIGHT-library.html
```

The package contains verified binaries, installer scripts, documentation and license notices. Private OEM files and local reports are excluded.

To regenerate the benchmark charts, install Matplotlib and run `python scripts/plot_benchmarks.py`. It reads the published JSON measurements and writes `assets/benchmarks.svg` and `.png`.

Implementation references: [protocol](PROTOCOL.md), [safety](SAFETY.md), [owner checks](OWNER_CHECKS.md).
