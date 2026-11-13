use crate::brres::{be16, be32, cstr, group, offset};

macro_rules! bad {
    () => {
        format!("E:model.format|line={}", line!())
    };
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sampler {
    pub texture: String,
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub uv_set: Option<u32>,
    pub srt: Option<[f32; 5]>,
}

#[derive(Clone, Debug)]
pub struct Material {
    pub name: String,
    pub samplers: Vec<Sampler>,
}

#[derive(Clone, Debug, Default)]
pub struct Submesh {
    pub material: String,
    pub shape: String,
    pub xlu: bool,
    pub sampler: Option<usize>,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub name: String,
    pub materials: Vec<Material>,
    pub submeshes: Vec<Submesh>,
    pub skipped_shapes: usize,
}

impl Submesh {
    pub fn texture<'a>(&self, m: &'a Model) -> Option<&'a Sampler> {
        let mat = m.materials.iter().find(|x| x.name == self.material)?;
        mat.samplers.get(self.sampler?)
    }
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
}

fn f32be(b: &[u8], at: usize) -> Result<f32, String> {
    Ok(f32::from_bits(be32(b, at)?))
}
fn s16be(b: &[u8], at: usize) -> Result<i16, String> {
    Ok(be16(b, at)? as i16)
}
fn u8at(b: &[u8], at: usize) -> Result<u8, String> {
    b.get(at).copied().ok_or_else(|| bad!())
}

pub fn models_in(b: &[u8]) -> Result<Vec<(String, usize)>, String> {
    if b.len() < 16 || &b[..4] != b"bres" {
        return Err(bad!());
    }
    let root = be16(b, 12)? as usize;
    if b.get(root..root + 4) != Some(b"root") {
        return Err(bad!());
    }
    let mut out = Vec::new();
    for (folder, sub) in group(b, root + 8)? {
        if folder == "3DModels(NW4R)" {
            for (name, sec) in group(b, sub)? {
                if b.get(sec..sec + 4) == Some(b"MDL0") {
                    out.push((name, sec));
                }
            }
        }
    }
    Ok(out)
}

fn vertex_array(b: &[u8], at: usize, kind: u8) -> Result<(u32, Vec<f32>, usize), String> {
    let data = offset(at, be32(b, at + 8)?, b.len())?;
    let id = be32(b, at + 0x10)?;
    let comp = be32(b, at + 0x14)?;
    let ty = be32(b, at + 0x18)?;
    let (mut div, stride) = (u8at(b, at + 0x1C)? as u32, u8at(b, at + 0x1D)? as usize);
    let count = be16(b, at + 0x1E)? as usize;
    let comps = match (kind, comp) {
        (b'p', 0) => 2,
        (b'p', 1) => 3,
        (b'n', 0) => 3,
        (b'n', 1) | (b'n', 2) => 9,
        (b'u', 0) => 1,
        (b'u', 1) => 2,
        _ => return Err(bad!()),
    };
    let size = match ty {
        0 | 1 => 1,
        2 | 3 => 2,
        4 => 4,
        _ => return Err(bad!()),
    };
    if kind == b'n' {
        div = match ty {
            1 => 6,
            3 => 14,
            _ => div,
        };
    }
    let stride = if stride == 0 { comps * size } else { stride };
    if count > 0x10000 || data + stride * count > b.len() || stride < comps * size {
        return Err(bad!());
    }
    let scale = 1.0 / (1u32 << div.min(31)) as f32;
    let mut out = Vec::with_capacity(count * comps.max(2));
    for i in 0..count {
        let e = data + i * stride;
        let mut vals = [0f32; 9];
        for (c, v) in vals.iter_mut().enumerate().take(comps) {
            let p = e + c * size;
            *v = match ty {
                0 => b[p] as f32 * scale,
                1 => b[p] as i8 as f32 * scale,
                2 => be16(b, p)? as f32 * scale,
                3 => s16be(b, p)? as f32 * scale,
                _ => f32be(b, p)?,
            };
        }
        match (kind, comps) {
            (b'p', 2) => out.extend_from_slice(&[vals[0], vals[1], 0.0]),
            (b'u', 1) => out.extend_from_slice(&[vals[0], 0.0]),
            (b'n', _) => out.extend_from_slice(&vals[..3]),
            _ => out.extend_from_slice(&vals[..comps]),
        }
    }
    let width = match kind {
        b'u' => 2,
        _ => 3,
    };
    Ok((id, out, width))
}

fn texgen_sources(b: &[u8], dl: usize, num: usize) -> Vec<Option<u32>> {
    let mut out = vec![None; num.min(8)];
    let end = (dl + 0xE0 + 0xA0).min(b.len());
    let mut p = dl + 0xE0;
    while p < end {
        match b[p] {
            0x00 => p += 1,
            0x61 => p += 5,
            0x08 => p += 6,
            0x10 => {
                let (Ok(n), Ok(addr)) = (be16(b, p + 1), be16(b, p + 3)) else { break };
                let n = n as usize + 1;
                for i in 0..n {
                    let reg = addr as usize + i;
                    if (0x1040..0x1048).contains(&reg) {
                        let slot = reg - 0x1040;
                        if let (Ok(v), true) = (be32(b, p + 5 + i * 4), slot < out.len()) {
                            let ty = (v >> 4) & 7;
                            let row = (v >> 7) & 0x1F;
                            out[slot] = if ty == 0 && (5..=12).contains(&row) { Some(row - 5) } else { None };
                        }
                    }
                }
                p += 5 + n * 4;
            }
            _ => break,
        }
    }
    out
}

fn material(b: &[u8], at: usize) -> Result<(u32, Material), String> {
    let name = cstr(b, offset(at, be32(b, at + 8)?, b.len())?)?;
    let id = be32(b, at + 0x0C)?;
    let num_texgens = u8at(b, at + 0x14)? as usize;
    let n_tex = be32(b, at + 0x2C)? as usize;
    if n_tex > 8 {
        return Err(bad!());
    }
    let dl = offset(at, be32(b, at + 0x3C)?, b.len())?;
    let sources = texgen_sources(b, dl, num_texgens.max(n_tex));
    let srt_flags = be32(b, at + 0x1A8)?;
    let mut samplers = Vec::new();
    if n_tex > 0 {
        let base = offset(at, be32(b, at + 0x30)?, b.len())?;
        for i in 0..n_tex {
            let s = base + i * 0x34;
            let texture = cstr(b, offset(s, be32(b, s)?, b.len())?)?;
            let flags = (srt_flags >> (i * 4)) & 0xF;
            let mut srt = None;
            if flags & 1 != 0 && flags & 0xE != 0xE {
                let e = at + 0x1B0 + i * 20;
                let v = [f32be(b, e)?, f32be(b, e + 4)?, f32be(b, e + 8)?, f32be(b, e + 12)?, f32be(b, e + 16)?];
                if v != [1.0, 1.0, 0.0, 0.0, 0.0] && v.iter().all(|x| x.is_finite()) {
                    srt = Some(v);
                }
            }
            samplers.push(Sampler {
                texture,
                wrap_s: be32(b, s + 0x18)?,
                wrap_t: be32(b, s + 0x1C)?,
                uv_set: if i < sources.len() { sources[i] } else { Some(0) },
                srt,
            });
        }
    }
    Ok((id, Material { name, samplers }))
}

pub fn apply_srt(srt: &[f32; 5], u: f32, v: f32) -> (f32, f32) {
    let [sx, sy, rot, tx, ty] = *srt;
    let (s, c) = rot.to_radians().sin_cos();
    let (mut x, mut y) = (u * sx, (v - 1.0) * sy + 1.0);
    let (cx, cy) = (x - 0.5, y - 0.5);
    x = cx * c + cy * s + 0.5;
    y = -cx * s + cy * c + 0.5;
    (x - tx, y + ty)
}

struct Bone {
    model: [f32; 12],
}

fn bone(b: &[u8], at: usize) -> Result<(u32, Bone), String> {
    let id = be32(b, at + 0x0C)?;
    let mut model = [0f32; 12];
    for (i, m) in model.iter_mut().enumerate() {
        *m = f32be(b, at + 0x70 + i * 4)?;
    }
    Ok((id, Bone { model }))
}

fn xform(m: &[f32; 12], p: [f32; 3], w: f32) -> [f32; 3] {
    [
        m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3] * w,
        m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7] * w,
        m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11] * w,
    ]
}

struct Shape {
    name: String,
    current_matrix: i32,
    vcd_lo: u32,
    vcd_hi: u32,
    dl: usize,
    dl_len: usize,
    pos: i16,
    nrm: i16,
    uv: [i16; 8],
}

fn shape(b: &[u8], at: usize, version: u32) -> Result<(u32, Shape), String> {
    let dl_len = be32(b, at + 0x24)? as usize;
    let dl = offset(at + 0x24, be32(b, at + 0x2C)?, b.len())?;
    if dl + dl_len > b.len() {
        return Err(bad!());
    }
    let mut uv = [-1i16; 8];
    for (i, u) in uv.iter_mut().enumerate() {
        *u = s16be(b, at + 0x50 + i * 2)?;
    }
    let _ = version;
    Ok((
        be32(b, at + 0x3C)?,
        Shape {
            name: cstr(b, offset(at, be32(b, at + 0x38)?, b.len())?)?,
            current_matrix: be32(b, at + 0x08)? as i32,
            vcd_lo: be32(b, at + 0x0C)?,
            vcd_hi: be32(b, at + 0x10)?,
            dl,
            dl_len,
            pos: s16be(b, at + 0x48)?,
            nrm: s16be(b, at + 0x4A)?,
            uv,
        },
    ))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Vtx {
    mtx: i32,
    pos: u32,
    nrm: u32,
    uv: [u32; 8],
}

fn primitives(b: &[u8], s: &Shape) -> Result<Vec<[Vtx; 3]>, String> {
    let pnmtx = s.vcd_lo & 1 != 0;
    let texmtx: Vec<bool> = (0..8).map(|i| s.vcd_lo & (1 << (1 + i)) != 0).collect();
    let pos_t = (s.vcd_lo >> 9) & 3;
    let nrm_t = (s.vcd_lo >> 11) & 3;
    let clr_t = [(s.vcd_lo >> 13) & 3, (s.vcd_lo >> 15) & 3];
    let uv_t: Vec<u32> = (0..8).map(|i| (s.vcd_hi >> (i * 2)) & 3).collect();
    for t in [pos_t, nrm_t, clr_t[0], clr_t[1]].iter().chain(uv_t.iter()) {
        if *t == 1 {
            return Err("E:model.directAttr".into());
        }
    }
    if pos_t == 0 {
        return Err(bad!());
    }
    let idx = |t: u32, p: &mut usize| -> Result<u32, String> {
        Ok(match t {
            2 => {
                let v = u8at(b, *p)? as u32;
                *p += 1;
                v
            }
            3 => {
                let v = be16(b, *p)? as u32;
                *p += 2;
                v
            }
            _ => u32::MAX,
        })
    };
    let mut slots = [-1i32; 10];
    let mut tris = Vec::new();
    let end = s.dl + s.dl_len;
    let mut p = s.dl;
    while p < end {
        let op = b[p];
        p += 1;
        match op {
            0x00 | 0x48 => {}
            0x08 => p += 5,
            0x10 => {
                let n = be16(b, p)? as usize + 1;
                p += 4 + n * 4;
            }
            0x20 | 0x28 | 0x30 | 0x38 => {
                let index = be16(b, p)?;
                let addr = be16(b, p + 2)? & 0xFFF;
                if op == 0x20 && addr % 12 == 0 && (addr / 12) < 10 {
                    slots[(addr / 12) as usize] = index as i32;
                }
                p += 4;
            }
            0x40 => p += 8,
            0x61 => p += 4,
            _ if op & 0x80 != 0 => {
                let kind = op & 0xF8;
                let n = be16(b, p)? as usize;
                p += 2;
                let mut verts = Vec::with_capacity(n);
                for _ in 0..n {
                    let mut mtx = s.current_matrix;
                    if pnmtx {
                        let v = u8at(b, p)? as usize;
                        p += 1;
                        if v % 3 != 0 || v / 3 >= 10 {
                            return Err(bad!());
                        }
                        mtx = slots[v / 3];
                    }
                    for t in &texmtx {
                        if *t {
                            p += 1;
                        }
                    }
                    let pos = idx(pos_t, &mut p)?;
                    let nrm = idx(nrm_t, &mut p)?;
                    idx(clr_t[0], &mut p)?;
                    idx(clr_t[1], &mut p)?;
                    let mut uv = [u32::MAX; 8];
                    for (i, t) in uv_t.iter().enumerate() {
                        uv[i] = idx(*t, &mut p)?;
                    }
                    verts.push(Vtx { mtx, pos, nrm, uv });
                }
                if p > end {
                    return Err(bad!());
                }
                match kind {
                    0x80 | 0x88 => {
                        for q in verts.chunks_exact(4) {
                            tris.push([q[0], q[1], q[2]]);
                            tris.push([q[0], q[2], q[3]]);
                        }
                    }
                    0x90 => tris.extend(verts.chunks_exact(3).map(|t| [t[0], t[1], t[2]])),
                    0x98 => {
                        for i in 2..verts.len() {
                            if i % 2 == 0 {
                                tris.push([verts[i - 2], verts[i - 1], verts[i]]);
                            } else {
                                tris.push([verts[i - 1], verts[i - 2], verts[i]]);
                            }
                        }
                    }
                    0xA0 => {
                        for i in 2..verts.len() {
                            tris.push([verts[0], verts[i - 1], verts[i]]);
                        }
                    }
                    _ => {}
                }
            }
            _ => return Err(bad!()),
        }
    }
    Ok(tris)
}

fn draws(b: &[u8], at: usize, xlu: bool, out: &mut Vec<(u32, u32, bool)>) -> Result<(), String> {
    let mut p = at;
    for _ in 0..100_000 {
        let op = u8at(b, p)?;
        match op {
            0x00 => p += 1,
            0x01 => return Ok(()),
            0x02 => p += 5,
            0x03 => {
                let n = u8at(b, p + 3)? as usize;
                p += 4 + n * 6;
            }
            0x04 => {
                out.push((be16(b, p + 1)? as u32, be16(b, p + 3)? as u32, xlu));
                p += 8;
            }
            0x05 => p += 5,
            _ => return Err(bad!()),
        }
    }
    Err(bad!())
}

pub fn is_light_map(texture: &str) -> bool {
    texture.starts_with("lm_") || texture == "lm"
}

pub fn parse(b: &[u8], at: usize) -> Result<Model, String> {
    if b.get(at..at + 4) != Some(b"MDL0") {
        return Err(bad!());
    }
    let size = be32(b, at + 4)? as usize;
    if at + size > b.len() {
        return Err(bad!());
    }
    let version = be32(b, at + 8)?;
    let nsec = match version {
        8 | 9 => 11,
        10 | 11 => 14,
        _ => return Err("E:model.version".into()),
    };
    let sec = |i: usize| -> Result<Option<usize>, String> {
        let v = be32(b, at + 0x10 + i * 4)?;
        if v == 0 { Ok(None) } else { offset(at, v, b.len()).map(Some) }
    };
    let (i_tree, i_bones, i_pos, i_nrm, i_uv, i_mat, i_shape) = if nsec == 14 { (0, 1, 2, 3, 5, 8, 10) } else { (0, 1, 2, 3, 5, 6, 8) };
    let name = cstr(b, offset(at, be32(b, at + 0x10 + nsec * 4)?, b.len())?)?;
    let info = at + 0x10 + nsec * 4 + 4;
    let table_at = offset(info, be32(b, info + 0x24)?, b.len())?;
    let n = be32(b, table_at)? as usize;
    if n > 4096 {
        return Err(bad!());
    }
    let mut mtx_to_bone = Vec::with_capacity(n);
    for i in 0..n {
        mtx_to_bone.push(be32(b, table_at + 4 + i * 4)? as i32);
    }
    let list = |i: usize| -> Result<Vec<(String, usize)>, String> {
        match sec(i)? {
            Some(g) => group(b, g),
            None => Ok(Vec::new()),
        }
    };
    let mut bones = std::collections::HashMap::new();
    for (_, a) in list(i_bones)? {
        let (id, bn) = bone(b, a)?;
        bones.insert(id, bn);
    }
    let arrays = |i: usize, kind: u8| -> Result<std::collections::HashMap<u32, Vec<f32>>, String> {
        let mut m = std::collections::HashMap::new();
        for (_, a) in list(i)? {
            let (id, v, _) = vertex_array(b, a, kind)?;
            m.insert(id, v);
        }
        Ok(m)
    };
    let pos = arrays(i_pos, b'p')?;
    let nrm = arrays(i_nrm, b'n')?;
    let uvs = arrays(i_uv, b'u')?;
    let mut materials = std::collections::BTreeMap::new();
    for (_, a) in list(i_mat)? {
        let (id, m) = material(b, a)?;
        materials.insert(id, m);
    }
    let mut shapes = std::collections::BTreeMap::new();
    for (_, a) in list(i_shape)? {
        let (id, s) = shape(b, a, version)?;
        shapes.insert(id, s);
    }
    let mut calls = Vec::new();
    for (name, a) in list(i_tree)? {
        match name.as_str() {
            "DrawOpa" => draws(b, a, false, &mut calls)?,
            "DrawXlu" => draws(b, a, true, &mut calls)?,
            _ => {}
        }
    }
    let mut model = Model { name, materials: materials.values().cloned().collect(), ..Default::default() };
    let drawn: std::collections::HashSet<u32> = calls.iter().map(|c| c.1).collect();
    model.skipped_shapes = shapes.keys().filter(|k| !drawn.contains(k)).count();
    for (mat_id, shape_id, xlu) in calls {
        let (Some(mat), Some(s)) = (materials.get(&mat_id), shapes.get(&shape_id)) else { return Err(bad!()) };
        let sampler = mat.samplers.iter().position(|x| !is_light_map(&x.texture) && x.uv_set.is_some());
        let uv_set = sampler.and_then(|i| mat.samplers[i].uv_set).unwrap_or(0) as usize;
        let srt = sampler.and_then(|i| mat.samplers[i].srt);
        let tris = primitives(b, s)?;
        let p_arr = pos.get(&(s.pos as u32)).filter(|_| s.pos >= 0).ok_or_else(|| bad!())?;
        let n_arr = if s.nrm >= 0 { nrm.get(&(s.nrm as u32)) } else { None };
        let set = if s.uv[uv_set] >= 0 { uv_set } else { s.uv.iter().position(|u| *u >= 0).unwrap_or(0) };
        let u_arr = if s.uv[set] >= 0 { uvs.get(&(s.uv[set] as u32)) } else { None };
        let mut sm = Submesh { material: mat.name.clone(), shape: s.name.clone(), xlu, sampler, ..Default::default() };
        let mut map = std::collections::HashMap::new();
        for t in &tris {
            for v in t {
                let key = (v.mtx, v.pos, v.nrm, v.uv[set]);
                if let Some(id) = map.get(&key) {
                    sm.indices.push(*id);
                    continue;
                }
                let id = map.len() as u32;
                map.insert(key, id);
                {
                    let pi = v.pos as usize * 3;
                    let raw = p_arr.get(pi..pi + 3).ok_or_else(|| bad!())?;
                    let bone = if v.mtx >= 0 { mtx_to_bone.get(v.mtx as usize).copied().unwrap_or(-1) } else { -1 };
                    let m = if bone >= 0 { bones.get(&(bone as u32)).map(|x| &x.model) } else { None };
                    let p = [raw[0], raw[1], raw[2]];
                    let p = m.map_or(p, |m| xform(m, p, 1.0));
                    sm.positions.extend_from_slice(&p);
                    let nn = match n_arr {
                        Some(a) if v.nrm != u32::MAX => {
                            let ni = v.nrm as usize * 3;
                            let r = a.get(ni..ni + 3).ok_or_else(|| bad!())?;
                            let n = m.map_or([r[0], r[1], r[2]], |m| xform(m, [r[0], r[1], r[2]], 0.0));
                            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                            [n[0] / l, n[1] / l, n[2] / l]
                        }
                        _ => [0.0, 1.0, 0.0],
                    };
                    sm.normals.extend_from_slice(&nn);
                    let (u, w) = match u_arr {
                        Some(a) if v.uv[set] != u32::MAX => {
                            let ui = v.uv[set] as usize * 2;
                            let r = a.get(ui..ui + 2).ok_or_else(|| bad!())?;
                            (r[0], r[1])
                        }
                        _ => (0.0, 0.0),
                    };
                    let (u, w) = srt.map_or((u, w), |s| apply_srt(&s, u, w));
                    sm.uvs.extend_from_slice(&[u, w]);
                }
                sm.indices.push(id);
            }
        }
        if !sm.indices.is_empty() {
            model.submeshes.push(sm);
        }
    }
    Ok(model)
}

pub fn parse_all(b: &[u8]) -> Result<Vec<Model>, String> {
    models_in(b)?.into_iter().map(|(_, at)| parse(b, at)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_identity_is_noop() {
        let (u, v) = apply_srt(&[1.0, 1.0, 0.0, 0.0, 0.0], 0.25, 0.75);
        assert!((u - 0.25).abs() < 1e-6 && (v - 0.75).abs() < 1e-6);
        let (u, v) = apply_srt(&[2.0, 1.0, 0.0, 0.0, 0.0], 0.25, 0.75);
        assert!((u - 0.5).abs() < 1e-6 && (v - 0.75).abs() < 1e-6);
    }

    #[test]
    fn primitive_decoding_quads_strips_fans() {
        let mut dl = vec![0x80u8, 0, 4];
        for i in 0..4u8 {
            dl.extend_from_slice(&[i, i]);
        }
        dl.extend_from_slice(&[0x98, 0, 4]);
        for i in 0..4u8 {
            dl.extend_from_slice(&[i, i]);
        }
        dl.extend_from_slice(&[0xA0, 0, 5]);
        for i in 0..5u8 {
            dl.extend_from_slice(&[i, i]);
        }
        dl.extend_from_slice(&[0xB8, 0, 1, 0, 0, 0, 0]);
        let s = Shape { name: "s".into(), current_matrix: 0, vcd_lo: 2 << 9, vcd_hi: 2, dl: 0, dl_len: dl.len(), pos: 0, nrm: -1, uv: [0, -1, -1, -1, -1, -1, -1, -1] };
        let tris = primitives(&dl, &s).unwrap();
        assert_eq!(tris.len(), 7);
        assert_eq!(tris[2].map(|v| v.pos), [0, 1, 2]);
        assert_eq!(tris[3].map(|v| v.pos), [2, 1, 3]);
        assert_eq!(tris[4].map(|v| v.pos), [0, 1, 2]);
        assert_eq!(tris[6].map(|v| v.pos), [0, 3, 4]);
    }
}
