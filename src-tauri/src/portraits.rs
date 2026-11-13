use crate::texture::Rgba8;

fn bad() -> String { "E:file.format".into() }

fn slice(bytes: &[u8], at: usize, len: usize) -> Result<&[u8], String> {
    bytes.get(at..at.checked_add(len).ok_or_else(bad)?).ok_or_else(bad)
}

fn be32(bytes: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(slice(bytes, at, 4)?.try_into().map_err(|_| bad())?))
}

fn decode_tpl(bytes: &[u8]) -> Result<Rgba8, String> {
    if be32(bytes, 0)? != 0x0020af30 { return Err(bad()); }
    let count = be32(bytes, 4)? as usize;
    let table = be32(bytes, 8)? as usize;
    if count == 0 || count > 256 || table < 12 { return Err(bad()); }
    slice(bytes, table, count * 8)?;
    let header = be32(bytes, table)? as usize;
    if header < table + count * 8 { return Err(bad()); }
    let h = slice(bytes, header, 36)?;
    let height = u16::from_be_bytes([h[0], h[1]]) as u32;
    let width = u16::from_be_bytes([h[2], h[3]]) as u32;
    if width == 0 || height == 0 || width > 512 || height > 512 { return Err(bad()); }
    let format = be32(h, 4)?;
    if !matches!(format, 0..=6 | 14) { return Err(bad()); }
    let f = gctex::TextureFormat::from_u32(format).ok_or_else(bad)?;
    let size = gctex::compute_image_size(f, width, height) as usize;
    let data_at = be32(h, 8)? as usize;
    if data_at < header + 36 { return Err(bad()); }
    let data = slice(bytes, data_at, size)?;
    crate::texture::decode(data, width, height, format)
}

const HEADS: &[(&str, &str)] = &[
    ("bmr", "baby_mario"), ("blg", "baby_luigi"),
    ("bpc", "baby_peach"), ("bds", "baby_daisy"),
    ("ko", "kinopio"), ("kk", "kinopico"), ("nk", "noko"), ("ka", "karon"),
    ("mr", "mario"), ("lg", "luigi"), ("pc", "peach"), ("ds", "daisy"),
    ("ys", "yoshi"), ("ca", "catherine"), ("dd", "didy"), ("jr", "koopa_jr"),
    ("wr", "wario"), ("wl", "waluigi"), ("dk", "donky"), ("kp", "koopa"),
    ("kt", "teresa"), ("rs", "roseta"), ("fk", "funky"), ("bk", "hone_koopa"),
];

fn archive_budget(bytes: &[u8]) -> Result<(), String> {
    if be32(bytes, 4)? != 32 { return Err(bad()); }
    let count = be32(bytes, 40)? as usize;
    let header_size = be32(bytes, 8)? as usize;
    if count == 0 || count > 4096 || header_size > 1 << 20 { return Err(bad()); }
    let strings = 32 + count * 12;
    let end = 32 + header_size;
    if strings > end { return Err(bad()); }
    slice(bytes, 32, header_size)?;
    let mut directories = Vec::new();
    for i in 0..count {
        let at = 32 + i * 12;
        let word = be32(bytes, at)?;
        let name_at = strings + (word & 0x00ff_ffff) as usize;
        if name_at >= end { return Err(bad()); }
        let search_end = (name_at + 129).min(end);
        if !bytes[name_at..search_end].contains(&0) { return Err(bad()); }
        while directories.last().is_some_and(|last| *last <= i) { directories.pop(); }
        if word >> 24 == 1 {
            directories.push(be32(bytes, at + 8)? as usize);
            if directories.len() > 17 { return Err(bad()); }
        }
    }
    Ok(())
}

fn from_archive(bytes: &[u8]) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    if bytes.len() > crate::yaz0::MAX_DECODED || archive_budget(bytes).is_err() { return out; }
    let Ok(arc) = crate::u8arc::parse(bytes) else { return out; };
    for (code, stem) in HEADS {
        let name = format!("button/timg/tt_{stem}_64x64.tpl");
        let Some(file) = arc.find(&name) else { continue; };
        let Ok(data) = slice(bytes, file.offset, file.size) else { continue; };
        let Ok(img) = decode_tpl(data) else { continue; };
        if let Ok(url) = crate::texture::data_url(&img) { out.insert((*code).into(), url); }
    }
    out
}

pub const UI_ARCHIVE: &str = "Scene/UI/MenuSingle.szs";

pub fn load(root: &std::path::Path) -> std::collections::HashMap<String, String> {
    use std::io::Read;
    let read = || -> Option<Vec<u8>> {
        let file = std::fs::File::open(crate::game::resolve(root, UI_ARCHIVE)).ok()?;
        if file.metadata().ok()?.len() > crate::yaz0::MAX_DECODED as u64 { return None; }
        let mut bytes = Vec::new();
        file.take(crate::yaz0::MAX_DECODED as u64 + 1).read_to_end(&mut bytes).ok()?;
        (bytes.len() <= crate::yaz0::MAX_DECODED).then_some(bytes)
    };
    read().map(|bytes| from_szs(&bytes)).unwrap_or_default()
}

fn from_szs(bytes: &[u8]) -> std::collections::HashMap<String, String> {
    if bytes.len() > crate::yaz0::MAX_DECODED { return Default::default(); }
    if !crate::yaz0::is_yaz0(bytes) { return from_archive(bytes); }
    if validate_yaz0(bytes).is_err() { return Default::default(); }
    crate::yaz0::decode(bytes).map(|raw| from_archive(&raw)).unwrap_or_default()
}

fn validate_yaz0(bytes: &[u8]) -> Result<(), String> {
    let size = be32(bytes, 4)? as usize;
    if size == 0 || size > crate::yaz0::MAX_DECODED { return Err(bad()); }
    let (mut at, mut written) = (16, 0);
    while written < size {
        let flags = slice(bytes, at, 1)?[0];
        at += 1;
        for bit in (0..8).rev() {
            if written == size { break; }
            if flags & (1 << bit) != 0 {
                slice(bytes, at, 1)?;
                at += 1;
                written += 1;
            } else {
                let pair = slice(bytes, at, 2)?;
                at += 2;
                let distance = (((pair[0] as usize & 15) << 8) | pair[1] as usize) + 1;
                let mut len = pair[0] as usize >> 4;
                if len == 0 {
                    len = slice(bytes, at, 1)?[0] as usize + 18;
                    at += 1;
                } else { len += 2; }
                if distance > written || len > size - written { return Err(bad()); }
                written += len;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tpl(format: u32, width: u16, height: u16, payload: &[u8]) -> Vec<u8> {
        let mut b = vec![0; 64];
        b[0..4].copy_from_slice(&0x0020af30u32.to_be_bytes());
        b[4..8].copy_from_slice(&1u32.to_be_bytes());
        b[8..12].copy_from_slice(&12u32.to_be_bytes());
        b[12..16].copy_from_slice(&20u32.to_be_bytes());
        b[20..22].copy_from_slice(&height.to_be_bytes());
        b[22..24].copy_from_slice(&width.to_be_bytes());
        b[24..28].copy_from_slice(&format.to_be_bytes());
        b[28..32].copy_from_slice(&64u32.to_be_bytes());
        b.extend_from_slice(payload);
        b
    }

    fn archive(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        use crate::u8arc::{Node, write};
        let mut nodes = vec![Node { is_dir: true, name: String::new(), a: 0, b: (entries.len() + 3) as u32 },
            Node { is_dir: true, name: "button".into(), a: 0, b: (entries.len() + 3) as u32 },
            Node { is_dir: true, name: "timg".into(), a: 1, b: (entries.len() + 3) as u32 }];
        let mut payloads = vec![vec![], vec![], vec![]];
        for (name, data) in entries {
            nodes.push(Node { is_dir: false, name: (*name).into(), a: 0, b: 0 });
            payloads.push(data.clone());
        }
        write(&nodes, &payloads)
    }

    #[test]
    fn native_archive_selects_exact_menu_heads_and_skips_corrupt_member() {
        let good = tpl(5, 4, 4, &[0xff; 32]);
        let raw = archive(&[("tt_mario_64x64.tpl", good.clone()),
            ("tt_hone_koopa_64x64.tpl", good.clone()), ("tt_koopa_64x64.tpl", good),
            ("tt_funky_64x64.tpl", vec![0; 3])]);
        let heads = from_archive(&raw);
        assert_eq!(heads.len(), 3);
        assert!(heads["mr"].starts_with("data:image/png;base64,"));
        assert!(heads.contains_key("bk") && heads.contains_key("kp"));
        assert!(!heads.contains_key("fk") && !heads.contains_key("bmr"));
    }

    #[test]
    fn compressed_archive_loads_from_game_root_without_requiring_other_ui_files() {
        let scratch = std::env::var_os("MKSS_TEST_SCRATCH").expect("set MKSS_TEST_SCRATCH for filesystem tests");
        let root = std::path::PathBuf::from(scratch).join(format!("portraits-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("Scene/UI")).unwrap();
        assert!(load(&root).is_empty());
        let raw = archive(&[("tt_mario_64x64.tpl", tpl(5, 4, 4, &[0xff; 32]))]);
        let packed = crate::yaz0::encode(&raw).unwrap();
        assert_eq!(from_szs(&packed).len(), 1);
        std::fs::write(root.join(UI_ARCHIVE), packed).unwrap();
        assert_eq!(load(&root).len(), 1);
        std::fs::write(root.join(UI_ARCHIVE), b"broken optional UI").unwrap();
        assert!(load(&root).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "opt-in: MKSS_PORTRAIT_UI=owned MenuSingle.szs, MKSS_TEST_SCRATCH=output dir"]
    fn real_menu_single_exports_all_24_native_portraits() {
        use base64::Engine;
        let path = std::env::var_os("MKSS_PORTRAIT_UI").expect("set MKSS_PORTRAIT_UI");
        let bytes = std::fs::read(path).unwrap();
        let heads = from_szs(&bytes);
        assert_eq!(heads.len(), crate::game::CHARACTERS.len());
        let out = std::path::PathBuf::from(std::env::var_os("MKSS_TEST_SCRATCH").expect("set MKSS_TEST_SCRATCH")).join("native-portraits");
        std::fs::create_dir_all(&out).unwrap();
        let mut hashes = std::collections::HashSet::new();
        let mut manifest = Vec::new();
        let mut characters = Vec::new();
        for (code, name) in crate::game::CHARACTERS {
            let png = base64::engine::general_purpose::STANDARD.decode(heads[*code].strip_prefix("data:image/png;base64,").unwrap()).unwrap();
            let img = crate::texture::load(&png).unwrap();
            assert_eq!((img.width, img.height), (64, 64), "{code}");
            assert!(img.pixels.chunks_exact(4).any(|p| p[3] == 0), "transparent {code}");
            assert!(img.pixels.chunks_exact(4).any(|p| p[3] > 0), "visible {code}");
            assert!(hashes.insert(crate::export::sha256_hex(&png)), "distinct {code}");
            std::fs::write(out.join(format!("{code}.png")), &png).unwrap();
            let stem = HEADS.iter().find(|(c, _)| c == code).unwrap().1;
            manifest.push(serde_json::json!({"code":code,"name":name,"member":format!("button/timg/tt_{stem}_64x64.tpl"),"width":64,"height":64,"png":format!("{code}.png")}));
            characters.push(serde_json::json!({"code":code,"name":name,"files":0,"portrait":heads[*code]}));
        }
        std::fs::write(out.join("mapping.json"), serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
        std::fs::write(out.join("portraits.json"), serde_json::to_vec_pretty(&serde_json::json!({"characters":characters})).unwrap()).unwrap();
        println!("Decoded/exported {} distinct native portraits to {}", heads.len(), out.display());
    }

    #[test]
    fn ui_archive_budget_bounds_path_expansion_before_u8_parser() {
        let raw = archive(&[("tt_mario_64x64.tpl", tpl(5, 4, 4, &[0xff; 32]))]);
        assert!(archive_budget(&raw).is_ok());
        let mut many = raw.clone();
        many[40..44].copy_from_slice(&4097u32.to_be_bytes());
        assert!(archive_budget(&many).is_err());
        let name = "a".repeat(129);
        assert!(archive_budget(&archive(&[(&name, vec![])])).is_err());
        use crate::u8arc::{Node, write};
        let mut nodes = vec![Node { is_dir: true, name: String::new(), a: 0, b: 19 }];
        for i in 1..19 { nodes.push(Node { is_dir: true, name: "nested".into(), a: i - 1, b: 19 }); }
        assert!(archive_budget(&write(&nodes, &vec![vec![]; 19])).is_err());
    }

    #[test]
    fn tpl_rejects_payload_overlapping_its_header() {
        let mut b = tpl(5, 4, 4, &[0xff; 32]);
        b[28..32].copy_from_slice(&0u32.to_be_bytes());
        assert!(decode_tpl(&b).is_err());
    }

    #[test]
    fn malformed_tpl_headers_and_truncations_never_panic() {
        let b = tpl(5, 4, 4, &[0xff; 32]);
        for end in 0..b.len() { assert!(decode_tpl(&b[..end]).is_err(), "truncated at {end}"); }
        for (at, value) in [(0, 0), (4, 0), (4, 257), (8, u32::MAX), (12, u32::MAX), (24, 7), (24, 8), (24, 9), (24, 10), (24, u32::MAX), (28, u32::MAX)] {
            let mut bad = b.clone();
            bad[at..at+4].copy_from_slice(&value.to_be_bytes());
            assert!(decode_tpl(&bad).is_err(), "offset {at} value {value}");
        }
        for dims in [(0u16, 4u16), (4, 0), (513, 4), (4, 513), (65535, 65535)] {
            assert!(decode_tpl(&tpl(5, dims.0, dims.1, &[0xff; 32])).is_err());
        }
    }

    #[test]
    fn malformed_yaz0_backreferences_sizes_and_truncations_are_rejected() {
        let raw = archive(&[("tt_mario_64x64.tpl", tpl(5, 4, 4, &[0xff; 32]))]);
        let packed = crate::yaz0::encode(&raw).unwrap();
        for end in 0..packed.len() { assert!(from_szs(&packed[..end]).is_empty()); }
        for (size, stream) in [(0u32, vec![]), (u32::MAX, vec![]), (16, vec![0, 0x10, 0]), (1, vec![0x80]), (2, vec![0x80, 42, 0x10, 0])] {
            let mut b = b"Yaz0".to_vec();
            b.extend_from_slice(&size.to_be_bytes());
            b.extend_from_slice(&[0; 8]);
            b.extend(stream);
            assert!(from_szs(&b).is_empty());
        }
        assert!(from_szs(&[]).is_empty());
    }

    #[test]
    fn rgb5a3_tpl_decodes_opaque_red_and_transparent_black() {
        let mut data = vec![0; 32];
        data[..2].copy_from_slice(&0xfc00u16.to_be_bytes());
        let img = decode_tpl(&tpl(5, 4, 4, &data)).unwrap();
        assert_eq!((img.width, img.height), (4, 4));
        assert_eq!(&img.pixels[..8], &[255, 0, 0, 255, 0, 0, 0, 0]);
    }
}
