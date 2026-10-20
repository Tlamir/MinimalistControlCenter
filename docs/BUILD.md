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

CI also installs `cargo-audit` with `cargo install cargo-audit --locked` and runs `cargo audit --file Cargo.lock` against the current RustSec advisory database. This checks known dependency vulnerabilities; it is separate from Defender malware detection and publisher signing.

## Package

Public release packaging requires valid, timestamped Authenticode signatures on both executables and the install/uninstall PowerShell scripts, all from the same certificate. Verification happens before executing the version check, and again on staged files before hashing and archiving. Signing must precede manifest/checksum generation because it changes file bytes.

For an explicitly unsigned local/CI development package:

```powershell
python scripts/package.py --allow-unsigned --runtime-notices <rust-sysroot>/share/doc/rust/COPYRIGHT-library.html
```

CI artifacts use this development option and are not trusted signed releases. The package contains verified binaries, installer scripts, documentation and license notices. Private OEM files and local reports are excluded.

## Release signing

Obtain a publicly trusted code-signing identity and Windows SDK SignTool. The helper uses an existing certificate/private key in the Windows `My` store (including supported hardware-backed keys); it does not create a certificate or change machine trust. Cloud signing services can instead produce the same four signed files in a staging directory.

[SignPath Foundation](https://signpath.org/) offers free signing for eligible open-source projects under its [terms](https://signpath.org/terms). Enrollment, approval and release-signing authorization are external prerequisites; this repository has no configured SignPath identity. After approval, use the service to sign the staged files and feed the downloaded signed files through the same packaging verification. No certificate or signed release is implied by these scripts.

```powershell
# Build/test first. Choose a fresh signing directory for each attempt.
.\scripts\sign-release.ps1 -CertificateThumbprint <publisher-thumbprint> -SignTool <absolute-path-to-signtool.exe>
$sysroot = (& rustc --print sysroot).Trim()
python scripts/package.py --bin-dir target/signed --script-dir target/signed --runtime-notices "$sysroot/share/doc/rust/COPYRIGHT-library.html"
```

The helper signs `mcc.exe`, `mcc-ui.exe`, `Install.ps1`, and `Uninstall.ps1` with SHA256 and an RFC3161 timestamp, then verifies Windows trust. Use `-LocalMachine` for a certificate in that store. `-TimestampUrl` supports a publisher's preferred timestamp service. No private key or certificate credentials belong in the repository. `Install.cmd` is a batch launcher and cannot carry an Authenticode signature; a signed native installer would be needed to replace that entry point.

The default packaging command rejects unsigned, tampered, untrusted, untimestamped or mixed-certificate inputs. Rebuilds require signing again. Self-signed certificates do not establish public publisher trust. A valid signature identifies the publisher but does not guarantee that a new download avoids [SmartScreen reputation warnings](https://learn.microsoft.com/windows/apps/package-and-deploy/smartscreen-reputation). See Microsoft's [Smart App Control signing requirements](https://learn.microsoft.com/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control).

Defender malware detections are separate from publisher warnings. Record the exact detection name, affected path, file hash and release version from Protection history before attributing a detection to MCC. A command-line detection against another program or repository does not establish a detection against either MCC executable. If a verified release is suspected of a false positive, submit the affected file through [Microsoft's software developer submission portal](https://www.microsoft.com/wdsi/filesubmission); a signature alone does not resolve a malware verdict.

To regenerate the benchmark charts, install Matplotlib and run `python scripts/plot_benchmarks.py`. It reads the published JSON measurements and writes `assets/benchmarks.svg` and `.png`.

Implementation references: [protocol](PROTOCOL.md), [safety](SAFETY.md), [owner checks](OWNER_CHECKS.md).
