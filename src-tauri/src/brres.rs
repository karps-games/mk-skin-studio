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

#[derive(Debug, Clone, PartialEq)]
pub struct NameRef {
    pub at: usize,
    pub base: usize,
    pub text: String,
    origin: Option<usize>,
}

impl NameRef {
    pub fn new(at: usize, base: usize, text: &str) -> Self {
        NameRef { at, base, text: text.to_string(), origin: None }
    }

    pub fn moved(&self, at: usize, base: usize) -> Self {
        NameRef { at, base, ..self.clone() }
    }
}

#[derive(Debug, Clone)]
pub struct Member {
    pub folder: String,
    pub name: String,
    pub bytes: Vec<u8>,
    pub names: Vec<NameRef>,
}

#[derive(Debug, Clone)]
pub struct Archive {
    raw: Vec<u8>,
    pos: Vec<usize>,
    len: Vec<usize>,
    order: Vec<usize>,
    folders: Vec<String>,
    strings: std::collections::HashMap<String, usize>,
    pub members: Vec<Member>,
}

pub(crate) fn entries(b: &[u8], group: usize) -> Result<Vec<(usize, String, usize)>, String> {
    let size = be32(b, group)? as usize;
    let count = be32(b, group + 4)? as usize;
    if count > 4096 || 8 + (count + 1) * 16 > size || group + size > b.len() {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(count);
    for i in 1..=count {
        let e = group + 8 + i * 16;
        let name = cstr(b, offset(group, be32(b, e + 8)?, b.len())?)?;
        out.push((e, name, offset(group, be32(b, e + 12)?, b.len())?));
    }
    Ok(out)
}

fn string_at(b: &[u8], field: usize, base: usize) -> Result<(usize, String), String> {
    let target = offset(base, be32(b, field)?, b.len())?;
    let text = cstr(b, target)?;
    if target < 4 || be32(b, target - 4)? as usize != text.len() {
        return Err(bad());
    }
    Ok((target, text))
}

struct Names<'a> {
    b: &'a [u8],
    at: usize,
    out: Vec<NameRef>,
}

impl Names<'_> {
    fn add(&mut self, field: usize, base: usize, optional: bool) -> Result<(), String> {
        if optional && be32(self.b, field)? == 0 {
            return Ok(());
        }
        let (target, text) = string_at(self.b, field, base)?;
        self.out.push(NameRef { at: field - self.at, base: base - self.at, text, origin: Some(target) });
        Ok(())
    }

    fn group(&mut self, field: usize) -> Result<Vec<usize>, String> {
        let v = be32(self.b, field)?;
        if v == 0 {
            return Ok(Vec::new());
        }
        let g = offset(self.at, v, self.b.len())?;
        let mut items = Vec::new();
        for (e, _, data) in entries(self.b, g)? {
            self.add(e + 8, g, false)?;
            items.push(data);
        }
        Ok(items)
    }

    fn table(&mut self, field: usize, count: usize) -> Result<(), String> {
        if count == 0 {
            return Ok(());
        }
        let t = offset(self.at, be32(self.b, field)?, self.b.len())?;
        for i in 0..count {
            self.add(t + i * 4, t, false)?;
        }
        Ok(())
    }
}

fn member_names(b: &[u8], at: usize) -> Result<Vec<NameRef>, String> {
    let mut n = Names { b, at, out: Vec::new() };
    match (b.get(at..at + 4).ok_or_else(bad)?, be32(b, at + 8)?) {
        (b"TEX0", 3) => n.add(at + 0x14, at, false)?,
        (b"MDL0", 11) => {
            n.add(at + 0x48, at, false)?;
            n.add(at + 0x64, at + 0x4C, true)?;
            for i in 0..14 {
                for d in n.group(at + 0x10 + i * 4)? {
                    match i {
                        1 => n.add(d + 8, d, false)?,
                        2..=7 => n.add(d + 0x0C, d, false)?,
                        8 => {
                            n.add(d + 8, d, false)?;
                            let count = be32(b, d + 0x2C)? as usize;
                            if count > 8 {
                                return Err(bad());
                            }
                            if count > 0 {
                                let s = offset(d, be32(b, d + 0x30)?, b.len())?;
                                for k in 0..count {
                                    n.add(s + k * 0x34, s + k * 0x34, true)?;
                                    n.add(s + k * 0x34 + 4, s + k * 0x34, true)?;
                                }
                            }
                        }
                        10 => n.add(d + 0x38, d, false)?,
                        _ => {}
                    }
                }
            }
        }
        (b"CHR0", 5) | (b"SRT0", 5) => {
            n.add(at + 0x18, at, false)?;
            n.add(at + 0x1C, at, true)?;
            for d in n.group(at + 0x10)? {
                n.add(d, d, false)?;
            }
        }
        (b"PAT0", 4) => {
            n.add(at + 0x28, at, false)?;
            n.add(at + 0x2C, at, true)?;
            for d in n.group(at + 0x10)? {
                n.add(d, d, false)?;
            }
            n.table(at + 0x14, be16(b, at + 0x34)? as usize)?;
            n.table(at + 0x18, be16(b, at + 0x36)? as usize)?;
        }
        _ => return Err("E:file.format".into()),
    }
    Ok(n.out)
}

fn id_bit(name: &[u8], id: u16) -> bool {
    let c = (id >> 3) as usize;
    c < name.len() && (name[c] >> (id & 7)) & 1 == 1
}

fn top_bit(v: u8) -> u16 {
    7 - v.leading_zeros() as u16
}

fn node_id(other: &[u8], name: &[u8]) -> u16 {
    if other.len() < name.len() {
        return ((name.len() as u16 - 1) << 3) | top_bit(name[name.len() - 1]);
    }
    for i in (0..name.len()).rev() {
        let d = other[i] ^ name[i];
        if d != 0 {
            return ((i as u16) << 3) | top_bit(d);
        }
    }
    0xFFFF
}

pub fn dictionary(names: &[&[u8]]) -> Vec<[u16; 3]> {
    let mut all: Vec<&[u8]> = vec![b""];
    all.extend_from_slice(names);
    let mut n: Vec<[u16; 3]> = vec![[0xFFFF, 0, 0]];
    for idx in 1..all.len() {
        let name = all[idx];
        let mut id = node_id(b"", name);
        let (mut left, mut right) = (idx as u16, idx as u16);
        let mut prev = 0usize;
        let mut cur = n[0][1] as usize;
        let mut is_right = false;
        while id <= n[cur][0] && n[cur][0] < n[prev][0] {
            if id == n[cur][0] {
                id = node_id(all[cur], name);
                if id_bit(all[cur], id) {
                    (left, right) = (idx as u16, cur as u16);
                } else {
                    (left, right) = (cur as u16, idx as u16);
                }
            }
            prev = cur;
            is_right = id_bit(name, n[cur][0]);
            cur = n[cur][if is_right { 2 } else { 1 }] as usize;
        }
        if all[cur].len() == name.len() && id_bit(all[cur], id) {
            right = cur as u16;
        } else {
            left = cur as u16;
        }
        n.push([id, left, right]);
        n[prev][if is_right { 2 } else { 1 }] = idx as u16;
    }
    n
}

fn put(out: &mut [u8], at: usize, v: i64) {
    out[at..at + 4].copy_from_slice(&(v as i32).to_be_bytes());
}

impl Archive {
    pub fn parse(b: &[u8]) -> Result<Archive, String> {
        if b.len() < 16 || &b[..4] != b"bres" || be16(b, 4)? != 0xFEFF || be32(b, 8)? as usize != b.len() {
            return Err(bad());
        }
        let root = be16(b, 12)? as usize;
        if root != 16 || b.get(root..root + 4) != Some(b"root") {
            return Err(bad());
        }
        let root_end = root.checked_add(be32(b, root + 4)? as usize).filter(|&e| e <= b.len()).ok_or_else(bad)?;
        let mut a = Archive {
            raw: b.to_vec(),
            pos: Vec::new(),
            len: Vec::new(),
            order: Vec::new(),
            folders: Vec::new(),
            strings: std::collections::HashMap::new(),
            members: Vec::new(),
        };
        let top = root + 8;
        for (e, folder, sub) in entries(b, top)? {
            a.strings.entry(folder.clone()).or_insert(string_at(b, e + 8, top)?.0);
            a.folders.push(folder.clone());
            for (se, name, at) in entries(b, sub)? {
                a.strings.entry(name.clone()).or_insert(string_at(b, se + 8, sub)?.0);
                let size = be32(b, at + 4)? as usize;
                if at < root_end || size < 16 || at + size > b.len() || be32(b, at + 12)? as i32 as i64 != -(at as i64) {
                    return Err(bad());
                }
                let names = member_names(b, at)?;
                for r in &names {
                    a.strings.entry(r.text.clone()).or_insert(r.origin.unwrap_or(0));
                }
                a.members.push(Member { folder: folder.clone(), name, bytes: b[at..at + size].to_vec(), names });
                a.pos.push(at);
                a.len.push(size);
            }
        }
        if a.members.is_empty() {
            return Err(bad());
        }
        a.order = (0..a.members.len()).collect();
        a.order.sort_by_key(|&i| a.pos[i]);
        for w in a.order.windows(2) {
            if a.pos[w[0]] + a.len[w[0]] > a.pos[w[1]] {
                return Err(bad());
            }
        }
        Ok(a)
    }

    pub fn add_member(&mut self, folder: &str, name: &str, bytes: Vec<u8>, names: Vec<NameRef>) -> Result<(), String> {
        if self.members.iter().any(|m| m.folder == folder && m.name == name) {
            return Err("E:file.exists".into());
        }
        if !self.folders.iter().any(|f| f == folder) {
            self.folders.push(folder.to_string());
        }
        let at = self.order.iter().rposition(|&i| self.members[i].folder == folder).map_or(self.order.len(), |p| p + 1);
        self.order.insert(at, self.members.len());
        self.members.push(Member { folder: folder.to_string(), name: name.to_string(), bytes, names });
        Ok(())
    }

    pub fn add_texture(&mut self, name: &str, width: u32, height: u32, format: u32, images: u32, data: &[u8]) -> Result<(), String> {
        let template = self.members.iter().find(|m| m.bytes.starts_with(b"TEX0")).ok_or_else(bad)?;
        if data_len(format, width, height, images) != Some(data.len()) {
            return Err(bad());
        }
        let mut b = template.bytes[..0x40].to_vec();
        b[0x10..0x14].copy_from_slice(&0x40u32.to_be_bytes());
        b[0x18..0x1C].copy_from_slice(&0u32.to_be_bytes());
        b[0x1C..0x1E].copy_from_slice(&(width as u16).to_be_bytes());
        b[0x1E..0x20].copy_from_slice(&(height as u16).to_be_bytes());
        b[0x20..0x24].copy_from_slice(&format.to_be_bytes());
        b[0x24..0x28].copy_from_slice(&images.to_be_bytes());
        b[0x28..0x2C].copy_from_slice(&0f32.to_be_bytes());
        b[0x2C..0x30].copy_from_slice(&((images - 1) as f32).to_be_bytes());
        b.extend_from_slice(data);
        let folder = template.folder.clone();
        self.add_member(&folder, name, b, vec![NameRef::new(0x14, 0, name)])
    }

    pub fn write(&self) -> Vec<u8> {
        let raw = &self.raw;
        let groups: Vec<Vec<usize>> = self.folders.iter().map(|f| (0..self.members.len()).filter(|&i| &self.members[i].folder == f).collect()).collect();
        let mut out = raw[..16].to_vec();
        out.extend_from_slice(b"root\0\0\0\0");
        let top = out.len();
        let mut starts = vec![top];
        let mut at = top + 8 + (groups.len() + 1) * 16;
        for g in &groups {
            starts.push(at);
            at += 8 + (g.len() + 1) * 16;
        }
        out.resize(at, 0);
        let root_len = (at - 16) as i64;
        put(&mut out, 20, root_len);
        let first = self.pos[self.order.iter().copied().find(|&i| i < self.pos.len()).unwrap_or(0)];
        let mut new_pos = vec![0usize; self.members.len()];
        let mut end = first;
        for &i in &self.order {
            if i < self.pos.len() {
                if self.pos[i] > end {
                    out.extend_from_slice(&raw[end..self.pos[i]]);
                }
                while out.len() % 32 != self.pos[i] % 32 {
                    out.push(0);
                }
                end = self.pos[i] + self.len[i];
            } else {
                while out.len() % 32 != 0 {
                    out.push(0);
                }
            }
            new_pos[i] = out.len();
            out.extend_from_slice(&self.members[i].bytes);
        }
        while out.len() % 32 != end % 32 {
            out.push(0);
        }
        let shift = out.len() as i64 - end as i64;
        out.extend_from_slice(&raw[end..]);
        let mut added: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        let mut texts: Vec<&str> = self.folders.iter().map(|f| f.as_str()).collect();
        for m in &self.members {
            texts.push(&m.name);
            texts.extend(m.names.iter().filter(|r| r.origin.is_none()).map(|r| r.text.as_str()));
        }
        for t in texts {
            if !self.strings.contains_key(t) {
                added.insert(t, 0);
            }
        }
        for (text, at) in added.iter_mut() {
            while out.len() % 4 != 0 {
                out.push(0);
            }
            out.extend_from_slice(&(text.len() as u32).to_be_bytes());
            *at = out.len();
            out.extend_from_slice(text.as_bytes());
            out.push(0);
        }
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let string = |text: &str| -> i64 {
            match self.strings.get(text) {
                Some(&t) => t as i64 + shift,
                None => added[text] as i64,
            }
        };
        let len = out.len() as i64;
        put(&mut out, 8, len);
        put(&mut out, 12, ((16i64) << 16) | (self.members.len() as i64 + 1));
        let write_group = |out: &mut Vec<u8>, g: usize, items: &[(&str, usize)]| {
            let refs: Vec<&[u8]> = items.iter().map(|x| x.0.as_bytes()).collect();
            put(out, g, (8 + (items.len() + 1) * 16) as i64);
            put(out, g + 4, items.len() as i64);
            for (k, [id, left, right]) in dictionary(&refs).into_iter().enumerate() {
                let e = g + 8 + k * 16;
                out[e..e + 2].copy_from_slice(&id.to_be_bytes());
                out[e + 4..e + 6].copy_from_slice(&left.to_be_bytes());
                out[e + 6..e + 8].copy_from_slice(&right.to_be_bytes());
                if k > 0 {
                    let (text, target) = items[k - 1];
                    put(out, e + 8, string(text) - g as i64);
                    put(out, e + 12, target as i64 - g as i64);
                }
            }
        };
        let folders: Vec<(&str, usize)> = self.folders.iter().map(|f| f.as_str()).zip(starts[1..].iter().copied()).collect();
        write_group(&mut out, top, &folders);
        for (gi, g) in groups.iter().enumerate() {
            let items: Vec<(&str, usize)> = g.iter().map(|&i| (self.members[i].name.as_str(), new_pos[i])).collect();
            write_group(&mut out, starts[gi + 1], &items);
        }
        for (i, m) in self.members.iter().enumerate() {
            let p = new_pos[i];
            put(&mut out, p + 4, m.bytes.len() as i64);
            put(&mut out, p + 12, -(p as i64));
            for r in &m.names {
                let t = match r.origin {
                    Some(t) => t as i64 + shift,
                    None => string(&r.text),
                };
                put(&mut out, p + r.at, t - (p + r.base) as i64);
            }
        }
        out
    }
}

#[cfg(test)]
mod archive_tests {
    use super::*;

    fn tiny() -> Vec<u8> {
        let mut b = vec![0u8; 0x200];
        b[..4].copy_from_slice(b"bres");
        b[4..6].copy_from_slice(&0xFEFFu16.to_be_bytes());
        b[12..14].copy_from_slice(&0x10u16.to_be_bytes());
        b[14..16].copy_from_slice(&2u16.to_be_bytes());
        b[0x10..0x14].copy_from_slice(b"root");
        b[0x14..0x18].copy_from_slice(&0x58u32.to_be_bytes());
        let w = |b: &mut Vec<u8>, at: usize, v: i64| b[at..at + 4].copy_from_slice(&(v as i32).to_be_bytes());
        for g in [0x18usize, 0x40] {
            w(&mut b, g, 0x28);
            w(&mut b, g + 4, 1);
        }
        let strings = 0x100usize;
        let folder = "Textures(NW4R)";
        let tex = "skin";
        let (folder_at, tex_at) = (strings + 4, strings + 4 + 16 + 4);
        w(&mut b, strings, folder.len() as i64);
        b[folder_at..folder_at + folder.len()].copy_from_slice(folder.as_bytes());
        w(&mut b, strings + 20, tex.len() as i64);
        b[tex_at..tex_at + tex.len()].copy_from_slice(tex.as_bytes());
        w(&mut b, 0x18 + 8 + 16 + 8, folder_at as i64 - 0x18);
        w(&mut b, 0x18 + 8 + 16 + 12, 0x40 - 0x18);
        w(&mut b, 0x40 + 8 + 16 + 8, tex_at as i64 - 0x40);
        w(&mut b, 0x40 + 8 + 16 + 12, 0x80 - 0x40);
        let t = 0x80;
        b[t..t + 4].copy_from_slice(b"TEX0");
        w(&mut b, t + 4, 0x60);
        w(&mut b, t + 8, 3);
        w(&mut b, t + 12, -(t as i64));
        w(&mut b, t + 0x10, 0x40);
        w(&mut b, t + 0x14, tex_at as i64 - t as i64);
        b[t + 0x1C..t + 0x1E].copy_from_slice(&8u16.to_be_bytes());
        b[t + 0x1E..t + 0x20].copy_from_slice(&4u16.to_be_bytes());
        w(&mut b, t + 0x20, 1);
        w(&mut b, t + 0x24, 1);
        for (i, v) in b[t + 0x40..t + 0x60].iter_mut().enumerate() {
            *v = (i * 9) as u8;
        }
        b.truncate(0x120);
        let len = b.len() as i64;
        w(&mut b, 8, len);
        Archive::parse(&b).unwrap().write()
    }

    #[test]
    fn unchanged_archive_writes_identical_bytes() {
        let b = tiny();
        assert_eq!(textures(&b).unwrap().len(), 1);
        assert!(Archive::parse(&b).unwrap().write() == b);
    }

    #[test]
    fn grown_member_moves_strings_and_keeps_names() {
        let b = tiny();
        let mut a = Archive::parse(&b).unwrap();
        a.members[0].bytes.extend_from_slice(&[0u8; 40]);
        let out = a.write();
        assert_eq!(out.len() as u32, be32(&out, 8).unwrap());
        let before = textures(&b).unwrap();
        let after = textures(&out).unwrap();
        assert_eq!(after[0].name, "skin");
        assert_eq!(&out[after[0].data..after[0].data + after[0].len], &b[before[0].data..before[0].data + before[0].len]);
        assert_eq!(Archive::parse(&out).unwrap().members[0].names, a.members[0].names.iter().map(|r| NameRef { origin: Some(r.origin.unwrap() + 64), ..r.clone() }).collect::<Vec<_>>());
    }

    #[test]
    fn added_texture_is_indexed_and_readable() {
        let b = tiny();
        let mut a = Archive::parse(&b).unwrap();
        let data: Vec<u8> = (0..32).map(|i| (i * 7) as u8).collect();
        a.add_texture("gold", 8, 8, 14, 1, &data).unwrap();
        assert!(a.add_texture("gold", 8, 8, 14, 1, &data).is_err());
        let out = a.write();
        let t = textures(&out).unwrap();
        assert_eq!(t.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), ["skin", "gold"]);
        assert_eq!((t[1].width, t[1].height, t[1].format, t[1].images), (8, 8, 14, 1));
        assert!(out[t[1].data..t[1].data + t[1].len] == data[..]);
        let before = &textures(&b).unwrap()[0];
        assert!(out[t[0].data..t[0].data + t[0].len] == b[before.data..before.data + before.len]);
        assert_eq!(Archive::parse(&out).unwrap().write(), out);
    }

    #[test]
    fn new_names_are_appended() {
        let b = tiny();
        let mut a = Archive::parse(&b).unwrap();
        a.members[0].names[0] = NameRef::new(0x14, 0, "renamed");
        let out = a.write();
        let t = Archive::parse(&out);
        assert!(t.is_ok());
        assert_eq!(cstr(&out, offset(0x80, be32(&out, 0x80 + 0x14).unwrap(), out.len()).unwrap()).unwrap(), "renamed");
    }

    #[test]
    fn real_every_driver_archive_round_trips() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(std::path::Path::new(&p))) else {
            return;
        };
        let mut files: Vec<std::path::PathBuf> =
            std::fs::read_dir(dir.join("Race/Kart")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "szs")).collect();
        files.push(dir.join("Scene/Model/Driver.szs"));
        files.sort();
        let (mut count, mut members) = (0, 0);
        for f in &files {
            let raw = crate::yaz0::decode(&std::fs::read(f).unwrap()).unwrap();
            let arc = crate::u8arc::parse(&raw).unwrap();
            for e in arc.files.iter().filter(|e| e.path.ends_with(".brres")) {
                let b = &raw[e.offset..e.offset + e.size];
                let label = format!("{}:{}", f.file_name().unwrap().to_string_lossy(), e.path);
                let a = Archive::parse(b).unwrap_or_else(|err| panic!("{label} {err}"));
                assert!(a.write() == b, "{label} not identical");
                let mut grown = a.clone();
                for m in &mut grown.members {
                    m.bytes.extend_from_slice(&[0u8; 32]);
                }
                let out = grown.write();
                let again = Archive::parse(&out).unwrap_or_else(|err| panic!("{label} grown {err}"));
                for (x, y) in a.members.iter().zip(&again.members) {
                    let key = |m: &Member| m.names.iter().map(|r| (r.at, r.base, r.text.clone())).collect::<Vec<_>>();
                    assert_eq!(key(x), key(y), "{label}");
                }
                assert_eq!(format!("{:?}", crate::mdl0::parse_all(b)), format!("{:?}", crate::mdl0::parse_all(&out)), "{label}");
                assert_eq!(format!("{:?}", crate::texmap::pat0_tracks(b)), format!("{:?}", crate::texmap::pat0_tracks(&out)), "{label}");
                let (t0, t1) = (textures(b).unwrap(), textures(&out).unwrap());
                assert_eq!(t0.len(), t1.len(), "{label}");
                for (x, y) in t0.iter().zip(&t1) {
                    assert_eq!((&x.name, x.width, x.height, x.format, x.images), (&y.name, y.width, y.height, y.format, y.images));
                    assert!(b[x.data..x.data + x.len] == out[y.data..y.data + y.len], "{label} {}", x.name);
                }
                count += 1;
                members += a.members.len();
            }
        }
        println!("{count} archives, {members} members");
        assert!(count > 1000);
    }
}
