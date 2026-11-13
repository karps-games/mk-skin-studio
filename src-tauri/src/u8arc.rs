const MAGIC: [u8; 4] = [0x55, 0xAA, 0x38, 0x2D];
const MAX_NODES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub is_dir: bool,
    pub name: String,
    pub a: u32,
    pub b: u32,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub node: usize,
    pub path: String,
    pub offset: usize,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct Archive {
    pub nodes: Vec<Node>,
    pub files: Vec<FileEntry>,
}

fn be32(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4).map(|s| u32::from_be_bytes(s.try_into().unwrap())).ok_or_else(|| "E:file.format".to_string())
}

impl Archive {
    pub fn find(&self, name: &str) -> Option<&FileEntry> {
        self.files.iter().find(|f| f.path.trim_start_matches("./") == name)
    }
}

pub fn parse(raw: &[u8]) -> Result<Archive, String> {
    let bad = || "E:file.format".to_string();
    if raw.len() < 0x20 || raw[..4] != MAGIC {
        return Err(bad());
    }
    let first = be32(raw, 4)? as usize;
    let header_size = be32(raw, 8)? as usize;
    let data_off = be32(raw, 12)? as usize;
    if first != 0x20 || first + 12 > raw.len() {
        return Err(bad());
    }
    let count = be32(raw, first + 8)? as usize;
    if count == 0 || count > MAX_NODES || raw[first] != 1 {
        return Err(bad());
    }
    let strings = first + count * 12;
    let str_end = first.checked_add(header_size).ok_or_else(bad)?;
    if strings > str_end || str_end > raw.len() || data_off < str_end || data_off > raw.len() {
        return Err(bad());
    }
    let mut nodes = Vec::with_capacity(count);
    for i in 0..count {
        let at = first + i * 12;
        let kind = raw[at];
        let name_off = (be32(raw, at)? & 0x00FF_FFFF) as usize;
        let a = be32(raw, at + 4)?;
        let b = be32(raw, at + 8)?;
        let s = strings.checked_add(name_off).ok_or_else(bad)?;
        if s >= str_end {
            return Err(bad());
        }
        let len = raw[s..str_end].iter().position(|&c| c == 0).ok_or_else(bad)?;
        let name = std::str::from_utf8(&raw[s..s + len]).map_err(|_| bad())?.to_string();
        match kind {
            1 => {
                let end = b as usize;
                if end <= i || end > count || (i > 0 && a as usize >= i) {
                    return Err(bad());
                }
                if i > 0 && (name.is_empty() || name == ".." || name.contains('/') || name.contains('\\')) {
                    return Err(bad());
                }
            }
            0 => {
                let (o, n) = (a as usize, b as usize);
                if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
                    return Err(bad());
                }
                if o < data_off || o.checked_add(n).is_none_or(|e| e > raw.len()) {
                    return Err(bad());
                }
            }
            _ => return Err(bad()),
        }
        nodes.push(Node { is_dir: kind == 1, name, a, b });
    }
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut files = Vec::new();
    for (i, n) in nodes.iter().enumerate().skip(1) {
        while stack.last().is_some_and(|(_, end)| *end <= i) {
            stack.pop();
        }
        if let Some((_, end)) = stack.last() {
            if n.is_dir && n.b as usize > *end {
                return Err(bad());
            }
        } else if n.b as usize > count && n.is_dir {
            return Err(bad());
        }
        let prefix: Vec<&str> = stack.iter().map(|(s, _)| s.as_str()).collect();
        let path = if prefix.is_empty() { n.name.clone() } else { format!("{}/{}", prefix.join("/"), n.name) };
        if n.is_dir {
            stack.push((n.name.clone(), n.b as usize));
        } else {
            files.push(FileEntry { node: i, path, offset: n.a as usize, size: n.b as usize });
        }
    }
    let mut seen = std::collections::HashSet::new();
    for f in &files {
        if !seen.insert(f.path.to_ascii_lowercase()) {
            return Err(bad());
        }
    }
    Ok(Archive { nodes, files })
}

pub fn exclusive(arc: &Archive, idx: usize) -> bool {
    let f = &arc.files[idx];
    let (s, e) = (f.offset, f.offset + f.size);
    arc.files.iter().enumerate().all(|(j, g)| j == idx || g.size == 0 || g.offset + g.size <= s || g.offset >= e)
}

fn align(v: usize, a: usize) -> usize {
    v.div_ceil(a) * a
}

pub fn write(nodes: &[Node], payloads: &[Vec<u8>]) -> Vec<u8> {
    let count = nodes.len();
    let mut strings = Vec::new();
    let mut name_offs = Vec::with_capacity(count);
    for n in nodes {
        name_offs.push(strings.len() as u32);
        strings.extend_from_slice(n.name.as_bytes());
        strings.push(0);
    }
    let header_size = count * 12 + strings.len();
    let data_off = align(0x20 + header_size, 0x20);
    let mut out = vec![0u8; data_off];
    out[..4].copy_from_slice(&MAGIC);
    out[4..8].copy_from_slice(&0x20u32.to_be_bytes());
    out[8..12].copy_from_slice(&(header_size as u32).to_be_bytes());
    out[12..16].copy_from_slice(&(data_off as u32).to_be_bytes());
    out[16..32].fill(0xCC);
    let mut offsets = vec![0usize; count];
    for (i, n) in nodes.iter().enumerate() {
        if !n.is_dir {
            let at = align(out.len(), 0x20);
            out.resize(at, 0);
            offsets[i] = at;
            out.extend_from_slice(&payloads[i]);
        }
    }
    out.resize(align(out.len(), 0x20), 0);
    for (i, n) in nodes.iter().enumerate() {
        let at = 0x20 + i * 12;
        let word = ((n.is_dir as u32) << 24) | name_offs[i];
        out[at..at + 4].copy_from_slice(&word.to_be_bytes());
        let (a, b) = if n.is_dir { (n.a, n.b) } else { (offsets[i] as u32, payloads[i].len() as u32) };
        out[at + 4..at + 8].copy_from_slice(&a.to_be_bytes());
        out[at + 8..at + 12].copy_from_slice(&b.to_be_bytes());
    }
    let s = 0x20 + count * 12;
    out[s..s + strings.len()].copy_from_slice(&strings);
    out
}

pub fn rewrite(raw: &[u8], arc: &Archive) -> Vec<u8> {
    let payloads: Vec<Vec<u8>> = arc
        .nodes
        .iter()
        .map(|n| if n.is_dir { Vec::new() } else { raw[n.a as usize..(n.a + n.b) as usize].to_vec() })
        .collect();
    write(&arc.nodes, &payloads)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn sample() -> Vec<u8> {
        let nodes = vec![
            Node { is_dir: true, name: String::new(), a: 0, b: 4 },
            Node { is_dir: true, name: ".".into(), a: 0, b: 4 },
            Node { is_dir: false, name: "a.bin".into(), a: 0, b: 0 },
            Node { is_dir: false, name: "driver_model.brres".into(), a: 0, b: 0 },
        ];
        write(&nodes, &[vec![], vec![], b"hello".to_vec(), vec![7u8; 100]])
    }

    #[test]
    fn synthetic_round_trip_and_paths() {
        let raw = sample();
        let arc = parse(&raw).unwrap();
        assert_eq!(arc.files.len(), 2);
        assert_eq!(arc.files[1].path, "./driver_model.brres");
        assert_eq!(arc.find("driver_model.brres").unwrap().size, 100);
        assert_eq!(rewrite(&raw, &arc), raw);
        assert!(exclusive(&arc, 0) && exclusive(&arc, 1));
    }

    #[test]
    fn rejects_malformed() {
        let raw = sample();
        for end in 0..raw.len() - 1 {
            let _ = parse(&raw[..end]);
        }
        let mut bad = raw.clone();
        bad[0x20 + 3 * 12 + 4..0x20 + 3 * 12 + 8].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse(&bad).is_err());
        let mut bad = raw.clone();
        bad[0x20 + 8..0x20 + 12].copy_from_slice(&999u32.to_be_bytes());
        assert!(parse(&bad).is_err());
        let mut bad = raw;
        bad[0] = 0;
        assert!(parse(&bad).is_err());
    }
}
