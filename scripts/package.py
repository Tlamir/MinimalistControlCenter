"""Package an explicit release allowlist, with verified binaries and no private/tool files."""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import sys
import tomllib
import zipfile


def main():
    root = pathlib.Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bin-dir', type=pathlib.Path, default=root / 'target/release')
    parser.add_argument('--runtime-notices', type=pathlib.Path, required=True)
    parser.add_argument('--output', type=pathlib.Path, default=root / 'target/packages')
    args = parser.parse_args()
    version = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))['package']['version']
    binary = args.bin_dir / 'mcc.exe'
    result = subprocess.run([str(binary.resolve()), '--version'], check=True, capture_output=True, text=True, timeout=10)
    if result.stdout.strip() != f'mcc {version}':
        raise ValueError('Release binary version does not match Cargo.toml')
    if 'Rust Standard Library' not in args.runtime_notices.read_text(encoding='utf-8'):
        raise ValueError('Expected Rust standard library runtime notices')
    for name in ['mcc.exe', 'mcc-ui.exe']:
        subprocess.run([sys.executable, str(root / 'scripts/audit_binary.py'), str(args.bin_dir / name)],
                       check=True, capture_output=True, timeout=10)
    folder = args.output / f'mcc-{version}-windows-x64'
    folder.mkdir(parents=True, exist_ok=True)
    files = {
        'mcc.exe': binary,
        'mcc-ui.exe': args.bin_dir / 'mcc-ui.exe',
        'README.md': root / 'README.md',
        'icon.svg': root / 'icon.svg',
        'BENCHMARKS.md': root / 'docs/BENCHMARKS.md',
        'FAN_SUPPORT.md': root / 'docs/FAN_SUPPORT.md',
        'OWNER_CHECKS.md': root / 'docs/OWNER_CHECKS.md',
        'SAFETY.md': root / 'docs/SAFETY.md',
        'PROTOCOL.md': root / 'docs/PROTOCOL.md',
        'BUILD.md': root / 'docs/BUILD.md',
        'benchmarks.svg': root / 'assets/benchmarks-idle-cpu-1pct.svg',
        'LICENSE': root / 'LICENSE',
        'THIRD_PARTY_NOTICES.md': root / 'THIRD_PARTY_NOTICES.md',
        'RUST-RUNTIME-NOTICES.html': args.runtime_notices,
        'Install.ps1': root / 'scripts/install.ps1',
        'Install.cmd': root / 'scripts/Install.cmd',
        'Uninstall.ps1': root / 'scripts/uninstall.ps1',
    }
    benchmark_text = files['BENCHMARKS.md'].read_text(encoding='utf-8')
    report_link = re.search(r'\[JSON report\]\((\.\./reports/benchmarks/[\w.-]+\.json)\)', benchmark_text)
    if not report_link:
        raise ValueError('Benchmark documentation must link its sanitized report')
    report_path = (root / 'docs' / report_link.group(1)).resolve()
    if report_path.parent != (root / 'reports/benchmarks').resolve():
        raise ValueError('Benchmark report outside the public report directory')
    files['BENCHMARKS.json'] = report_path
    for name, source in files.items():
        shutil.copyfile(source, folder / name)
    # Installed documentation is flat, with all linked documents present alongside the README.
    readme = folder / 'README.md'
    readme.write_text(readme.read_text(encoding='utf-8').replace('](docs/', '](')
                      .replace('](assets/benchmarks-idle-cpu-1pct.svg)', '](benchmarks.svg)'), encoding='utf-8')
    bench = folder / 'BENCHMARKS.md'
    repo_url = 'https://github.com/Tlamir/MinimalistControlCenter/blob/main/'
    bench.write_text(benchmark_text.replace(report_link.group(1), 'BENCHMARKS.json')
                     .replace('../assets/benchmarks-idle-cpu-1pct.svg', 'benchmarks.svg')
                     .replace('../assets/', 'https://raw.githubusercontent.com/Tlamir/MinimalistControlCenter/main/assets/')
                     .replace('../reports/', repo_url + 'reports/')
                     .replace('../scripts/', repo_url + 'scripts/'), encoding='utf-8')
    (folder / 'VERSION').write_text(version + '\n', encoding='ascii')
    installed = ['mcc.exe', 'mcc-ui.exe', 'README.md', 'icon.svg', 'BENCHMARKS.md', 'FAN_SUPPORT.md',
                 'OWNER_CHECKS.md', 'SAFETY.md', 'PROTOCOL.md', 'BUILD.md', 'benchmarks.svg', 'BENCHMARKS.json', 'LICENSE', 'THIRD_PARTY_NOTICES.md',
                 'RUST-RUNTIME-NOTICES.html', 'Uninstall.ps1', 'VERSION']
    hashes = {name: hashlib.sha256((folder / name).read_bytes()).hexdigest() for name in installed}
    (folder / 'manifest.json').write_text(json.dumps(hashes, indent=2) + '\n', encoding='utf-8')
    archive = folder.parent / (folder.name + '.zip')
    names = [*files, 'VERSION', 'manifest.json']
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as release:
        for name in names:
            release.write(folder / name, f'{folder.name}/{name}')
    # Verify the archive, including exact allowlist and the manifest's installed bytes.
    with zipfile.ZipFile(archive) as release:
        if release.testzip() or set(release.namelist()) != {f'{folder.name}/{name}' for name in names}:
            raise ValueError('Release archive verification failed')
        for name, expected in hashes.items():
            if hashlib.sha256(release.read(f'{folder.name}/{name}')).hexdigest() != expected:
                raise ValueError(f'Archive hash mismatch: {name}')
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix('.zip.sha256').write_text(f'{digest}  {archive.name}\n', encoding='ascii')
    # A stable release asset name keeps the README's latest-download link working.
    download = archive.parent / 'mcc-windows-x64-installer.zip'
    shutil.copyfile(archive, download)
    download.with_suffix('.zip.sha256').write_text(f'{digest}  {download.name}\n', encoding='ascii')
    print(f'Packaged and verified: {archive}')
    print(f'SHA256: {digest}')


if __name__ == '__main__':
    main()
