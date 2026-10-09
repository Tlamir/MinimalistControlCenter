"""Inspect PE imports without executing the program. Defense in depth, not a safety proof."""
import json
import pathlib
import struct
import sys


def imports(path):
    data = pathlib.Path(path).read_bytes()

    def u16(offset):
        return struct.unpack_from('<H', data, offset)[0]

    def u32(offset):
        return struct.unpack_from('<I', data, offset)[0]

    if data[:2] != b'MZ':
        raise ValueError('Not a PE file')
    pe = u32(0x3C)
    if data[pe:pe + 4] != b'PE\0\0':
        raise ValueError('Invalid PE signature')
    optional = pe + 24
    if u16(optional) != 0x20B:
        raise ValueError('Expected 64-bit PE')
    section_start = optional + u16(pe + 20)
    sections = []
    for index in range(u16(pe + 6)):
        offset = section_start + index * 40
        sections.append((u32(offset + 12), u32(offset + 16), u32(offset + 20)))

    def file_offset(rva):
        for address, raw_size, raw_start in sections:
            if address <= rva < address + raw_size:
                return raw_start + rva - address
        raise ValueError(f'RVA outside file-backed sections: {rva}')

    def string(offset):
        end = data.index(b'\0', offset, min(len(data), offset + 1024))
        return data[offset:end].decode('ascii')

    table = file_offset(u32(optional + 120))
    found = {}
    for index in range(256):
        entry = table + index * 20
        original, _, _, name, first = struct.unpack_from('<IIIII', data, entry)
        if not any((original, name, first)):
            return found
        dll = string(file_offset(name))
        symbols = []
        thunk = file_offset(original or first)
        for item in range(65536):
            address = struct.unpack_from('<Q', data, thunk + item * 8)[0]
            if address == 0:
                break
            if address & (1 << 63):
                symbols.append(f'ordinal:{address & 0xFFFF}')
            else:
                symbols.append(string(file_offset(address) + 2))
        else:
            raise ValueError('Unterminated thunk table')
        found[dll] = symbols
    raise ValueError('Unterminated import table')


def main():
    found = imports(sys.argv[1])
    forbidden_functions = {
        'RegSetValueExW', 'RegSetValueExA',
        'RegSetKeyValueW', 'WriteProcessMemory',
        'PowerWriteACValueIndex', 'PowerWriteDCValueIndex',
        'CreateServiceW', 'ChangeServiceConfigW', 'NtLoadDriver',
    }
    forbidden_libraries = {'ecio.dll', 'smiodll.dll', 'inpoutx64.dll', 'winring0x64.dll'}
    errors = []
    for dll, symbols in found.items():
        if dll.lower() in forbidden_libraries:
            errors.append(dll)
        errors.extend(f'{dll}:{symbol}' for symbol in symbols if symbol in forbidden_functions)
    print(json.dumps({'imports': found, 'forbidden_imports': errors,
                      'scope': 'No driver installation/configuration, raw EC I/O libraries, remote memory writes or power-limit writes. HID, standard power-plan selection and reversible handover of the fixed OEM user-mode launcher are expected in v0.2.'}, indent=2))
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
