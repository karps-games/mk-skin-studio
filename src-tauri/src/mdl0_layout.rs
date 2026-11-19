use crate::brres::{be32, dictionary, offset, NameRef};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Header,
    Info,
    Dictionary(usize),
    Item(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loc {
    pub chunk: usize,
    pub off: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[u8; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u16; 3]>,
}

fn get16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn set16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}

fn set32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}

fn quantize(vals: &[f32], ty: u32, div: u32, out: &mut Vec<u8>) -> Result<(), String> {
    let scale = (1u32 << div.min(31)) as f32;
    for &v in vals {
        let q = (v * scale).round();
        let fits = |lo: f32, hi: f32| (lo..=hi).contains(&q);
        match ty {
            0 if fits(0.0, 255.0) => out.push(q as u8),
            1 if fits(-128.0, 127.0) => out.push(q as i8 as u8),
            2 if fits(0.0, 65535.0) => out.extend_from_slice(&(q as u16).to_be_bytes()),
            3 if fits(-32768.0, 32767.0) => out.extend_from_slice(&(q as i16).to_be_bytes()),
            4 => out.extend_from_slice(&v.to_be_bytes()),
            _ => return Err("E:model.range".into()),
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub kinds: Vec<Kind>,
    pub chunks: Vec<Vec<u8>>,
    align: Vec<(usize, usize)>,
    links: Vec<[Loc; 3]>,
    names: Vec<(Loc, Loc, NameRef)>,
}

fn dict_items(b: &[u8], g: usize) -> Result<Vec<(usize, usize)>, String> {
    let size = be32(b, g)? as usize;
    let count = be32(b, g + 4)? as usize;
    if count > 4096 || 8 + (count + 1) * 16 > size || g + size > b.len() {
        return Err(crate::brres::bad());
    }
    (1..=count).map(|i| g + 8 + i * 16).map(|e| Ok((e, offset(g, be32(b, e + 12)?, b.len())?))).collect()
}

fn unsupported() -> String {
    "E:model.unsupported".into()
}

struct Scan<'a> {
    b: &'a [u8],
    starts: BTreeMap<usize, Kind>,
    links: Vec<[usize; 3]>,
}

impl Scan<'_> {
    fn link(&mut self, field: usize, base: usize) -> Result<usize, String> {
        let target = offset(base, be32(self.b, field)?, self.b.len())?;
        self.links.push([field, base, target]);
        Ok(target)
    }

    fn model_link(&mut self, item: usize) -> Result<(), String> {
        if be32(self.b, item + 4)? as i32 as i64 != -(item as i64) {
            return Err(crate::brres::bad());
        }
        self.link(item + 4, item).map(|_| ())
    }

    fn zero(&self, field: usize) -> Result<(), String> {
        if be32(self.b, field)? != 0 {
            return Err(unsupported());
        }
        Ok(())
    }
}

impl Layout {
    pub fn parse(b: &[u8], names: &[NameRef]) -> Result<Layout, String> {
        if b.len() < 0x90 || &b[..4] != b"MDL0" || be32(b, 8)? != 11 {
            return Err("E:model.version".into());
        }
        if be32(b, 4)? as usize != b.len() {
            return Err(crate::brres::bad());
        }
        let mut s = Scan { b, starts: BTreeMap::new(), links: Vec::new() };
        s.starts.insert(0, Kind::Header);
        s.starts.insert(0x4C, Kind::Info);
        let mut items: Vec<Vec<usize>> = vec![Vec::new(); 14];
        for (i, list) in items.iter_mut().enumerate() {
            if be32(b, 0x10 + i * 4)? == 0 {
                continue;
            }
            if matches!(i, 6 | 7 | 12 | 13) {
                return Err(unsupported());
            }
            let d = s.link(0x10 + i * 4, 0)?;
            s.starts.insert(d, Kind::Dictionary(i));
            for (e, data) in dict_items(b, d)? {
                s.links.push([e + 12, d, data]);
                s.starts.entry(data).or_insert(Kind::Item(i));
                if !list.contains(&data) {
                    list.push(data);
                }
            }
        }
        s.model_link(0x4C)?;
        s.link(0x4C + 0x24, 0x4C)?;
        for (i, list) in items.iter().enumerate() {
            for &at in list {
                if !matches!(i, 0 | 11) {
                    s.model_link(at)?;
                }
                match i {
                    1 => {
                        for f in [0x5C, 0x60, 0x64, 0x68] {
                            if be32(b, at + f)? != 0 {
                                s.link(at + f, at)?;
                            }
                        }
                        s.zero(at + 0x6C)?;
                    }
                    2..=5 => {
                        s.link(at + 8, at)?;
                    }
                    8 => {
                        let tev = s.link(at + 0x28, at)?;
                        s.starts.entry(tev).or_insert(Kind::Item(9));
                        if be32(b, at + 0x2C)? > 0 {
                            s.link(at + 0x30, at)?;
                        }
                        s.zero(at + 0x34)?;
                        s.zero(at + 0x38)?;
                        s.link(at + 0x3C, at)?;
                    }
                    10 => {
                        s.link(at + 0x20, at + 0x18)?;
                        s.link(at + 0x2C, at + 0x24)?;
                        s.link(at + 0x64, at)?;
                    }
                    11 => {
                        let n = be32(b, at)? as usize;
                        if n > 256 {
                            return Err(crate::brres::bad());
                        }
                        for k in 0..n {
                            s.link(at + 4 + k * 8, at)?;
                            s.link(at + 8 + k * 8, at)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        let starts: Vec<usize> = s.starts.keys().copied().collect();
        let loc = |p: usize| -> Result<Loc, String> {
            if p >= b.len() {
                return Err(crate::brres::bad());
            }
            let chunk = starts.partition_point(|&x| x <= p) - 1;
            Ok(Loc { chunk, off: p - starts[chunk] })
        };
        let mut links = Vec::with_capacity(s.links.len());
        for [f, base, t] in &s.links {
            links.push([loc(*f)?, loc(*base)?, loc(*t)?]);
        }
        let mut refs = Vec::with_capacity(names.len());
        for r in names {
            refs.push((loc(r.at)?, loc(r.base)?, r.clone()));
        }
        let mut chunks = Vec::with_capacity(starts.len());
        for (i, &at) in starts.iter().enumerate() {
            let end = starts.get(i + 1).copied().unwrap_or(b.len());
            chunks.push(b[at..end].to_vec());
        }
        Ok(Layout {
            kinds: s.starts.values().copied().collect(),
            align: starts.iter().map(|&p| (32, p % 32)).collect(),
            chunks,
            links,
            names: refs,
        })
    }

    pub fn items(&self, section: usize) -> Vec<(String, usize)> {
        let Some(d) = self.kinds.iter().position(|k| *k == Kind::Dictionary(section)) else {
            return Vec::new();
        };
        let mut out: Vec<(usize, String, usize)> = self
            .names
            .iter()
            .filter(|(f, _, _)| f.chunk == d && f.off >= 32 && (f.off - 32) % 16 == 0)
            .filter_map(|(f, _, r)| {
                let data = Loc { chunk: d, off: f.off + 4 };
                self.links.iter().find(|l| l[0] == data).map(|l| (f.off, r.text.clone(), l[2].chunk))
            })
            .collect();
        out.sort();
        out.into_iter().map(|(_, n, c)| (n, c)).collect()
    }

    fn shift(&mut self, chunk: usize, at: usize, by: usize) {
        let fix = |l: &mut Loc| {
            if l.chunk == chunk && l.off >= at {
                l.off += by;
            }
        };
        for l in &mut self.links {
            l.iter_mut().for_each(fix);
        }
        for (f, b, _) in &mut self.names {
            fix(f);
            fix(b);
        }
    }

    fn splice(&mut self, chunk: usize, at: usize, bytes: &[u8]) {
        self.shift(chunk, at, bytes.len());
        self.chunks[chunk].splice(at..at, bytes.iter().copied());
    }

    fn insert_chunk(&mut self, index: usize, kind: Kind, bytes: Vec<u8>, align: (usize, usize)) -> usize {
        let fix = |l: &mut Loc| {
            if l.chunk >= index {
                l.chunk += 1;
            }
        };
        for l in &mut self.links {
            l.iter_mut().for_each(fix);
        }
        for (f, b, _) in &mut self.names {
            fix(f);
            fix(b);
        }
        self.kinds.insert(index, kind);
        self.align.insert(index, align);
        self.chunks.insert(index, bytes);
        index
    }

    fn duplicate(&mut self, src: usize, index: usize) -> usize {
        self.insert_chunk(index, self.kinds[src], self.chunks[src].clone(), self.align[src]);
        let src = if src >= index { src + 1 } else { src };
        let own = |l: Loc| if l.chunk == src { Loc { chunk: index, off: l.off } } else { l };
        let links: Vec<[Loc; 3]> = self.links.iter().filter(|l| l[0].chunk == src).map(|l| [own(l[0]), own(l[1]), own(l[2])]).collect();
        let names: Vec<(Loc, Loc, NameRef)> = self.names.iter().filter(|n| n.0.chunk == src).map(|(f, b, r)| (own(*f), own(*b), r.clone())).collect();
        self.links.extend(links);
        self.names.extend(names);
        index
    }

    fn name_at(&self, chunk: usize, off: usize) -> Option<&str> {
        let at = Loc { chunk, off };
        self.names.iter().find(|n| n.0 == at).map(|n| n.2.text.as_str())
    }

    fn target(&self, chunk: usize, off: usize) -> Option<Loc> {
        let at = Loc { chunk, off };
        self.links.iter().find(|l| l[0] == at).map(|l| l[2])
    }

    fn draws(&self, shape: u16) -> Result<Vec<(usize, usize, Option<[u16; 3]>)>, String> {
        let mut out = Vec::new();
        for (_, d) in self.items(0) {
            let b = &self.chunks[d];
            let mut p = 0;
            let mut draw = None;
            loop {
                match b.get(p).copied().ok_or_else(crate::brres::bad)? {
                    0 => p += 1,
                    1 => break,
                    2 | 5 => p += 5,
                    3 => p += 4 + 6 * *b.get(p + 3).ok_or_else(crate::brres::bad)? as usize,
                    4 => {
                        if get16(b, p + 3) == shape {
                            draw = Some([get16(b, p + 1), get16(b, p + 5), b[p + 7] as u16]);
                        }
                        p += 8;
                    }
                    _ => return Err(unsupported()),
                }
            }
            out.push((d, p, draw));
        }
        Ok(out)
    }

    pub fn rigid_shapes(&self) -> Vec<(String, u32, String, String)> {
        let Some(info) = self.kinds.iter().position(|k| *k == Kind::Info) else { return Vec::new() };
        let Some(lut) = self.target(info, 0x24) else { return Vec::new() };
        let lb = &self.chunks[lut.chunk];
        let bones = self.items(1);
        let mats = self.items(8);
        let mut out = Vec::new();
        for (name, c) in self.items(10) {
            let b = &self.chunks[c];
            let (Ok(cur), Ok(verts), Ok(id)) = (be32(b, 8), be32(b, 0x40), be32(b, 0x3C)) else { continue };
            let Some(bone_id) = be32(lb, lut.off + 4 + cur as usize * 4).ok().filter(|_| (cur as i32) >= 0) else { continue };
            let bone = bones.iter().find(|x| be32(&self.chunks[x.1], 0x0C).ok() == Some(bone_id)).map(|x| x.0.clone());
            let mat = self.draws(id as u16).ok().and_then(|d| d.into_iter().find_map(|x| x.2)).and_then(|[m, _, _]| {
                mats.iter().find(|x| be32(&self.chunks[x.1], 0x0C).ok() == Some(m as u32)).map(|x| x.0.clone())
            });
            if let (Some(bone), Some(mat)) = (bone, mat) {
                out.push((name, verts, bone, mat));
            }
        }
        out
    }

    pub fn add_material(&mut self, template: &str, name: &str, texture: &str) -> Result<u16, String> {
        let mats = self.items(8);
        let src = mats.iter().find(|x| x.0 == template).map(|x| x.1).ok_or_else(unsupported)?;
        let last = mats.iter().map(|x| x.1).max().unwrap_or(src);
        let id = mats.len() as u16;
        let c = self.duplicate(src, last + 1);
        self.rename(c, 8, name);
        let n = be32(&self.chunks[c], 0x2C)? as usize;
        let samplers = self.target(c, 0x30).filter(|_| n > 0).ok_or_else(unsupported)?.off;
        self.rename(c, samplers, texture);
        let b = &mut self.chunks[c];
        set32(b, 0x0C, id as u32);
        set32(b, samplers + 0x18, 1);
        set32(b, samplers + 0x1C, 1);
        for (k, v) in [1.0f32, 1.0, 0.0, 0.0, 0.0].iter().enumerate() {
            b[0x1B0 + k * 4..0x1B4 + k * 4].copy_from_slice(&v.to_be_bytes());
        }
        self.add_entry(8, name, c)?;
        let tev = self.target(c, 0x28).ok_or_else(unsupported)?.chunk;
        self.add_entry(9, name, tev)?;
        for k in 0..n {
            let s = samplers + k * 0x34;
            let tex = self.name_at(c, s).ok_or_else(unsupported)?.to_string();
            let links = self.items(11);
            let item = match links.iter().find(|x| x.0 == tex) {
                Some(x) => x.1,
                None => {
                    let last = links.iter().map(|x| x.1).max().ok_or_else(unsupported)?;
                    let item = self.insert_chunk(last + 1, Kind::Item(11), vec![0u8; 4], (4, 0));
                    self.add_entry(11, &tex, item)?;
                    item
                }
            };
            let count = be32(&self.chunks[item], 0)? as usize;
            let at = 4 + count * 8;
            self.splice(item, at, &[0u8; 8]);
            set32(&mut self.chunks[item], 0, count as u32 + 1);
            let base = Loc { chunk: item, off: 0 };
            self.links.push([Loc { chunk: item, off: at }, base, Loc { chunk: c, off: 0 }]);
            self.links.push([Loc { chunk: item, off: at + 4 }, base, Loc { chunk: c, off: s }]);
        }
        Ok(id)
    }

    fn rename(&mut self, chunk: usize, off: usize, text: &str) {
        let at = Loc { chunk, off };
        if let Some(n) = self.names.iter_mut().find(|n| n.0 == at) {
            n.2 = NameRef::new(0, 0, text);
        }
    }

    fn add_entry(&mut self, section: usize, name: &str, target: usize) -> Result<(), String> {
        let d = self.kinds.iter().position(|k| *k == Kind::Dictionary(section)).ok_or_else(unsupported)?;
        let n = be32(&self.chunks[d], 4)? as usize;
        let at = 8 + (n + 1) * 16;
        self.splice(d, at, &[0u8; 16]);
        let c = &mut self.chunks[d];
        set32(c, 0, (8 + (n + 2) * 16) as u32);
        set32(c, 4, (n + 1) as u32);
        self.links.push([Loc { chunk: d, off: at + 12 }, Loc { chunk: d, off: 0 }, Loc { chunk: target, off: 0 }]);
        self.names.push((Loc { chunk: d, off: at + 8 }, Loc { chunk: d, off: 0 }, NameRef::new(0, 0, name)));
        let names: Vec<String> = self.items(section).into_iter().map(|(n, _)| n).collect();
        let refs: Vec<&[u8]> = names.iter().map(|n| n.as_bytes()).collect();
        let c = &mut self.chunks[d];
        for (k, [id, left, right]) in dictionary(&refs).into_iter().enumerate() {
            let e = 8 + k * 16;
            set16(c, e, id);
            set16(c, e + 4, left);
            set16(c, e + 6, right);
        }
        Ok(())
    }

    fn add_array(&mut self, section: usize, id: u16, name: &str, mesh: &Mesh) -> Result<(u16, u32), String> {
        let list = self.items(section);
        let src = list.iter().map(|x| x.1).find(|&c| be32(&self.chunks[c], 0x10).ok() == Some(id as u32)).ok_or_else(unsupported)?;
        let last = list.iter().map(|x| x.1).max().unwrap_or(src);
        let new_id = list.len() as u16;
        let c = self.duplicate(src, last + 1);
        self.rename(c, 0x0C, name);
        let b = &self.chunks[c];
        let data = be32(b, 8)? as usize;
        let (comp, ty, mut div) = (be32(b, 0x14)?, be32(b, 0x18)?, b[0x1C] as u32);
        let mut out = b[..data].to_vec();
        let count = match section {
            2 => {
                if comp != 1 {
                    return Err(unsupported());
                }
                let flat: Vec<f32> = mesh.positions.iter().flatten().copied().collect();
                let limit = match ty {
                    1 => 127.0,
                    3 => 32767.0,
                    _ => f32::MAX,
                };
                let peak = flat.iter().fold(0.0f32, |m, v| m.max(v.abs()));
                while div > 0 && peak * (1u32 << div) as f32 > limit {
                    div -= 1;
                }
                out[0x1C] = div as u8;
                quantize(&flat, ty, div, &mut out)?;
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for p in &mesh.positions {
                    for i in 0..3 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                }
                for (i, v) in lo.iter().chain(&hi).enumerate() {
                    out[0x20 + i * 4..0x24 + i * 4].copy_from_slice(&v.to_be_bytes());
                }
                mesh.positions.len()
            }
            3 => {
                let div = match ty {
                    1 => 6,
                    3 => 14,
                    _ => div,
                };
                if comp != 0 {
                    return Err(unsupported());
                }
                let flat: Vec<f32> = mesh.normals.iter().flatten().copied().collect();
                quantize(&flat, ty, div, &mut out)?;
                mesh.normals.len()
            }
            4 => {
                if comp != 1 || ty != 5 {
                    return Err(unsupported());
                }
                out.extend(mesh.colors.iter().flatten());
                mesh.colors.len()
            }
            _ => {
                if comp != 1 {
                    return Err(unsupported());
                }
                let flat: Vec<f32> = mesh.uvs.iter().flatten().copied().collect();
                quantize(&flat, ty, div, &mut out)?;
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                for p in &mesh.uvs {
                    for i in 0..2 {
                        lo[i] = lo[i].min(p[i]);
                        hi[i] = hi[i].max(p[i]);
                    }
                }
                for (i, v) in lo.iter().chain(&hi).enumerate() {
                    out[0x20 + i * 4..0x24 + i * 4].copy_from_slice(&v.to_be_bytes());
                }
                mesh.uvs.len()
            }
        };
        while out.len() % 32 != 0 {
            out.push(0);
        }
        let len = out.len() as u32;
        set32(&mut out, 0, len);
        set32(&mut out, 0x10, new_id as u32);
        set16(&mut out, 0x1E, count as u16);
        self.chunks[c] = out;
        self.add_entry(section, name, c)?;
        Ok((new_id, div))
    }

    pub fn add_rigid_mesh(&mut self, template: &str, name: &str, mesh: &Mesh, material: Option<u16>) -> Result<(), String> {
        let shapes = self.items(10);
        let ts = shapes.iter().find(|x| x.0 == template).map(|x| x.1).ok_or_else(unsupported)?;
        let t = self.chunks[ts].clone();
        let (lo, hi) = (be32(&t, 0x0C)?, be32(&t, 0x10)?);
        let attr = |v: u32, shift: u32| (v >> shift) & 3;
        if (be32(&t, 8)? as i32) < 0 || lo & 0x1FF != 0 || attr(lo, 15) != 0 || hi >> 2 != 0 || attr(lo, 9) == 0 {
            return Err(unsupported());
        }
        let used: Vec<(usize, usize, u32)> = [(2, 0x48, attr(lo, 9)), (3, 0x4A, attr(lo, 11)), (4, 0x4C, attr(lo, 13)), (5, 0x50, attr(hi, 0))]
            .into_iter()
            .filter(|u| u.2 != 0)
            .collect();
        if used.iter().any(|u| u.2 < 2) {
            return Err(unsupported());
        }
        let n = mesh.positions.len();
        let sizes = [(3, mesh.normals.len()), (4, mesh.colors.len()), (5, mesh.uvs.len())];
        if n == 0 || n > 255 || used.iter().any(|u| sizes.iter().any(|s| s.0 == u.0 && s.1 != n)) {
            return Err(unsupported());
        }
        let mut ids = Vec::new();
        let mut pos_div = 0;
        for &(section, field, _) in &used {
            let (id, div) = self.add_array(section, get16(&t, field), name, mesh)?;
            if section == 2 {
                pos_div = div;
            }
            ids.push((field, id));
        }
        let shapes = self.items(10);
        let ts = shapes.iter().find(|x| x.0 == template).map(|x| x.1).ok_or_else(unsupported)?;
        let last = shapes.iter().map(|x| x.1).max().unwrap_or(ts);
        let c = self.duplicate(ts, last + 1);
        self.rename(c, 0x38, name);
        let dl = self.links.iter().find(|l| l[0] == Loc { chunk: c, off: 0x2C }).map(|l| l[2].off).ok_or_else(unsupported)?;
        let setup = self.links.iter().find(|l| l[0] == Loc { chunk: c, off: 0x20 }).map(|l| l[2].off).ok_or_else(unsupported)?;
        let b = &mut self.chunks[c];
        let vat = b[dl] & 7;
        let reg = (setup..dl.saturating_sub(5)).find(|&p| b[p] == 0x08 && b[p + 1] == 0x70 | vat).ok_or_else(unsupported)?;
        let v = (be32(b, reg + 2)? & !(0x1F << 4)) | (pos_div << 4);
        set32(b, reg + 2, v);
        let mut prim = vec![0x90 | vat];
        prim.extend_from_slice(&((mesh.triangles.len() * 3) as u16).to_be_bytes());
        for t in &mesh.triangles {
            for &v in t {
                for &(_, _, kind) in &used {
                    if kind == 2 {
                        prim.push(v as u8);
                    } else {
                        prim.extend_from_slice(&v.to_be_bytes());
                    }
                }
            }
        }
        while prim.len() % 32 != 0 {
            prim.push(0);
        }
        b.truncate(dl);
        b.extend_from_slice(&prim);
        let len = b.len() as u32;
        set32(b, 0, len);
        set32(b, 0x24, prim.len() as u32);
        set32(b, 0x28, prim.len() as u32);
        set32(b, 0x3C, shapes.len() as u32);
        set32(b, 0x40, n as u32);
        set32(b, 0x44, mesh.triangles.len() as u32);
        for (field, id) in ids {
            set16(b, field, id);
        }
        self.add_entry(10, name, c)?;
        let tid = be32(&t, 0x3C)? as u16;
        let mut added = false;
        for (d, p, draw) in self.draws(tid)? {
            if let Some([mat, bone, prio]) = draw {
                let mut cmd = vec![4];
                cmd.extend_from_slice(&material.unwrap_or(mat).to_be_bytes());
                cmd.extend_from_slice(&(shapes.len() as u16).to_be_bytes());
                cmd.extend_from_slice(&bone.to_be_bytes());
                cmd.push(prio as u8);
                self.splice(d, p, &cmd);
                added = true;
            }
        }
        if !added {
            return Err(unsupported());
        }
        let info = self.kinds.iter().position(|k| *k == Kind::Info).ok_or_else(unsupported)?;
        let i = &mut self.chunks[info];
        let verts = be32(i, 0x10)? + n as u32;
        let tris = be32(i, 0x14)? + mesh.triangles.len() as u32;
        set32(i, 0x10, verts);
        set32(i, 0x14, tris);
        Ok(())
    }

    pub fn bone(&self, name: &str) -> Option<([f32; 12], [f32; 12])> {
        let c = self.items(1).into_iter().find(|x| x.0 == name)?.1;
        let b = &self.chunks[c];
        let m = |at: usize| -> [f32; 12] { std::array::from_fn(|i| f32::from_bits(be32(b, at + i * 4).unwrap_or(0))) };
        Some((m(0x70), m(0xA0)))
    }

    pub fn extend_bounds(&mut self, lo: [f32; 3], hi: [f32; 3]) {
        if let Some(info) = self.kinds.iter().position(|k| *k == Kind::Info) {
            let b = &mut self.chunks[info];
            for k in 0..3 {
                let (a, z) = (0x28 + k * 4, 0x34 + k * 4);
                let cur_lo = f32::from_bits(be32(b, a).unwrap_or(0));
                let cur_hi = f32::from_bits(be32(b, z).unwrap_or(0));
                b[a..a + 4].copy_from_slice(&cur_lo.min(lo[k]).to_be_bytes());
                b[z..z + 4].copy_from_slice(&cur_hi.max(hi[k]).to_be_bytes());
            }
        }
    }

    pub fn write(&self) -> (Vec<u8>, Vec<NameRef>) {
        let mut out = Vec::new();
        let mut start = Vec::with_capacity(self.chunks.len());
        for (c, &(m, r)) in self.chunks.iter().zip(&self.align) {
            while out.len() % m != r {
                out.push(0);
            }
            start.push(out.len());
            out.extend_from_slice(c);
        }
        let at = |l: Loc| start[l.chunk] + l.off;
        for &[f, base, t] in &self.links {
            let v = at(t) as i64 - at(base) as i64;
            out[at(f)..at(f) + 4].copy_from_slice(&(v as i32).to_be_bytes());
        }
        let len = out.len() as u32;
        out[4..8].copy_from_slice(&len.to_be_bytes());
        let names = self.names.iter().map(|(f, base, r)| r.moved(at(*f), at(*base))).collect();
        (out, names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brres::{dictionary, entries, textures, Archive};

    fn tree_of(b: &[u8], g: usize) -> Vec<[u16; 3]> {
        let n = be32(b, g + 4).unwrap() as usize;
        let h = |a: usize| u16::from_be_bytes([b[a], b[a + 1]]);
        (0..=n).map(|i| g + 8 + i * 16).map(|e| [h(e), h(e + 4), h(e + 6)]).collect()
    }

    fn check_tree(b: &[u8], g: usize) {
        let names: Vec<String> = entries(b, g).unwrap().into_iter().map(|e| e.1).collect();
        let refs: Vec<&[u8]> = names.iter().map(|n| n.as_bytes()).collect();
        assert_eq!(dictionary(&refs), tree_of(b, g), "{names:?}");
    }

    fn find(tree: &[[u16; 3]], names: &[&[u8]], key: &[u8]) -> Option<usize> {
        let mut prev = 0usize;
        let mut cur = tree[0][1] as usize;
        while tree[cur][0] < tree[prev][0] {
            prev = cur;
            let id = tree[cur][0];
            let c = (id >> 3) as usize;
            let bit = c < key.len() && (key[c] >> (id & 7)) & 1 == 1;
            cur = tree[cur][if bit { 2 } else { 1 }] as usize;
        }
        (cur > 0 && names[cur - 1] == key).then_some(cur - 1)
    }

    #[test]
    fn dictionary_finds_every_name() {
        let names: Vec<&[u8]> = vec![b"model", b"model_lod", b"mario_all", b"mario_eye.0", b"mario_eye.1", b"a", b"crown", b"model_cpu", b"m"];
        let tree = dictionary(&names);
        for (i, n) in names.iter().enumerate() {
            assert_eq!(find(&tree, &names, n), Some(i));
        }
        assert_eq!(find(&tree, &names, b"missing"), None);
    }

    fn tiny() -> Vec<u8> {
        let mut b = vec![0u8; 0x200];
        let w = |b: &mut Vec<u8>, at: usize, v: i64| b[at..at + 4].copy_from_slice(&(v as i32).to_be_bytes());
        b[..4].copy_from_slice(b"MDL0");
        w(&mut b, 8, 11);
        w(&mut b, 0x14, 0xA0);
        w(&mut b, 0x4C, 0x40);
        w(&mut b, 0x50, -0x4C);
        w(&mut b, 0x4C + 0x24, 0x40);
        w(&mut b, 0x8C, 1);
        w(&mut b, 0xA0, 0x28);
        w(&mut b, 0xA4, 1);
        w(&mut b, 0xA0 + 8, 0xFFFF_0000u32 as i64);
        w(&mut b, 0xA0 + 24 + 12, 0x100 - 0xA0);
        w(&mut b, 0x100, 0xD0);
        w(&mut b, 0x104, -0x100);
        b.truncate(0x1E0);
        let len = b.len() as i64;
        w(&mut b, 4, len);
        b
    }

    #[test]
    fn unchanged_model_writes_identical_bytes() {
        let b = tiny();
        let l = Layout::parse(&b, &[]).unwrap();
        assert_eq!(l.kinds, vec![Kind::Header, Kind::Info, Kind::Dictionary(1), Kind::Item(1)]);
        assert!(l.write().0 == b);
    }

    #[test]
    fn grown_chunks_keep_every_link() {
        let b = tiny();
        let mut l = Layout::parse(&b, &[NameRef::new(0x108, 0x100, "bone")]).unwrap();
        for c in l.chunks.iter_mut().skip(1) {
            c.extend_from_slice(&[0u8; 32]);
        }
        let (out, names) = l.write();
        assert_eq!(be32(&out, 4).unwrap() as usize, out.len());
        let bones = offset(0, be32(&out, 0x14).unwrap(), out.len()).unwrap();
        let bone = dict_items(&out, bones).unwrap()[0].1;
        assert_eq!(be32(&out, bone + 4).unwrap() as i32, -(bone as i32));
        assert_eq!(be32(&out, 0x4C + 4).unwrap() as i32, -0x4C);
        assert_eq!(Layout::parse(&out, &names).unwrap().kinds, l.kinds);
        assert_eq!((names[0].at, names[0].base), (bone + 8, bone));
    }

    #[test]
    fn real_every_driver_model_round_trips() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(std::path::Path::new(&p))) else {
            return;
        };
        let mut files: Vec<std::path::PathBuf> =
            std::fs::read_dir(dir.join("Race/Kart")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "szs")).collect();
        files.push(dir.join("Scene/Model/Driver.szs"));
        files.sort();
        let (mut models, mut trees) = (0, 0);
        for f in &files {
            let raw = crate::yaz0::decode(&std::fs::read(f).unwrap()).unwrap();
            let arc = crate::u8arc::parse(&raw).unwrap();
            for e in arc.files.iter().filter(|e| e.path.ends_with(".brres")) {
                let b = &raw[e.offset..e.offset + e.size];
                let label = format!("{}:{}", f.file_name().unwrap().to_string_lossy(), e.path);
                let root = u16::from_be_bytes([b[12], b[13]]) as usize + 8;
                check_tree(b, root);
                for (_, _, sub) in entries(b, root).unwrap() {
                    check_tree(b, sub);
                    trees += 2;
                    for (_, _, at) in entries(b, sub).unwrap() {
                        if !b[at..].starts_with(b"MDL0") {
                            continue;
                        }
                        for i in 0..14 {
                            let v = be32(b, at + 0x10 + i * 4).unwrap();
                            if v != 0 {
                                check_tree(b, at + v as usize);
                                trees += 1;
                            }
                        }
                    }
                }
                let a = Archive::parse(b).unwrap();
                let mut grown = a.clone();
                for m in grown.members.iter_mut().filter(|m| m.bytes.starts_with(b"MDL0")) {
                    let l = Layout::parse(&m.bytes, &m.names).unwrap_or_else(|err| panic!("{label} {} {err}", m.name));
                    let (same, names) = l.write();
                    assert!(same == m.bytes && names == m.names, "{label} {}", m.name);
                    let mut big = l.clone();
                    for c in big.chunks.iter_mut().skip(1) {
                        c.extend_from_slice(&[0u8; 32]);
                    }
                    let (bytes, names) = big.write();
                    let again = Layout::parse(&bytes, &names).unwrap_or_else(|err| panic!("{label} {} grown {err}", m.name));
                    assert_eq!((&again.kinds, &again.links), (&l.kinds, &l.links), "{label} {}", m.name);
                    m.bytes = bytes;
                    m.names = names;
                    models += 1;
                }
                let out = grown.write();
                assert_eq!(format!("{:?}", crate::mdl0::parse_all(b)), format!("{:?}", crate::mdl0::parse_all(&out)), "{label}");
                assert_eq!(format!("{:?}", crate::texmap::pat0_tracks(b)), format!("{:?}", crate::texmap::pat0_tracks(&out)), "{label}");
            }
        }
        println!("{models} models, {trees} dictionaries");
        assert!(models > 1000);
    }

    fn xf(m: &[f32; 12], p: [f32; 3], w: f32) -> [f32; 3] {
        std::array::from_fn(|r| m[r * 4] * p[0] + m[r * 4 + 1] * p[1] + m[r * 4 + 2] * p[2] + m[r * 4 + 3] * w)
    }

    fn box_mesh(inv: &[f32; 12], center: [f32; 3], size: f32) -> Mesh {
        let mut mesh = Mesh::default();
        let h = size / 2.0;
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let mut n = [0.0f32; 3];
                n[axis] = sign;
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                let base = mesh.positions.len() as u16;
                for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    let mut p = center;
                    p[axis] += sign * h;
                    p[u] += a * h;
                    p[v] += b * h;
                    mesh.positions.push(xf(inv, p, 1.0));
                    let nb = xf(inv, n, 0.0);
                    let len = (nb[0] * nb[0] + nb[1] * nb[1] + nb[2] * nb[2]).sqrt();
                    mesh.normals.push(nb.map(|x| x / len));
                    mesh.colors.push([255, 255, 255, 255]);
                    mesh.uvs.push([0.1, 0.1]);
                }
                for t in [[0, 1, 2], [0, 2, 3], [0, 2, 1], [0, 3, 2]] {
                    mesh.triangles.push(t.map(|i| base + i));
                }
            }
        }
        mesh
    }

    #[test]
    fn real_mesh_follows_bone() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(std::path::Path::new(&p))) else {
            return;
        };
        let mut names: Vec<String> = std::fs::read_dir(dir.join("Race/Kart"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with("-mr.szs"))
            .collect();
        names.sort();
        assert!(!names.is_empty());
        for name in &names {
            let rel = format!("Race/Kart/{name}");
            let raw = crate::skin::read_archive(&dir, &rel).unwrap();
            let u8a = crate::u8arc::parse(&raw).unwrap();
            let entry = u8a.find("driver_model.brres").unwrap().clone();
            let b = &raw[entry.offset..entry.offset + entry.size];
            let mut a = Archive::parse(b).unwrap();
            let m = a.members.iter_mut().find(|m| m.name == "model" && m.bytes.starts_with(b"MDL0")).unwrap();
            let mut l = Layout::parse(&m.bytes, &m.names).unwrap();
            let (model, inv) = l.bone("face_1").unwrap();
            let before = crate::mdl0::parse_all(b).unwrap();
            let body = before.iter().find(|x| x.name == "model").unwrap();
            let face = [model[3], model[7], model[11]];
            let head: Vec<[f32; 3]> = body.submeshes.iter().flat_map(|s| s.positions.chunks(3).map(|p| [p[0], p[1], p[2]])).filter(|p| p[1] > face[1]).collect();
            let top = head.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            let (minx, maxx) = head.iter().fold((f32::MAX, f32::MIN), |(a, z), p| (a.min(p[0]), z.max(p[0])));
            let (minz, maxz) = head.iter().fold((f32::MAX, f32::MIN), |(a, z), p| (a.min(p[2]), z.max(p[2])));
            let size = (maxx - minx).max(maxz - minz) * 0.45;
            let center = [(minx + maxx) / 2.0, top + size * 0.4, (minz + maxz) / 2.0];
            let mesh = box_mesh(&inv, center, size);
            l.add_rigid_mesh("polygon1", "box", &mesh, None).unwrap_or_else(|e| panic!("{name} {e}"));
            l.extend_bounds(center.map(|c| c - size), center.map(|c| c + size));
            let (bytes, refs) = l.write();
            m.bytes = bytes;
            m.names = refs;
            let out = a.write();
            let after = crate::mdl0::parse_all(&out).unwrap_or_else(|e| panic!("{name} {e}"));
            assert_eq!(after.len(), before.len());
            let model_after = after.iter().find(|x| x.name == "model").unwrap();
            let rest: Vec<String> = model_after.submeshes.iter().filter(|s| s.shape != "box").map(|s| format!("{s:?}")).collect();
            let orig: Vec<String> = body.submeshes.iter().map(|s| format!("{s:?}")).collect();
            assert_eq!(rest, orig, "{name}");
            let added = model_after.submeshes.iter().find(|s| s.shape == "box").unwrap();
            assert_eq!(added.triangles(), 24);
            let tol = size * 0.05 + 0.01;
            for p in added.positions.chunks(3) {
                let ok = (0..3).all(|k| (p[k] - center[k]).abs() <= size / 2.0 + tol) && (0..3).any(|k| ((p[k] - center[k]).abs() - size / 2.0).abs() <= tol);
                assert!(ok, "{name} {p:?} {center:?} {size}");
            }
            for (x, y) in before.iter().zip(&after).filter(|(x, _)| x.name != "model") {
                assert_eq!(format!("{x:?}"), format!("{y:?}"));
            }
            let mut payloads = Vec::new();
            for (i, node) in u8a.nodes.iter().enumerate() {
                payloads.push(if node.is_dir {
                    Vec::new()
                } else if i == entry.node {
                    out.clone()
                } else {
                    raw[node.a as usize..(node.a + node.b) as usize].to_vec()
                });
            }
            let packed = crate::u8arc::write(&u8a.nodes, &payloads);
            let check = crate::u8arc::parse(&packed).unwrap();
            let e2 = check.find("driver_model.brres").unwrap();
            assert!(packed[e2.offset..e2.offset + e2.size] == out[..]);
            assert!(crate::yaz0::decode(&crate::yaz0::encode(&packed).unwrap()).unwrap() == packed);
        }
    }

    fn anchor(positions: &[f32]) -> ([f32; 3], f32) {
        let pts: Vec<[f32; 3]> = positions.chunks(3).map(|p| [p[0], p[1], p[2]]).collect();
        let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(a, z), p| (a.min(p[1]), z.max(p[1])));
        let band: Vec<&[f32; 3]> = pts.iter().filter(|p| p[1] > hi - (hi - lo) * 0.12).collect();
        let (minx, maxx) = band.iter().fold((f32::MAX, f32::MIN), |(a, z), p| (a.min(p[0]), z.max(p[0])));
        let (minz, maxz) = band.iter().fold((f32::MAX, f32::MIN), |(a, z), p| (a.min(p[2]), z.max(p[2])));
        let size = (maxx - minx).max(maxz - minz) * 0.6;
        ([(minx + maxx) / 2.0, hi, (minz + maxz) / 2.0], size)
    }

    fn ring_mesh(inv: &[f32; 12], center: [f32; 3], size: f32) -> Mesh {
        let mut mesh = Mesh::default();
        let (r, band, spike) = (size / 2.0, size * 0.22, size * 0.32);
        let n = 16;
        let push = |mesh: &mut Mesh, p: [f32; 3], nrm: [f32; 3], uv: [f32; 2]| {
            mesh.positions.push(xf(inv, p, 1.0));
            let nb = xf(inv, nrm, 0.0);
            let len = (nb[0] * nb[0] + nb[1] * nb[1] + nb[2] * nb[2]).sqrt().max(1e-6);
            mesh.normals.push(nb.map(|x| x / len));
            mesh.colors.push([255, 255, 255, 255]);
            mesh.uvs.push(uv);
            (mesh.positions.len() - 1) as u16
        };
        let y0 = center[1] - band * 0.4;
        for i in 0..n {
            let (a0, a1) = (i as f32 / n as f32 * std::f32::consts::TAU, (i + 1) as f32 / n as f32 * std::f32::consts::TAU);
            let at = |a: f32, y: f32| [center[0] + r * a.cos(), y, center[2] + r * a.sin()];
            let nr = |a: f32| [a.cos(), 0.0, a.sin()];
            let (u0, u1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
            let q = [
                push(&mut mesh, at(a0, y0), nr(a0), [u0, 0.95]),
                push(&mut mesh, at(a1, y0), nr(a1), [u1, 0.95]),
                push(&mut mesh, at(a1, y0 + band), nr(a1), [u1, 0.55]),
                push(&mut mesh, at(a0, y0 + band), nr(a0), [u0, 0.55]),
            ];
            for t in [[0, 1, 2], [0, 2, 3], [0, 2, 1], [0, 3, 2]] {
                mesh.triangles.push(t.map(|k| q[k]));
            }
            if i % 2 == 0 {
                let am = (a0 + a1) / 2.0;
                let tip = [center[0] + r * am.cos(), y0 + band + spike, center[2] + r * am.sin()];
                let s = [
                    push(&mut mesh, at(a0, y0 + band), nr(a0), [u0, 0.5]),
                    push(&mut mesh, at(a1, y0 + band), nr(a1), [u1, 0.5]),
                    push(&mut mesh, tip, nr(am), [(u0 + u1) / 2.0, 0.05]),
                ];
                mesh.triangles.push([s[0], s[1], s[2]]);
                mesh.triangles.push([s[0], s[2], s[1]]);
            }
        }
        mesh
    }

    fn sample_texture() -> (u32, u32, Vec<u8>) {
        let (w, h) = (32u32, 32u32);
        let mut img = crate::texture::Rgba8 { width: w, height: h, pixels: Vec::new() };
        for y in 0..h {
            for x in 0..w {
                let v = ((x * 8 + y * 4) % 256) as u8;
                img.pixels.extend_from_slice(&[v, 255 - v, 128, 255]);
            }
        }
        (w, h, crate::texture::encode(&img, 14, 1).unwrap())
    }

    fn decorate(label: &str, b: &[u8]) -> (Vec<u8>, usize) {
        let (w, h, data) = sample_texture();
        let mut a = Archive::parse(b).unwrap();
        a.add_texture("accessory", w, h, 14, 1, &data).unwrap();
        let before = crate::mdl0::parse_all(b).unwrap();
        let mut count = 0;
        for m in a.members.iter_mut().filter(|m| m.bytes.starts_with(b"MDL0")) {
            let mut l = Layout::parse(&m.bytes, &m.names).unwrap();
            let rigid = l.rigid_shapes();
            let (shape, _, bone, mat) = rigid.iter().min_by_key(|x| x.1).unwrap_or_else(|| panic!("{label} {} no rigid shape", m.name)).clone();
            let model = before.iter().find(|x| x.name == m.name).unwrap();
            let all: Vec<f32> = model.submeshes.iter().flat_map(|s| s.positions.iter().copied()).collect();
            let (center, size) = anchor(&all);
            let (_, inv) = l.bone(&bone).unwrap();
            let id = l.add_material(&mat, "accessory", "accessory").unwrap_or_else(|e| panic!("{label} {} {e}", m.name));
            l.add_rigid_mesh(&shape, "accessory", &ring_mesh(&inv, center, size), Some(id)).unwrap_or_else(|e| panic!("{label} {} {e}", m.name));
            l.extend_bounds(center.map(|c| c - size), center.map(|c| c + size));
            let (bytes, refs) = l.write();
            m.bytes = bytes;
            m.names = refs;
            count += 1;
        }
        let out = a.write();
        let after = crate::mdl0::parse_all(&out).unwrap_or_else(|e| panic!("{label} {e}"));
        for (x, y) in before.iter().zip(&after) {
            let rest: Vec<String> = y.submeshes.iter().filter(|s| s.shape != "accessory").map(|s| format!("{s:?}")).collect();
            let orig: Vec<String> = x.submeshes.iter().map(|s| format!("{s:?}")).collect();
            assert_eq!(rest, orig, "{label} {}", x.name);
            let added = y.submeshes.iter().find(|s| s.shape == "accessory").unwrap();
            assert_eq!(added.material, "accessory");
            assert_eq!(added.texture(y).map(|t| t.texture.as_str()), Some("accessory"), "{label} {}", x.name);
        }
        assert!(textures(&out).unwrap().iter().any(|t| t.name == "accessory" && t.width == w));
        assert_eq!(format!("{:?}", crate::texmap::pat0_tracks(b)), format!("{:?}", crate::texmap::pat0_tracks(&out)), "{label}");
        let again = Archive::parse(&out).unwrap();
        for m in again.members.iter().filter(|m| m.bytes.starts_with(b"MDL0")) {
            Layout::parse(&m.bytes, &m.names).unwrap_or_else(|e| panic!("{label} {} reparse {e}", m.name));
        }
        (out, count)
    }

    fn replace_member(raw: &[u8], member: &str, data: Vec<u8>) -> Vec<u8> {
        let u = crate::u8arc::parse(raw).unwrap();
        let target = u.find(member).unwrap().node;
        let payloads: Vec<Vec<u8>> = u
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| if n.is_dir { Vec::new() } else if i == target { data.clone() } else { raw[n.a as usize..(n.a + n.b) as usize].to_vec() })
            .collect();
        crate::u8arc::write(&u.nodes, &payloads)
    }

    #[test]
    fn real_textured_mesh_in_every_model() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(std::path::Path::new(&p))) else {
            return;
        };
        let ch = crate::game::scan(&dir).unwrap().into_iter().find(|c| c.code == "mr").unwrap();
        let mut models = 0;
        for rel in &ch.files {
            let raw = crate::skin::read_archive(&dir, rel).unwrap();
            let u = crate::u8arc::parse(&raw).unwrap();
            let e = u.find("driver_model.brres").unwrap();
            let (out, n) = decorate(rel, &raw[e.offset..e.offset + e.size]);
            models += n;
            let packed = replace_member(&raw, "driver_model.brres", out);
            assert!(crate::yaz0::decode(&crate::yaz0::encode(&packed).unwrap()).unwrap() == packed);
        }
        let rel = crate::game::DRIVER_SZS;
        let raw = crate::skin::read_archive(&dir, rel).unwrap();
        let u = crate::u8arc::parse(&raw).unwrap();
        let e = u.find("mr.brres").unwrap();
        let (out, n) = decorate(rel, &raw[e.offset..e.offset + e.size]);
        models += n;
        let packed = replace_member(&raw, "mr.brres", out);
        assert!(crate::u8arc::parse(&packed).unwrap().find("mr.brres").is_some());
        assert!(models > 90, "{models}");
    }
}
