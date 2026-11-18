use crate::brres::{be32, offset, NameRef};
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

#[derive(Clone, Debug)]
pub struct Layout {
    pub kinds: Vec<Kind>,
    pub chunks: Vec<Vec<u8>>,
    align: Vec<usize>,
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
            align: starts.iter().map(|&p| p % 32).collect(),
            chunks,
            links,
            names: refs,
        })
    }

    pub fn write(&self) -> (Vec<u8>, Vec<NameRef>) {
        let mut out = Vec::new();
        let mut start = Vec::with_capacity(self.chunks.len());
        for (c, &a) in self.chunks.iter().zip(&self.align) {
            while out.len() % 32 != a {
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
    use crate::brres::{dictionary, entries, Archive};

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
}
