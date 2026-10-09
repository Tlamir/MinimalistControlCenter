"""Generate notices from locked Cargo metadata; no network or vendor files are used."""
import json
import pathlib
import sys

metadata = json.loads(pathlib.Path(sys.argv[1]).read_text(encoding='utf-8-sig'))
parts = ['# Third-party notices\n\nLocked Cargo dependencies, including build-time and platform-specific dependencies. The supplied license texts are reproduced below. Rust runtime notices must also accompany binary distributions.\n']
for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
    if package['source'] is None:
        continue
    directory = pathlib.Path(package['manifest_path']).parent
    licenses = sorted({p for pattern in ['LICENSE*', 'COPYING*', 'UNLICENSE*'] for p in directory.glob(pattern) if p.is_file()})
    parts.append(f"\n## {package['name']} {package['version']}\n\nDeclared license: {package.get('license')}\n")
    if not licenses:
        raise RuntimeError(f"Missing license text for {package['name']}; inspect the crate before packaging")
    for path in licenses:
        parts.append(f'\n### {path.name}\n\n' + path.read_text(encoding='utf-8') + '\n')
pathlib.Path('THIRD_PARTY_NOTICES.md').write_text(''.join(parts), encoding='utf-8')
print(f"Recorded notices for {len(metadata['packages']) - 1} locked packages")
