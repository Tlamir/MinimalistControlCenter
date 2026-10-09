"""Generate local import libraries for the SDK-less gnullvm toolchain, using OS exports.

Does not copy DLL implementation or load a DLL. MSVC CI uses the Windows SDK instead.
"""
import pathlib
import struct
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
out = root / '.tools' / 'windows-imports'
out.mkdir(parents=True, exist_ok=True)
linker = root / '.tools/rustup/toolchains/stable-x86_64-pc-windows-gnullvm/lib/rustlib/x86_64-pc-windows-gnullvm/bin/rust-lld.exe'
for name in ['advapi32', 'ole32', 'oleaut32']:
    data = pathlib.Path('C:/Windows/System32', name + '.dll').read_bytes()
    u16 = lambda p: struct.unpack_from('<H', data, p)[0]
    u32 = lambda p: struct.unpack_from('<I', data, p)[0]
    pe = u32(0x3c)
    optional = pe + 24
    if data[:2] != b'MZ' or u16(optional) != 0x20b:
        raise ValueError('Expected Windows PE64 system DLL')
    start = optional + u16(pe + 20)
    sections = [struct.unpack_from('<IIII', data, start + i * 40 + 8) for i in range(u16(pe + 6))]

    def offset(rva):
        for _, address, size, raw in sections:
            if address <= rva < address + size:
                return raw + rva - address
        raise ValueError('Invalid DLL export RVA')

    table = offset(u32(optional + 112))
    count, names = u32(table + 24), offset(u32(table + 32))
    exports = []
    for i in range(count):
        pos = offset(u32(names + 4 * i))
        exports.append(data[pos:data.index(b'\0', pos)].decode('ascii'))
    definition = out / (name + '.def')
    definition.write_text(f'LIBRARY {name}.dll\nEXPORTS\n' + '\n'.join(exports) + '\n', encoding='ascii')
    subprocess.run([str(linker), '-flavor', 'link', '/lib', '/machine:x64', f'/def:{definition}', f'/out:{out / ("lib" + name + ".a")}'], check=True)
    print(f'{name}: {len(exports)} symbols, local import library generated')
