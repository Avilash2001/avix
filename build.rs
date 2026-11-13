//! Build script: embeds Windows resources in Avix.exe and works around the GNU toolchain.
//!
//! GNU builds get one COFF object with a `.rsrc` section holding:
//! - the application manifest (DPI awareness, modern controls, supported OSes, asInvoker; Avix
//!   elevates itself at runtime when it needs to, so it can also run in a limited mode),
//! - the app icon in 8 sizes (drawn by `src/icon_render.rs`, the same code as the tray icon),
//! - version info (product name, version, copyright), which Explorer, Task Manager and code
//!   signing (SignPath) read.
//!
//! It's written directly, the way the `embed-manifest` crate writes its manifest-only object,
//! so no resource compiler (rc.exe / windres) is needed: rustup's bundled MinGW is enough.
//! MSVC builds keep embed-manifest's manifest only.

#[path = "src/icon_render.rs"]
#[allow(dead_code)]
mod icon_render;

use std::path::PathBuf;

const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;
const RT_VERSION: u32 = 16;
const RT_MANIFEST: u32 = 24;
const LANG_EN_US: u32 = 1033;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/icon_render.rs");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest = embed_manifest::new_manifest("Avix.Optimizer");

    let gnu = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    let x64 = std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64");
    if gnu && x64 {
        let mut res: Vec<(u32, u32, Vec<u8>)> = Vec::new();
        res.push((RT_MANIFEST, 1, manifest.to_string().into_bytes()));
        let (icons, group) = icons();
        for (i, img) in icons.into_iter().enumerate() {
            res.push((RT_ICON, i as u32 + 1, img));
        }
        res.push((RT_GROUP_ICON, 1, group));
        res.push((RT_VERSION, 1, version_info()));
        let obj = coff_resources(&res);
        let path = out.join("avix-resources.o");
        std::fs::write(&path, obj).unwrap();
        println!("cargo:rustc-link-arg-bins={}", path.display());

        // rustup's minimal MinGW runtime ships no libshlwapi.a, but the `webbrowser` crate (pulled
        // in by eframe) links `-lshlwapi` for one function. An empty archive satisfies the flag;
        // the symbol itself resolves from the windows-targets import library already linked.
        let stub = out.join("stub-libs");
        std::fs::create_dir_all(&stub).unwrap();
        std::fs::write(stub.join("libshlwapi.a"), b"!<arch>\n").unwrap();
        println!("cargo:rustc-link-search=native={}", stub.display());
    } else {
        embed_manifest::embed_manifest(manifest).expect("unable to embed manifest");
    }
}

// ------------------------------------------------------------------ icon

/// RT_ICON images (bitmaps for the small sizes, PNG for 256 px) and the RT_GROUP_ICON directory.
fn icons() -> (Vec<Vec<u8>>, Vec<u8>) {
    const SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
    let mut images = Vec::new();
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes()); // reserved
    group.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    group.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    for (i, &s) in SIZES.iter().enumerate() {
        let rgba = icon_render::render_rgba(s, icon_render::IconState::Idle);
        let img = if s >= 256 { png(s, &rgba) } else { dib(s, &rgba) };
        // GRPICONDIRENTRY (14 bytes): width/height 0 means 256
        group.push(if s >= 256 { 0 } else { s as u8 });
        group.push(if s >= 256 { 0 } else { s as u8 });
        group.push(0); // colour count
        group.push(0); // reserved
        group.extend_from_slice(&1u16.to_le_bytes()); // planes
        group.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        group.extend_from_slice(&(img.len() as u32).to_le_bytes());
        group.extend_from_slice(&(i as u16 + 1).to_le_bytes()); // RT_ICON id
        images.push(img);
    }
    (images, group)
}

/// 32-bit BGRA bitmap with AND mask, as stored inside .ico files (bottom-up rows).
fn dib(s: u32, rgba: &[u8]) -> Vec<u8> {
    let mask_row = s.div_ceil(32) * 4;
    let mut v = Vec::new();
    for x in [40u32, s, s * 2] {
        v.extend_from_slice(&x.to_le_bytes()); // biSize, biWidth, biHeight (colour + mask)
    }
    v.extend_from_slice(&1u16.to_le_bytes()); // planes
    v.extend_from_slice(&32u16.to_le_bytes()); // bit count
    v.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    v.extend_from_slice(&(s * s * 4 + mask_row * s).to_le_bytes());
    v.extend_from_slice(&[0; 16]); // resolution, colours used/important
    for y in (0..s).rev() {
        for x in 0..s {
            let i = ((y * s + x) * 4) as usize;
            v.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    for y in (0..s).rev() {
        let mut row = vec![0u8; mask_row as usize];
        for x in 0..s {
            if rgba[((y * s + x) * 4 + 3) as usize] == 0 {
                row[(x / 8) as usize] |= 0x80 >> (x % 8); // 1 = transparent
            }
        }
        v.extend_from_slice(&row);
    }
    v
}

fn png(s: u32, rgba: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, s, s);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(rgba).unwrap();
    }
    buf
}

// ------------------------------------------------------------------ version info

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

fn pad4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// One VERSIONINFO block: header, key, value, then child blocks (each 32-bit aligned).
fn block(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![0, 0];
    v.extend_from_slice(&value_len.to_le_bytes());
    v.extend_from_slice(&u16::from(text).to_le_bytes());
    v.extend_from_slice(&utf16z(key));
    pad4(&mut v);
    v.extend_from_slice(value);
    for c in children {
        pad4(&mut v);
        v.extend_from_slice(c);
    }
    let len = v.len() as u16;
    v[0..2].copy_from_slice(&len.to_le_bytes());
    v
}

fn version_info() -> Vec<u8> {
    let num = |k: &str| std::env::var(k).unwrap().parse::<u32>().unwrap();
    let (major, minor, patch) = (num("CARGO_PKG_VERSION_MAJOR"), num("CARGO_PKG_VERSION_MINOR"), num("CARGO_PKG_VERSION_PATCH"));
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let ms = (major << 16) | minor;
    let ls = patch << 16;

    let mut fixed = Vec::new();
    for x in [0xFEEF04BDu32, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        // signature, struct version, file version, product version, flags mask, flags,
        // OS (NT, Win32), type (app), subtype, date
        fixed.extend_from_slice(&x.to_le_bytes());
    }

    let strings = [
        ("CompanyName", "Avilash Ghosh"),
        ("FileDescription", "Avix"),
        ("FileVersion", version.as_str()),
        ("InternalName", "avix"),
        ("LegalCopyright", "Copyright (c) 2026 Avilash Ghosh. MIT License."),
        ("OriginalFilename", "Avix.exe"),
        ("ProductName", "Avix"),
        ("ProductVersion", version.as_str()),
        ("Comments", "Real-time Windows performance optimizer and debloater. https://github.com/Avilash2001/avix"),
    ];
    let entries: Vec<Vec<u8>> = strings
        .iter()
        .map(|(k, v)| {
            let val = utf16z(v);
            block(k, &val, (val.len() / 2) as u16, true, &[])
        })
        .collect();
    let table = block("040904B0", &[], 0, true, &entries); // US English, Unicode
    let string_info = block("StringFileInfo", &[], 0, true, &[table]);
    let translation = block("Translation", &[0x09, 0x04, 0xB0, 0x04], 4, false, &[]);
    let var_info = block("VarFileInfo", &[], 0, true, &[translation]);
    block("VS_VERSION_INFO", &fixed, fixed.len() as u16, false, &[string_info, var_info])
}

// ------------------------------------------------------------------ COFF .rsrc object

/// A COFF object with one `.rsrc` section holding `resources` (type, id, data), all in US English.
/// Layout: type directory, name directories, language directories, data entries, then the data.
/// Each data entry's address is section-relative and gets an ADDR32NB relocation so the linker
/// turns it into an RVA.
fn coff_resources(resources: &[(u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut types: Vec<u32> = resources.iter().map(|r| r.0).collect();
    types.sort_unstable();
    types.dedup();
    let names = |t: u32| {
        let mut v: Vec<usize> = (0..resources.len()).filter(|&i| resources[i].0 == t).collect();
        v.sort_by_key(|&i| resources[i].1);
        v
    };
    let dir = |n: usize| 16 + 8 * n as u32;

    // offsets of every table
    let mut off = dir(types.len());
    let mut name_dir = Vec::new();
    for &t in &types {
        name_dir.push(off);
        off += dir(names(t).len());
    }
    let mut lang_dir = vec![0u32; resources.len()];
    for &t in &types {
        for i in names(t) {
            lang_dir[i] = off;
            off += dir(1);
        }
    }
    let mut data_entry = vec![0u32; resources.len()];
    for &t in &types {
        for i in names(t) {
            data_entry[i] = off;
            off += 16;
        }
    }
    let mut data_off = vec![0u32; resources.len()];
    for &t in &types {
        for i in names(t) {
            off = off.div_ceil(8) * 8;
            data_off[i] = off;
            off += resources[i].2.len() as u32;
        }
    }

    let table = |n: usize| {
        let mut v = vec![0u8; 16];
        v[14..16].copy_from_slice(&(n as u16).to_le_bytes());
        v
    };
    let entry = |id: u32, target: u32, subdir: bool| {
        let mut v = id.to_le_bytes().to_vec();
        v.extend_from_slice(&(target | if subdir { 0x8000_0000 } else { 0 }).to_le_bytes());
        v
    };
    let mut sec = Vec::new();
    sec.extend(table(types.len()));
    for (k, &t) in types.iter().enumerate() {
        sec.extend(entry(t, name_dir[k], true));
    }
    for &t in &types {
        let ns = names(t);
        sec.extend(table(ns.len()));
        for i in ns {
            sec.extend(entry(resources[i].1, lang_dir[i], true));
        }
    }
    for &t in &types {
        for i in names(t) {
            sec.extend(table(1));
            sec.extend(entry(LANG_EN_US, data_entry[i], false));
        }
    }
    let mut relocs = Vec::new();
    for &t in &types {
        for i in names(t) {
            relocs.push(sec.len() as u32);
            sec.extend_from_slice(&data_off[i].to_le_bytes());
            sec.extend_from_slice(&(resources[i].2.len() as u32).to_le_bytes());
            sec.extend_from_slice(&[0; 8]); // code page, reserved
        }
    }
    for &t in &types {
        for i in names(t) {
            sec.resize(data_off[i] as usize, 0);
            sec.extend_from_slice(&resources[i].2);
        }
    }
    sec.resize(sec.len().div_ceil(8) * 8, 0);

    // file header (20) + one section header (40), then raw data, relocations, symbols, strings
    let raw_ptr = 60u32;
    let reloc_ptr = raw_ptr + sec.len() as u32;
    let sym_ptr = reloc_ptr + 10 * relocs.len() as u32;
    let mut o = Vec::new();
    o.extend_from_slice(&0x8664u16.to_le_bytes()); // x86-64
    o.extend_from_slice(&1u16.to_le_bytes()); // sections
    o.extend_from_slice(&0u32.to_le_bytes()); // timestamp (0: reproducible)
    o.extend_from_slice(&sym_ptr.to_le_bytes());
    o.extend_from_slice(&2u32.to_le_bytes()); // symbols (section symbol + its aux record)
    o.extend_from_slice(&[0; 4]); // optional header size, characteristics
    o.extend_from_slice(b".rsrc\0\0\0");
    o.extend_from_slice(&[0; 8]); // virtual size, virtual address
    o.extend_from_slice(&(sec.len() as u32).to_le_bytes());
    o.extend_from_slice(&raw_ptr.to_le_bytes());
    o.extend_from_slice(&reloc_ptr.to_le_bytes());
    o.extend_from_slice(&[0; 4]); // line numbers
    o.extend_from_slice(&(relocs.len() as u16).to_le_bytes());
    o.extend_from_slice(&[0; 2]);
    o.extend_from_slice(&[0x40, 0, 0x30, 0xc0]); // initialized data, 4-byte align, read, write
    o.extend_from_slice(&sec);
    for r in &relocs {
        o.extend_from_slice(&r.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes()); // symbol 0: the section
        o.extend_from_slice(&3u16.to_le_bytes()); // IMAGE_REL_AMD64_ADDR32NB
    }
    o.extend_from_slice(b".rsrc\0\0\0");
    o.extend_from_slice(&[0; 4]); // value
    o.extend_from_slice(&1u16.to_le_bytes()); // section number
    o.extend_from_slice(&[0, 0, 3, 1]); // type, class static, 1 aux record
    o.extend_from_slice(&(sec.len() as u32).to_le_bytes());
    o.extend_from_slice(&(relocs.len() as u16).to_le_bytes());
    o.extend_from_slice(&[0; 12]);
    o.extend_from_slice(&4u32.to_le_bytes()); // empty string table
    o
}
