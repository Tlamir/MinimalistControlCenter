//! Embed an original icon as a Windows COFF resource, without an SDK or build dependencies.
use std::{env, fs, path::PathBuf};
fn u16_at(b: &[u8], n: usize) -> u16 {
    u16::from_le_bytes(b[n..n + 2].try_into().unwrap())
}
fn u32_at(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn put16(b: &mut [u8], n: usize, v: u16) {
    b[n..n + 2].copy_from_slice(&v.to_le_bytes());
}
fn put32(b: &mut [u8], n: usize, v: u32) {
    b[n..n + 4].copy_from_slice(&v.to_le_bytes());
}
fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ARCH").unwrap(),
        "x86_64",
        "Windows release is x64"
    );
    let ico = fs::read("assets/app.ico").unwrap();
    let n = u16_at(&ico, 4) as usize;
    assert!((1..=16).contains(&n));
    let mut group = ico[..6].to_vec();
    let mut icons = Vec::new();
    for i in 0..n {
        let pos = 6 + i * 16;
        group.extend_from_slice(&ico[pos..pos + 12]);
        group.extend_from_slice(&((i + 1) as u16).to_le_bytes());
        let start = u32_at(&ico, pos + 12) as usize;
        let len = u32_at(&ico, pos + 8) as usize;
        icons.push(ico[start..start + len].to_vec());
    }
    // Root -> numeric type -> numeric resource ID -> language 0 -> data entry.
    let types = [(3u32, icons), (14u32, vec![group])];
    let mut data = vec![0u8; 32];
    put16(&mut data, 14, 2);
    let mut relocations = Vec::new();
    for (index, (kind, payloads)) in types.into_iter().enumerate() {
        let dir = data.len();
        put32(&mut data, 16 + index * 8, kind);
        put32(&mut data, 20 + index * 8, dir as u32 | 0x8000_0000);
        data.resize(dir + 16 + payloads.len() * 8, 0);
        put16(&mut data, dir + 14, payloads.len() as u16);
        for (i, payload) in payloads.into_iter().enumerate() {
            let lang = data.len();
            put32(&mut data, dir + 16 + i * 8, (i + 1) as u32);
            put32(&mut data, dir + 20 + i * 8, lang as u32 | 0x8000_0000);
            data.resize(lang + 24, 0);
            put16(&mut data, lang + 14, 1);
            let entry = data.len();
            put32(&mut data, lang + 20, entry as u32);
            data.resize(entry + 16, 0);
            let start = data.len();
            put32(&mut data, entry, start as u32);
            put32(&mut data, entry + 4, payload.len() as u32);
            relocations.push(entry as u32);
            data.extend_from_slice(&payload);
            while !data.len().is_multiple_of(4) {
                data.push(0);
            }
        }
    }
    let mut obj = vec![0u8; 60];
    put16(&mut obj, 0, 0x8664);
    put16(&mut obj, 2, 1);
    put32(
        &mut obj,
        8,
        (60 + data.len() + relocations.len() * 10) as u32,
    );
    put32(&mut obj, 12, 1);
    obj[20..25].copy_from_slice(b".rsrc");
    put32(&mut obj, 36, data.len() as u32);
    put32(&mut obj, 40, 60);
    put32(&mut obj, 44, (60 + data.len()) as u32);
    put16(&mut obj, 52, relocations.len() as u16);
    put32(&mut obj, 56, 0x4030_0040); // Readable initialized data, alignment 4.
    obj.extend_from_slice(&data);
    for r in relocations {
        obj.extend_from_slice(&r.to_le_bytes());
        obj.extend_from_slice(&0u32.to_le_bytes());
        obj.extend_from_slice(&3u16.to_le_bytes()); // IMAGE_REL_AMD64_ADDR32NB
    }
    let mut symbol = [0u8; 18];
    symbol[..5].copy_from_slice(b".rsrc");
    put16(&mut symbol, 12, 1);
    symbol[16] = 3; // Section-local static symbol.
    obj.extend_from_slice(&symbol);
    obj.extend_from_slice(&4u32.to_le_bytes());
    let path = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("app-icon.obj");
    fs::write(&path, obj).unwrap();
    println!("cargo:rustc-link-arg-bin=mcc-ui={}", path.display());
}
