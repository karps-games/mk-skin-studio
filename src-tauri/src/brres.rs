#[derive(Debug, Clone, PartialEq)]
pub struct Tex {
    pub name: String,
    pub section: usize,
    pub data: usize,
    pub len: usize,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub images: u32,
}

pub(crate) fn bad() -> String {
    "E:file.format".into()
}
pub(crate) fn be16(b: &[u8], at: usize) -> Result<u16, String> {
    b.get(at..at + 2).map(|s| u16::from_be_bytes(s.try_into().unwrap())).ok_or_else(bad)
}
pub(crate) fn be32(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_be_bytes(s.try_into().unwrap())).ok_or_else(bad)
}
pub(crate) fn offset(base: usize, rel: u32, len: usize) -> Result<usize, String> {
    let v = base as i64 + rel as i32 as i64;
    if v < 0 || v as usize >= len {
        return Err(bad());
    }
    Ok(v as usize)
}
pub(crate) fn cstr(b: &[u8], at: usize) -> Result<String, String> {
    let tail = b.get(at..).ok_or_else(bad)?;
    let n = tail.iter().take(256).position(|&c| c == 0).ok_or_else(bad)?;
    String::from_utf8(tail[..n].to_vec()).map_err(|_| bad())
}

pub(crate) fn group(b: &[u8], group: usize) -> Result<Vec<(String, usize)>, String> {
    let size = be32(b, group)? as usize;
    let count = be32(b, group + 4)? as usize;
    if count > 4096 || 8 + (count + 1) * 16 > size || group + size > b.len() {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(count);
    for i in 1..=count {
        let e = group + 8 + i * 16;
        let name = cstr(b, offset(group, be32(b, e + 8)?, b.len())?)?;
        let data = offset(group, be32(b, e + 12)?, b.len())?;
        out.push((name, data));
    }
    Ok(out)
}

pub fn is_paletted(format: u32) -> bool {
    matches!(format, 8..=10)
}

pub fn format_name(format: u32) -> &'static str {
    match format {
        0 => "I4",
        1 => "I8",
        2 => "IA4",
        3 => "IA8",
        4 => "RGB565",
        5 => "RGB5A3",
        6 => "RGBA8",
        8 => "C4",
        9 => "C8",
        10 => "C14X2",
        14 => "CMPR",
        _ => "?",
    }
}

pub fn data_len(format: u32, width: u32, height: u32, images: u32) -> Option<usize> {
    let f = gctex::TextureFormat::from_u32(format)?;
    if is_paletted(format) || format == 0xFF || width == 0 || height == 0 || width > 1024 || height > 1024 || images == 0 || images > 11 {
        return None;
    }
    Some(gctex::compute_image_size_mip(f, width, height, images) as usize)
}

pub fn textures(b: &[u8]) -> Result<Vec<Tex>, String> {
    if b.len() < 16 || &b[..4] != b"bres" || be16(b, 4)? != 0xFEFF {
        return Err(bad());
    }
    if be32(b, 8)? as usize != b.len() {
        return Err(bad());
    }
    let root = be16(b, 12)? as usize;
    if root < 16 || b.get(root..root + 4) != Some(b"root") {
        return Err(bad());
    }
    let root_len = be32(b, root + 4)? as usize;
    if root.checked_add(root_len).is_none_or(|e| e > b.len()) {
        return Err(bad());
    }
    let mut out = Vec::new();
    for (folder, sub) in group(b, root + 8)? {
        if folder != "Textures(NW4R)" {
            continue;
        }
        for (name, sec) in group(b, sub)? {
            if b.get(sec..sec + 4) != Some(b"TEX0") {
                return Err(bad());
            }
            let size = be32(b, sec + 4)? as usize;
            if size < 0x40 || sec + size > b.len() {
                return Err(bad());
            }
            let section = &b[sec..sec + size];
            let own_name = cstr(b, offset(sec, be32(section, 0x14)?, b.len())?)?;
            if own_name != name {
                return Err(bad());
            }
            let flags = be32(section, 0x18)?;
            let width = be16(section, 0x1C)? as u32;
            let height = be16(section, 0x1E)? as u32;
            let format = be32(section, 0x20)?;
            let images = be32(section, 0x24)?;
            let data_rel = be32(section, 0x10)? as usize;
            if flags & 1 != 0 || is_paletted(format) {
                continue;
            }
            let len = data_len(format, width, height, images).ok_or_else(bad)?;
            if data_rel < 0x30 || data_rel.checked_add(len).is_none_or(|e| e > size) {
                return Err(bad());
            }
            out.push(Tex { name, section: sec, data: sec + data_rel, len, width, height, format, images });
        }
    }
    for (i, t) in out.iter().enumerate() {
        for u in &out[i + 1..] {
            if t.data < u.data + u.len && u.data < t.data + t.len {
                return Err(bad());
            }
        }
    }
    Ok(out)
}
