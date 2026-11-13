use crate::brres::{be16, be32, cstr, group, offset};
use crate::mdl0::{self, Model};
use crate::parts::wrap;
use crate::{game, skin, u8arc};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Lod {
    Race1,
    Race2,
    Race4,
    Menu,
}

pub fn lod_of(rel: &str) -> Lod {
    if rel == game::DRIVER_SZS {
        Lod::Menu
    } else if rel.ends_with("_2.szs") {
        Lod::Race2
    } else if rel.ends_with("_4.szs") {
        Lod::Race4
    } else {
        Lod::Race1
    }
}

pub fn folders(b: &[u8]) -> Result<Vec<(String, Vec<String>)>, String> {
    let root = be16(b, 12)? as usize;
    let mut out = Vec::new();
    for (folder, sub) in group(b, root + 8)? {
        out.push((folder, group(b, sub)?.into_iter().map(|(n, _)| n).collect()));
    }
    Ok(out)
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatTrack {
    pub anim: String,
    pub material: String,
    pub slot: u32,
    pub frames: u16,
    pub keys: Vec<(f32, String)>,
}

pub fn pat0_tracks(b: &[u8]) -> Result<Vec<PatTrack>, String> {
    let root = be16(b, 12)? as usize;
    let mut out = Vec::new();
    for (folder, sub) in group(b, root + 8)? {
        if folder != "AnmTexPat(NW4R)" {
            continue;
        }
        for (name, at) in group(b, sub)? {
            if b.get(at..at + 4) != Some(b"PAT0") {
                continue;
            }
            let version = be32(b, at + 8)?;
            let h = if version >= 4 { at + 0x28 } else { at + 0x24 };
            let data = offset(at, be32(b, at + 0x10)?, b.len())?;
            let tex_table = offset(at, be32(b, at + 0x14)?, b.len())?;
            let frames = be16(b, h + 8)?;
            let n_tex = be16(b, h + 12)? as usize;
            let mut tex_names = Vec::with_capacity(n_tex);
            for i in 0..n_tex {
                tex_names.push(cstr(b, offset(tex_table, be32(b, tex_table + i * 4)?, b.len())?)?);
            }
            for (mat_name, m) in group(b, data)? {
                let flags = be32(b, m + 4)?;
                let mut field = m + 8;
                for slot in 0..8u32 {
                    let f = (flags >> (slot * 4)) & 0xF;
                    if f & 1 == 0 {
                        continue;
                    }
                    let mut keys = Vec::new();
                    if f & 2 != 0 {
                        let ti = be16(b, field)? as usize;
                        keys.push((0.0, tex_names.get(ti).cloned().unwrap_or_default()));
                    } else {
                        let rel = be32(b, field)?;
                        let sane = |a: usize| be16(b, a).is_ok_and(|n| n >= 1 && n as u32 <= frames as u32 + 2) && be32(b, a + 8).is_ok_and(|f| f32::from_bits(f) == 0.0);
                        let a1 = offset(m, rel, b.len())?;
                        let anim = if sane(a1) { a1 } else { offset(field, rel, b.len())? };
                        let n = be16(b, anim)? as usize;
                        for k in 0..n.min(1024) {
                            let e = anim + 8 + k * 8;
                            let fr = f32::from_bits(be32(b, e)?);
                            let ti = be16(b, e + 4)? as usize;
                            keys.push((fr, tex_names.get(ti).cloned().unwrap_or_default()));
                        }
                    }
                    out.push(PatTrack { anim: name.clone(), material: mat_name.clone(), slot, frames, keys });
                    field += 4;
                }
            }
        }
    }
    Ok(out)
}

pub struct Source {
    pub file: String,
    pub lod: Lod,
    pub model: Model,
}

pub fn driver_brres(root: &Path, rel: &str, code: &str) -> Result<Option<Vec<u8>>, String> {
    let raw = skin::read_archive(root, rel)?;
    let arc = u8arc::parse(&raw).map_err(|_| format!("E:game.badFile|file={rel}"))?;
    let member = if rel == game::DRIVER_SZS { game::menu_member(&arc, code) } else { Some(skin::DRIVER_MEMBER.to_string()) };
    let Some(member) = member else { return Ok(None) };
    let Some(f) = arc.find(&member) else { return Ok(None) };
    Ok(Some(raw[f.offset..f.offset + f.size].to_vec()))
}

pub fn all_sources(root: &Path, ch: &game::Character) -> Result<Vec<Source>, String> {
    let mut files = ch.files.clone();
    files.push(game::DRIVER_SZS.to_string());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for rel in files {
        let Some(b) = driver_brres(root, &rel, &ch.code)? else { continue };
        for model in mdl0::parse_all(&b).map_err(|e| format!("{e}|file={rel}"))? {
            let tris: usize = model.submeshes.iter().map(|s| s.triangles()).sum();
            let texs: Vec<String> = model.submeshes.iter().filter_map(|s| s.texture(&model).map(|t| t.texture.clone())).collect();
            if seen.insert((lod_of(&rel), model.name.clone(), tris, texs)) {
                out.push(Source { lod: lod_of(&rel), file: rel.clone(), model });
            }
        }
    }
    Ok(out)
}

pub fn rasterize(model: &Model, texture: &str, w: u32, h: u32, mask: &mut [u8]) -> usize {
    let tris = tris_of(model, texture);
    let smear = smears(&tris, w, h);
    let mut n = 0;
    for (t, sm) in tris.iter().zip(&smear) {
        if *sm {
            continue;
        }
        let tri = [0, 1, 2].map(|k| [t.uv[k][0] * w as f32, t.uv[k][1] * h as f32]);
        n += 1;
        raster_tri(tri, w, h, t.ws, t.wt, |p| mask[p as usize] = 1);
    }
    n
}

fn areas(t: &Tri, w: u32, h: u32) -> (f32, f32) {
    let uv = [0, 1, 2].map(|k| [t.uv[k][0] * w as f32, t.uv[k][1] * h as f32]);
    let a2 = ((uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1]) - (uv[1][1] - uv[0][1]) * (uv[2][0] - uv[0][0])).abs() / 2.0;
    let c = cross3(sub3(t.p[1], t.p[0]), sub3(t.p[2], t.p[0]));
    (a2, dot3(c, c).sqrt() / 2.0)
}

fn smears(tris: &[Tri], w: u32, h: u32) -> Vec<bool> {
    let dens: Vec<(f32, f32)> = tris.iter().map(|t| areas(t, w, h)).collect();
    let (uv, a3): (f32, f32) = dens.iter().fold((0.0, 0.0), |acc, d| (acc.0 + d.0, acc.1 + d.1));
    if a3 <= 1e-6 {
        return vec![false; tris.len()];
    }
    let mean = uv / a3;
    dens.iter().map(|(uv, a3)| *uv >= 512.0 && *uv / a3.max(1e-6) >= mean * 50.0).collect()
}

fn raster_tri(tri: [[f32; 2]; 3], w: u32, h: u32, ws: u32, wt: u32, mut put: impl FnMut(u32)) {
    let minx = tri.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let maxx = tri.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    let miny = tri.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let maxy = tri.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    if !(minx.is_finite() && maxx.is_finite() && miny.is_finite() && maxy.is_finite()) || maxx - minx > 4.0 * w as f32 || maxy - miny > 4.0 * h as f32 {
        return;
    }
    let area = (tri[1][0] - tri[0][0]) * (tri[2][1] - tri[0][1]) - (tri[1][1] - tri[0][1]) * (tri[2][0] - tri[0][0]);
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    for y in (miny - 0.5).floor() as i64..(maxy + 0.5).ceil() as i64 {
        for x in (minx - 0.5).floor() as i64..(maxx + 0.5).ceil() as i64 {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let inside = (0..3).all(|i| {
                let (a, b) = (tri[i], tri[(i + 1) % 3]);
                let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt().max(1e-6);
                ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) * sign / len > -0.49
            });
            if inside {
                put(wrap(y, h, wt) * w + wrap(x, w, ws));
            }
        }
    }
    let cx = (tri[0][0] + tri[1][0] + tri[2][0]) / 3.0;
    let cy = (tri[0][1] + tri[1][1] + tri[2][1]) / 3.0;
    put(wrap(cy.floor() as i64, h, wt) * w + wrap(cx.floor() as i64, w, ws));
}

pub fn frame_base(name: &str) -> Option<String> {
    let (stem, num) = name.rsplit_once('.')?;
    if num.is_empty() || !num.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{stem}.0"))
}

fn sampled_as<'a>(sources: &'a [Source], name: &str) -> Option<String> {
    let direct = |t: &str| sources.iter().any(|s| s.model.submeshes.iter().any(|m| m.texture(&s.model).is_some_and(|x| x.texture == t)));
    if direct(name) {
        Some(name.to_string())
    } else {
        frame_base(name).filter(|b| direct(b))
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TexUse {
    pub texture: String,
    pub lod: Lod,
    pub file: String,
    pub model: String,
    pub material: String,
    pub shape: String,
    pub triangles: usize,
    pub texels: u32,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TexFacts {
    pub texture: String,
    pub width: u32,
    pub height: u32,
    pub role: String,
    pub used_texels: u32,
    pub mirrored_texels: u32,
    pub smear_triangles: usize,
    pub lods: Vec<Lod>,
    pub derived_from: Option<String>,
    #[serde(skip)]
    pub used: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct LodLink {
    pub low: String,
    pub lw: u32,
    pub lh: u32,
    pub highs: Vec<(String, u32, u32)>,
    pub samples: Vec<i32>,
    pub uv_delta: f32,
    pub dist_rel: f32,
    pub mapped: u32,
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn closest_bary(p: [f32; 3], a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let ab = sub3(b, a);
    let ac = sub3(c, a);
    let ap = sub3(p, a);
    let d1 = dot3(ab, ap);
    let d2 = dot3(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return [1.0, 0.0, 0.0];
    }
    let bp = sub3(p, b);
    let d3 = dot3(ab, bp);
    let d4 = dot3(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return [0.0, 1.0, 0.0];
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [1.0 - v, v, 0.0];
    }
    let cp = sub3(p, c);
    let d5 = dot3(ab, cp);
    let d6 = dot3(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return [0.0, 0.0, 1.0];
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [1.0 - w, 0.0, w];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return [0.0, 1.0 - w, w];
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    [1.0 - v - w, v, w]
}

struct Tri {
    p: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
    n: [f32; 3],
    ws: u32,
    wt: u32,
}

fn tris_of(model: &Model, texture: &str) -> Vec<Tri> {
    tris_where(model, |t| t == texture).into_iter().map(|(_, t)| t).collect()
}

fn tris_where(model: &Model, keep: impl Fn(&str) -> bool) -> Vec<(String, Tri)> {
    let mut out = Vec::new();
    for s in &model.submeshes {
        let Some(smp) = s.texture(model).filter(|x| keep(&x.texture)) else { continue };
        for t in 0..s.triangles() {
            let mut p = [[0f32; 3]; 3];
            let mut uv = [[0f32; 2]; 3];
            for k in 0..3 {
                let v = s.indices[t * 3 + k] as usize;
                p[k] = [s.positions[v * 3], s.positions[v * 3 + 1], s.positions[v * 3 + 2]];
                uv[k] = [s.uvs[v * 2], s.uvs[v * 2 + 1]];
            }
            let n = cross3(sub3(p[1], p[0]), sub3(p[2], p[0]));
            let l = dot3(n, n).sqrt().max(1e-12);
            out.push((smp.texture.clone(), Tri { p, uv, n: [n[0] / l, n[1] / l, n[2] / l], ws: smp.wrap_s, wt: smp.wrap_t }));
        }
    }
    out
}

pub fn lod_link(low_model: &Model, low: &str, lw: u32, lh: u32, high_model: &Model, highs: &[(String, u32, u32)]) -> LodLink {
    let lt = tris_of(low_model, low);
    let ht: Vec<(usize, Tri)> = tris_where(high_model, |t| highs.iter().any(|h| h.0 == t))
        .into_iter()
        .map(|(name, t)| (highs.iter().position(|h| h.0 == name).unwrap(), t))
        .collect();
    let mut samples = vec![-1i32; (lw * lh * 4) as usize];
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (_, t) in &ht {
        for p in &t.p {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
    }
    let size = (0..3).map(|a| hi[a] - lo[a]).fold(0f32, f32::max).max(1e-3);
    let (mut uv_sum, mut d_sum, mut n) = (0f64, 0f64, 0u32);
    for t in &lt {
        let px: Vec<[f32; 2]> = t.uv.iter().map(|u| [u[0] * lw as f32, u[1] * lh as f32]).collect();
        let minx = px.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let maxx = px.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let miny = px.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let maxy = px.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        if !(minx.is_finite() && maxy.is_finite()) || maxx - minx > 4.0 * lw as f32 || maxy - miny > 4.0 * lh as f32 {
            continue;
        }
        let area = (px[1][0] - px[0][0]) * (px[2][1] - px[0][1]) - (px[1][1] - px[0][1]) * (px[2][0] - px[0][0]);
        if area.abs() < 1e-9 {
            continue;
        }
        let sy0 = ((miny - 0.25) * 2.0).floor() as i64;
        let sy1 = ((maxy + 0.25) * 2.0).ceil() as i64;
        let sx0 = ((minx - 0.25) * 2.0).floor() as i64;
        let sx1 = ((maxx + 0.25) * 2.0).ceil() as i64;
        for sy in sy0..sy1 {
            for sx in sx0..sx1 {
                let q = [sx as f32 * 0.5 + 0.25, sy as f32 * 0.5 + 0.25];
                let w1 = ((px[2][0] - px[1][0]) * (q[1] - px[1][1]) - (px[2][1] - px[1][1]) * (q[0] - px[1][0])) / area;
                let w2 = ((px[0][0] - px[2][0]) * (q[1] - px[2][1]) - (px[0][1] - px[2][1]) * (q[0] - px[2][0])) / area;
                let w0 = 1.0 - w1 - w2;
                let (b0, b1, b2) = (w1, w2, w0);
                if b0 < -0.02 || b1 < -0.02 || b2 < -0.02 {
                    continue;
                }
                let p3 = [0, 1, 2].map(|a| t.p[0][a] * b0 + t.p[1][a] * b1 + t.p[2][a] * b2);
                let luv = [t.uv[0][0] * b0 + t.uv[1][0] * b1 + t.uv[2][0] * b2, t.uv[0][1] * b0 + t.uv[1][1] * b1 + t.uv[2][1] * b2];
                let mut best = (f32::MAX, 0usize, [0f32; 3]);
                for (i, (_, h)) in ht.iter().enumerate() {
                    let bc = closest_bary(p3, h.p[0], h.p[1], h.p[2]);
                    let c = [0, 1, 2].map(|a| h.p[0][a] * bc[0] + h.p[1][a] * bc[1] + h.p[2][a] * bc[2]);
                    let d = sub3(p3, c);
                    let d2 = dot3(d, d) + (1.0 - dot3(h.n, t.n)).max(0.0) * (size * 0.03).powi(2);
                    if d2 < best.0 {
                        best = (d2, i, bc);
                    }
                }
                if best.0 == f32::MAX {
                    continue;
                }
                let (src, h) = &ht[best.1];
                let (hw, hh) = (highs[*src].1, highs[*src].2);
                let bc = best.2;
                let huv = [h.uv[0][0] * bc[0] + h.uv[1][0] * bc[1] + h.uv[2][0] * bc[2], h.uv[0][1] * bc[0] + h.uv[1][1] * bc[1] + h.uv[2][1] * bc[2]];
                let hx = wrap((huv[0] * hw as f32).floor() as i64, hw, h.ws);
                let hy = wrap((huv[1] * hh as f32).floor() as i64, hh, h.wt);
                let lx = wrap(sx.div_euclid(2), lw, t.ws) as usize;
                let ly = wrap(sy.div_euclid(2), lh, t.wt) as usize;
                let k = (sy.rem_euclid(2) * 2 + sx.rem_euclid(2)) as usize;
                let slot = &mut samples[(ly * lw as usize + lx) * 4 + k];
                if *slot < 0 {
                    *slot = ((*src as i32) << 24) | (hy * hw + hx) as i32;
                    uv_sum += (((luv[0] - huv[0]).abs() + (luv[1] - huv[1]).abs()) / 2.0) as f64;
                    d_sum += ({ let c = [0, 1, 2].map(|a| h.p[0][a] * bc[0] + h.p[1][a] * bc[1] + h.p[2][a] * bc[2]); let d = sub3(p3, c); dot3(d, d).sqrt() } / size) as f64;
                    n += 1;
                }
            }
        }
    }
    for c in samples.chunks_exact_mut(4) {
        if let Some(&v) = c.iter().find(|v| **v >= 0) {
            for x in c.iter_mut() {
                if *x < 0 {
                    *x = v;
                }
            }
        }
    }
    let mapped = samples.chunks_exact(4).filter(|c| c[0] >= 0).count() as u32;
    LodLink {
        low: low.into(),
        lw,
        lh,
        samples,
        highs: highs.to_vec(),
        uv_delta: if n > 0 { (uv_sum / n as f64) as f32 } else { f32::NAN },
        dist_rel: if n > 0 { (d_sum / n as f64) as f32 } else { f32::NAN },
        mapped,
    }
}

pub const SHARE_MIN_MATCH: f32 = 0.2;
pub const SHARE_MAX_ERR: f32 = 14.0;
pub const SHARE_TEXEL_ERR: f32 = 32.0;
pub const SHARE_EPS: f32 = 0.002;

#[allow(clippy::too_many_arguments)]
pub fn share_link(dst_model: &Model, dst_tex: &str, dst_px: &[u8], dw: u32, dh: u32, src_model: &Model, src_tex: &str, src_px: &[u8], sw: u32, sh: u32, max_rel: f32) -> LodLink {
    let dt = tris_of(dst_model, dst_tex);
    let st = tris_of(src_model, src_tex);
    let (lo, hi) = bbox_of(src_model);
    let size = (0..3).map(|a| hi[a] - lo[a]).fold(0f32, f32::max).max(1e-3);
    let eps = size * max_rel;
    let cell = size / 48.0;
    let key = |p: [f32; 3]| [0, 1, 2].map(|a| ((p[a] - lo[a]) / cell).floor() as i32);
    let mut grid: std::collections::HashMap<[i32; 3], Vec<u32>> = Default::default();
    for (i, t) in st.iter().enumerate() {
        let mut a = [f32::MAX; 3];
        let mut b = [f32::MIN; 3];
        for p in &t.p {
            for k in 0..3 {
                a[k] = a[k].min(p[k] - eps);
                b[k] = b[k].max(p[k] + eps);
            }
        }
        let (ka, kb) = (key(a), key(b));
        if (0..3).map(|k| (kb[k] - ka[k] + 1) as i64).product::<i64>() > 200_000 {
            continue;
        }
        for x in ka[0]..=kb[0] {
            for y in ka[1]..=kb[1] {
                for z in ka[2]..=kb[2] {
                    grid.entry([x, y, z]).or_default().push(i as u32);
                }
            }
        }
    }
    let mut parent: Vec<usize> = (0..dt.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let mut owner: std::collections::HashMap<[i32; 5], usize> = Default::default();
    for (i, t) in dt.iter().enumerate() {
        for k in 0..3 {
            let v = [(t.p[k][0] * 64.0).round() as i32, (t.p[k][1] * 64.0).round() as i32, (t.p[k][2] * 64.0).round() as i32, (t.uv[k][0] * 8192.0).round() as i32, (t.uv[k][1] * 8192.0).round() as i32];
            match owner.get(&v) {
                Some(&j) => {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                }
                None => {
                    owner.insert(v, i);
                }
            }
        }
    }
    let mut subs: Vec<(u32, u8, u32, i32, u32)> = Vec::new();
    let smear = smears(&dt, dw, dh);
    let (mut uv_sum, mut d_sum, mut n) = (0f64, 0f64, 0u32);
    for (ti, (t, sm)) in dt.iter().zip(&smear).enumerate() {
        if *sm {
            continue;
        }
        let chart = find(&mut parent, ti) as u32;
        let px: Vec<[f32; 2]> = t.uv.iter().map(|u| [u[0] * dw as f32, u[1] * dh as f32]).collect();
        let minx = px.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let maxx = px.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let miny = px.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let maxy = px.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        if !(minx.is_finite() && maxy.is_finite()) || maxx - minx > 4.0 * dw as f32 || maxy - miny > 4.0 * dh as f32 {
            continue;
        }
        let area = (px[1][0] - px[0][0]) * (px[2][1] - px[0][1]) - (px[1][1] - px[0][1]) * (px[2][0] - px[0][0]);
        if area.abs() < 1e-9 {
            continue;
        }
        for sy in ((miny - 0.25) * 2.0).floor() as i64..((maxy + 0.25) * 2.0).ceil() as i64 {
            for sx in ((minx - 0.25) * 2.0).floor() as i64..((maxx + 0.25) * 2.0).ceil() as i64 {
                let q = [sx as f32 * 0.5 + 0.25, sy as f32 * 0.5 + 0.25];
                let b0 = ((px[2][0] - px[1][0]) * (q[1] - px[1][1]) - (px[2][1] - px[1][1]) * (q[0] - px[1][0])) / area;
                let b1 = ((px[0][0] - px[2][0]) * (q[1] - px[2][1]) - (px[0][1] - px[2][1]) * (q[0] - px[2][0])) / area;
                let b2 = 1.0 - b0 - b1;
                if b0 < -0.02 || b1 < -0.02 || b2 < -0.02 {
                    continue;
                }
                let p3 = [0, 1, 2].map(|a| t.p[0][a] * b0 + t.p[1][a] * b1 + t.p[2][a] * b2);
                let duv = [t.uv[0][0] * b0 + t.uv[1][0] * b1 + t.uv[2][0] * b2, t.uv[0][1] * b0 + t.uv[1][1] * b1 + t.uv[2][1] * b2];
                let lx = wrap(sx.div_euclid(2), dw, t.ws);
                let ly = wrap(sy.div_euclid(2), dh, t.wt);
                let k = (sy.rem_euclid(2) * 2 + sx.rem_euclid(2)) as u8;
                let mut best = (f32::MAX, usize::MAX, [0f32; 3]);
                if let Some(cands) = grid.get(&key(p3)) {
                    for &i in cands {
                        let h = &st[i as usize];
                        if dot3(h.n, t.n) < 0.8 {
                            continue;
                        }
                        let bc = closest_bary(p3, h.p[0], h.p[1], h.p[2]);
                        let c = [0, 1, 2].map(|a| h.p[0][a] * bc[0] + h.p[1][a] * bc[1] + h.p[2][a] * bc[2]);
                        let d = sub3(p3, c);
                        let d2 = dot3(d, d);
                        if d2 < best.0 {
                            best = (d2, i as usize, bc);
                        }
                    }
                }
                let mut s = -1;
                if best.1 != usize::MAX && best.0.sqrt() <= eps {
                    let (h, bc) = (&st[best.1], best.2);
                    let suv = [h.uv[0][0] * bc[0] + h.uv[1][0] * bc[1] + h.uv[2][0] * bc[2], h.uv[0][1] * bc[0] + h.uv[1][1] * bc[1] + h.uv[2][1] * bc[2]];
                    let hx = wrap((suv[0] * sw as f32).floor() as i64, sw, h.ws);
                    let hy = wrap((suv[1] * sh as f32).floor() as i64, sh, h.wt);
                    s = (hy * sw + hx) as i32;
                    uv_sum += (((duv[0] - suv[0]).abs() + (duv[1] - suv[1]).abs()) / 2.0) as f64;
                    d_sum += (best.0.sqrt() / size) as f64;
                    n += 1;
                }
                subs.push((ly * dw + lx, k, chart, s, ti as u32));
            }
        }
    }
    let mut stat: std::collections::HashMap<u32, (u32, u32, f64, Vec<u32>)> = Default::default();
    for &(i, _, c, s, _) in &subs {
        let e = stat.entry(c).or_insert_with(|| (0, 0, 0.0, vec![0; 256]));
        e.0 += 1;
        if s >= 0 {
            e.1 += 1;
            let (o, q) = (i as usize * 4, s as usize * 4);
            let err = (0..3).map(|ch| (dst_px[o + ch] as f64 - src_px[q + ch] as f64).abs()).sum::<f64>() / 3.0;
            e.2 += err;
            e.3[(err.round() as usize).min(255)] += 1;
        }
    }
    let median = |h: &[u32], m: u32| {
        let mut acc = 0;
        h.iter().position(|&c| {
            acc += c;
            acc * 2 >= m
        }).unwrap_or(255) as f32
    };
    let shared = |c: u32| stat.get(&c).is_some_and(|(all, m, _, h)| *m as f32 >= *all as f32 * SHARE_MIN_MATCH && median(h, *m) <= SHARE_MAX_ERR);
    let mut samples = vec![-1i32; (dw * dh * 4) as usize];
    let mut bad = vec![false; (dw * dh) as usize];
    for &(i, k, c, s, _) in &subs {
        if s < 0 || !shared(c) {
            bad[i as usize] = true;
        } else if samples[i as usize * 4 + k as usize] < 0 {
            samples[i as usize * 4 + k as usize] = s;
        }
    }
    for (i, c) in samples.chunks_exact_mut(4).enumerate() {
        if bad[i] {
            c.fill(-1);
            continue;
        }
        if let Some(&v) = c.iter().find(|v| **v >= 0) {
            for x in c.iter_mut() {
                if *x < 0 {
                    *x = v;
                }
            }
        }
    }
    let (w, h) = (dw as i64, dh as i64);
    let differs: Vec<bool> = (0..(dw * dh) as usize)
        .map(|i| {
            let c = &samples[i * 4..i * 4 + 4];
            c[0] >= 0 && (0..3).map(|ch| (dst_px[i * 4 + ch] as f32 - c.iter().map(|&s| src_px[s as usize * 4 + ch] as f32).sum::<f32>() / 4.0).abs()).sum::<f32>() / 3.0 > SHARE_TEXEL_ERR
        })
        .collect();
    let morph = |m: &[bool], all: bool| -> Vec<bool> {
        (0..(w * h))
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let mut it = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0 && ny >= 0 && nx < w && ny < h && m[(ny * w + nx) as usize]
                });
                if all { it.all(|v| v) } else { it.any(|v| v) }
            })
            .collect()
    };
    let drop = morph(&morph(&morph(&differs, true), false), false);
    for (i, c) in samples.chunks_exact_mut(4).enumerate() {
        if drop[i] {
            c.fill(-1);
        }
    }
    let mapped = samples.chunks_exact(4).filter(|c| c[0] >= 0).count() as u32;
    LodLink {
        low: dst_tex.into(),
        lw: dw,
        lh: dh,
        samples,
        highs: vec![(src_tex.into(), sw, sh)],
        uv_delta: if n > 0 { (uv_sum / n as f64) as f32 } else { f32::NAN },
        dist_rel: if n > 0 { (d_sum / n as f64) as f32 } else { f32::NAN },
        mapped,
    }
}

pub fn derive_low(link: &LodLink, low_orig: &[u8], orig: &[&[u8]], edit: &[&[u8]]) -> Vec<u8> {
    let mut out = low_orig.to_vec();
    for i in 0..(link.lw * link.lh) as usize {
        let s = &link.samples[i * 4..i * 4 + 4];
        if s[0] < 0 {
            continue;
        }
        let mut delta = [0f32; 4];
        let mut changed = false;
        for &v in s {
            let (src, o) = ((v >> 24) as usize, (v & 0xFF_FFFF) as usize * 4);
            for c in 0..4 {
                let d = edit[src][o + c] as f32 - orig[src][o + c] as f32;
                changed |= d != 0.0;
                delta[c] += d / 4.0;
            }
        }
        if !changed {
            continue;
        }
        for c in 0..4 {
            out[i * 4 + c] = (low_orig[i * 4 + c] as f32 + delta[c]).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

pub fn resample_low(link: &LodLink, low_orig: &[u8], highs: &[&[u8]]) -> Vec<u8> {
    let mut out = low_orig.to_vec();
    for i in 0..(link.lw * link.lh) as usize {
        let s = &link.samples[i * 4..i * 4 + 4];
        if s[0] < 0 {
            continue;
        }
        for c in 0..4 {
            let v: f32 = s.iter().map(|&v| highs[(v >> 24) as usize][(v & 0xFF_FFFF) as usize * 4 + c] as f32).sum::<f32>() / 4.0;
            out[i * 4 + c] = v.round() as u8;
        }
    }
    out
}

fn ray_hit(o: [f32; 3], d: [f32; 3], t: &Tri) -> Option<f32> {
    let e1 = sub3(t.p[1], t.p[0]);
    let e2 = sub3(t.p[2], t.p[0]);
    let h = cross3(d, e2);
    let a = dot3(e1, h);
    if a.abs() < 1e-9 {
        return None;
    }
    let f = 1.0 / a;
    let s = sub3(o, t.p[0]);
    let u = f * dot3(s, h);
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = cross3(s, e1);
    let v = f * dot3(d, q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some((f * dot3(e2, q)).abs())
}

pub fn hidden_under(model: &Model, texture: &str, w: u32, h: u32, cover: impl Fn(&str) -> bool) -> (Vec<u8>, Vec<u8>) {
    let body = tris_of(model, texture);
    let covers: Vec<Tri> = tris_where(model, |t| t != texture && cover(t)).into_iter().map(|(_, t)| t).collect();
    let mut seen = vec![0u8; (w * h) as usize];
    let mut visible = vec![0u8; (w * h) as usize];
    if covers.is_empty() {
        return (vec![0; (w * h) as usize], seen);
    }
    let (lo, hi) = bbox_of(model);
    let size = (0..3).map(|a| hi[a] - lo[a]).fold(0f32, f32::max);
    let maxd = size * 0.03;
    for t in &body {
        let px: Vec<[f32; 2]> = t.uv.iter().map(|u| [u[0] * w as f32, u[1] * h as f32]).collect();
        let area = (px[1][0] - px[0][0]) * (px[2][1] - px[0][1]) - (px[1][1] - px[0][1]) * (px[2][0] - px[0][0]);
        if area.abs() < 1e-9 {
            continue;
        }
        let minx = px.iter().map(|p| p[0]).fold(f32::MAX, f32::min).floor() as i64;
        let maxx = px.iter().map(|p| p[0]).fold(f32::MIN, f32::max).ceil() as i64;
        let miny = px.iter().map(|p| p[1]).fold(f32::MAX, f32::min).floor() as i64;
        let maxy = px.iter().map(|p| p[1]).fold(f32::MIN, f32::max).ceil() as i64;
        if maxx - minx > 4 * w as i64 || maxy - miny > 4 * h as i64 {
            continue;
        }
        for y in miny..maxy {
            for x in minx..maxx {
                let q = [x as f32 + 0.5, y as f32 + 0.5];
                let b0 = ((px[2][0] - px[1][0]) * (q[1] - px[1][1]) - (px[2][1] - px[1][1]) * (q[0] - px[1][0])) / area;
                let b1 = ((px[0][0] - px[2][0]) * (q[1] - px[2][1]) - (px[0][1] - px[2][1]) * (q[0] - px[2][0])) / area;
                let b2 = 1.0 - b0 - b1;
                if b0 < -0.01 || b1 < -0.01 || b2 < -0.01 {
                    continue;
                }
                let p3 = [0, 1, 2].map(|a| t.p[0][a] * b0 + t.p[1][a] * b1 + t.p[2][a] * b2);
                let i = (wrap(y, h, t.wt) * w + wrap(x, w, t.ws)) as usize;
                seen[i] = 1;
                if visible[i] != 0 {
                    continue;
                }
                let covered = covers.iter().any(|c| ray_hit(p3, t.n, c).is_some_and(|d| d < maxd));
                if !covered {
                    visible[i] = 1;
                }
            }
        }
    }
    let hidden = seen.iter().zip(&visible).map(|(s, v)| u8::from(*s != 0 && *v == 0)).collect();
    (hidden, seen)
}

pub struct CharMap {
    pub uses: Vec<TexUse>,
    pub facts: Vec<TexFacts>,
    pub links: Vec<LodLink>,
    pub shares: Vec<LodLink>,
    pub pat0: Vec<(Lod, PatTrack)>,
}

fn body_texture(m: &Model) -> Option<String> {
    let mut c: Vec<(String, usize)> = Vec::new();
    for s in &m.submeshes {
        let Some(t) = s.texture(m) else { continue };
        if skin::is_eye(&t.texture) {
            continue;
        }
        match c.iter_mut().find(|x| x.0 == t.texture) {
            Some(x) => x.1 += s.triangles(),
            None => c.push((t.texture.clone(), s.triangles())),
        }
    }
    c.into_iter().max_by_key(|x| x.1).map(|x| x.0)
}

pub fn outfit_shares(sources: &[Source], originals: &std::collections::HashMap<String, crate::texture::Rgba8>) -> Vec<LodLink> {
    let pick = |v: &str| sources.iter().find(|s| s.lod == Lod::Race1 && !is_lod_model(&s.model.name) && crate::model::vehicle_of("race", &s.file) == Some(v));
    let (Some(k), Some(b)) = (pick("kart"), pick("bike")) else { return Vec::new() };
    let (Some(kt), Some(bt)) = (body_texture(&k.model), body_texture(&b.model)) else { return Vec::new() };
    let (Some(ki), Some(bi)) = (originals.get(&kt), originals.get(&bt)) else { return Vec::new() };
    if kt == bt {
        return Vec::new();
    }
    vec![
        share_link(&b.model, &bt, &bi.pixels, bi.width, bi.height, &k.model, &kt, &ki.pixels, ki.width, ki.height, SHARE_EPS),
        share_link(&k.model, &kt, &ki.pixels, ki.width, ki.height, &b.model, &bt, &bi.pixels, bi.width, bi.height, SHARE_EPS),
    ]
}

fn is_lod_model(name: &str) -> bool {
    name.to_ascii_lowercase().contains("lod")
}

fn bbox_of(m: &Model) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for s in &m.submeshes {
        for p in s.positions.chunks_exact(3) {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
    }
    (lo, hi)
}
fn bbox_dist(a: &([f32; 3], [f32; 3]), b: &([f32; 3], [f32; 3])) -> f32 {
    (0..3).map(|i| (a.0[i] - b.0[i]).abs() + (a.1[i] - b.1[i]).abs()).sum()
}

pub fn analyze(root: &Path, ch: &game::Character, sizes: &[(String, u32, u32)]) -> Result<CharMap, String> {
    analyze_with(root, ch, sizes, None)
}

pub fn analyze_with(root: &Path, ch: &game::Character, sizes: &[(String, u32, u32)], originals: Option<&std::collections::HashMap<String, crate::texture::Rgba8>>) -> Result<CharMap, String> {
    let sources = all_sources(root, ch)?;
    let mut uses = Vec::new();
    for s in &sources {
        for sm in &s.model.submeshes {
            let Some(smp) = sm.texture(&s.model) else { continue };
            let Some((_, w, h)) = sizes.iter().find(|(n, _, _)| *n == smp.texture) else { continue };
            let mut mask = vec![0u8; (*w * *h) as usize];
            let one = Model { name: s.model.name.clone(), materials: s.model.materials.clone(), submeshes: vec![sm.clone()], skipped_shapes: 0 };
            rasterize(&one, &smp.texture, *w, *h, &mut mask);
            uses.push(TexUse {
                texture: smp.texture.clone(),
                lod: s.lod,
                file: s.file.clone(),
                model: s.model.name.clone(),
                material: sm.material.clone(),
                shape: sm.shape.clone(),
                triangles: sm.triangles(),
                texels: mask.iter().map(|&v| v as u32).sum(),
            });
        }
    }
    let mut facts = Vec::new();
    let mut links = Vec::new();
    for (name, w, h) in sizes {
        let (w, h) = (*w, *h);
        let mapped = sampled_as(&sources, name);
        let mut used = vec![0u8; (w * h) as usize];
        let mut left = vec![false; (w * h) as usize];
        let mut right = vec![false; (w * h) as usize];
        let mut lods = Vec::new();
        let mut only_lod_models = true;
        let mut smear_tris = 0usize;
        if let Some(m) = &mapped {
            for s in &sources {
                let n = rasterize(&s.model, m, w, h, &mut used);
                if n > 0 {
                    if !lods.contains(&s.lod) {
                        lods.push(s.lod);
                    }
                    only_lod_models &= is_lod_model(&s.model.name);
                    let (lo, hi) = bbox_of(&s.model);
                    let xc = (lo[0] + hi[0]) / 2.0;
                    let eps = (hi[0] - lo[0]) * 0.04;
                    let tris = tris_of(&s.model, m);
                    let sm = smears(&tris, w, h);
                    smear_tris += sm.iter().filter(|x| **x).count();
                    for (t, smeared) in tris.iter().zip(&sm) {
                        let cx = (t.p[0][0] + t.p[1][0] + t.p[2][0]) / 3.0;
                        if *smeared || (cx - xc).abs() < eps {
                            continue;
                        }
                        let side = if cx > xc { &mut right } else { &mut left };
                        let tri = [0, 1, 2].map(|k| [t.uv[k][0] * w as f32, t.uv[k][1] * h as f32]);
                        raster_tri(tri, w, h, t.ws, t.wt, |p| side[p as usize] = true);
                    }
                }
            }
        }
        lods.sort();
        let role = if mapped.as_deref() != Some(name.as_str()) && mapped.is_some() || skin::is_eye(name) {
            "eye"
        } else if mapped.is_some() && only_lod_models {
            "low"
        } else if mapped.is_some() {
            "body"
        } else {
            "other"
        };
        let mut derived_from = None;
        if role == "low" {
            let low_src = sources.iter().find(|s| is_lod_model(&s.model.name) && s.model.submeshes.iter().any(|m| m.texture(&s.model).is_some_and(|x| &x.texture == name)));
            if let Some(ls) = low_src {
                let lb = bbox_of(&ls.model);
                let full = sources
                    .iter()
                    .filter(|s| !is_lod_model(&s.model.name) && s.model.submeshes.iter().any(|m| m.texture(&s.model).is_some_and(|x| !skin::is_eye(&x.texture))))
                    .min_by(|a, b| bbox_dist(&bbox_of(&a.model), &lb).partial_cmp(&bbox_dist(&bbox_of(&b.model), &lb)).unwrap_or(std::cmp::Ordering::Equal));
                if let Some(fs) = full {
                    let mut highs: Vec<(String, u32, u32, usize)> = Vec::new();
                    for m in &fs.model.submeshes {
                        let Some(t) = m.texture(&fs.model) else { continue };
                        let Some((_, hw, hh)) = sizes.iter().find(|(n, _, _)| *n == t.texture) else { continue };
                        match highs.iter_mut().find(|h| h.0 == t.texture) {
                            Some(h) => h.3 += m.triangles(),
                            None => highs.push((t.texture.clone(), *hw, *hh, m.triangles())),
                        }
                    }
                    highs.sort_by_key(|h| (skin::is_eye(&h.0), std::cmp::Reverse(h.3)));
                    if let Some(first) = highs.first().filter(|h| !skin::is_eye(&h.0)) {
                        derived_from = Some(first.0.clone());
                        let hs: Vec<(String, u32, u32)> = highs.iter().map(|h| (h.0.clone(), h.1, h.2)).collect();
                        links.push(lod_link(&ls.model, name, w, h, &fs.model, &hs));
                    }
                }
            }
        }
        let mirrored_texels = left.iter().zip(&right).filter(|(a, b)| **a && **b).count() as u32;
        facts.push(TexFacts {
            texture: name.clone(),
            width: w,
            height: h,
            role: role.into(),
            used_texels: used.iter().map(|&v| v as u32).sum(),
            mirrored_texels,
            smear_triangles: smear_tris,
            lods,
            derived_from,
            used,
        });
    }
    let mut pat0 = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut files = ch.files.clone();
    files.push(game::DRIVER_SZS.to_string());
    for rel in files {
        let lod = lod_of(&rel);
        if !seen.insert(lod) {
            continue;
        }
        if let Some(b) = driver_brres(root, &rel, &ch.code)? {
            for t in pat0_tracks(&b).unwrap_or_default() {
                pat0.push((lod, t));
            }
        }
    }
    let shares = originals.map(|o| outfit_shares(&sources, o)).unwrap_or_default();
    Ok(CharMap { uses, facts, links, shares, pat0 })
}

pub fn runs(mask: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < mask.len() {
        if mask[i] != 0 {
            let s = i;
            while i < mask.len() && mask[i] != 0 {
                i += 1;
            }
            out.extend_from_slice(&[s as u32, (i - s) as u32]);
        } else {
            i += 1;
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdl0::{Material, Sampler, Submesh};
    use std::path::PathBuf;

    fn quad(sm: &mut Submesh, uv: [f32; 4], x: [f32; 2], y: [f32; 2], z: f32) {
        let base = (sm.positions.len() / 3) as u32;
        let [u0, v0, u1, v1] = uv;
        for (u, v, px, py) in [(u0, v0, x[0], y[1]), (u1, v0, x[1], y[1]), (u1, v1, x[1], y[0]), (u0, v1, x[0], y[0])] {
            sm.positions.extend_from_slice(&[px, py, z]);
            sm.normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            sm.uvs.extend_from_slice(&[u, v]);
        }
        sm.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    fn model(tex: &str, quads: &[([f32; 4], [f32; 2], [f32; 2])]) -> Model {
        let mut sm = Submesh { material: format!("{tex}_m"), shape: "p".into(), sampler: Some(0), ..Default::default() };
        for (uv, x, y) in quads {
            quad(&mut sm, *uv, *x, *y, 0.0);
        }
        Model { name: "model".into(), materials: vec![Material { name: format!("{tex}_m"), samplers: vec![Sampler { texture: tex.into(), wrap_s: 1, wrap_t: 1, uv_set: Some(0), srt: None }] }], submeshes: vec![sm], skipped_shapes: 0 }
    }

    #[test]
    fn lod_link_follows_the_surface_and_derive_keeps_untouched_texels() {
        let low = model("low", &[([0.0, 0.0, 1.0, 1.0], [0.0, 10.0], [0.0, 10.0])]);
        let high = model("all", &[([0.5, 0.0, 1.0, 1.0], [0.0, 5.0], [0.0, 10.0]), ([0.0, 0.0, 0.5, 1.0], [5.0, 10.0], [0.0, 10.0])]);
        let l = lod_link(&low, "low", 8, 8, &high, &[("all".into(), 16, 8)]);
        assert_eq!(l.mapped, 64);
        assert!(l.dist_rel < 1e-3, "{}", l.dist_rel);
        let s = l.samples[(4 * 8 + 1) * 4];
        assert!(s >= 0 && (s & 0xFF_FFFF) as u32 % 16 >= 8, "{s}");
        let lo = vec![100u8; 8 * 8 * 4];
        let ho = vec![50u8; 16 * 8 * 4];
        assert_eq!(derive_low(&l, &lo, &[&ho], &[&ho]), lo);
        let mut he = ho.clone();
        for y in 0..8 {
            for x in 8..16 {
                he[(y * 16 + x) * 4..(y * 16 + x) * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let d = derive_low(&l, &lo, &[&ho], &[&he]);
        assert_eq!(&d[(4 * 8 + 1) * 4..(4 * 8 + 1) * 4 + 3], &[255, 255, 255]);
        assert_eq!(&d[(4 * 8 + 6) * 4..(4 * 8 + 6) * 4 + 3], &[100, 100, 100]);
    }

    #[test]
    fn smear_triangles_are_not_counted_as_shown() {
        let m = model("all", &[([0.0, 0.0, 0.5, 1.0], [0.0, 64.0], [0.0, 64.0]), ([0.0, 0.0, 1.0, 1.0], [100.0, 101.0], [0.0, 1.0])]);
        let mut mask = vec![0u8; 64 * 32];
        let n = rasterize(&m, "all", 64, 32, &mut mask);
        assert_eq!(n, 2, "the two smear triangles are skipped");
        let used: u32 = mask.iter().map(|&v| v as u32).sum();
        assert!((32 * 32..=34 * 33).contains(&used), "{used}");
        assert_eq!(mask[10 * 64 + 50], 0, "right half only reached by the smear");
    }

    fn game_dir() -> Option<PathBuf> {
        game::resolve_root(&PathBuf::from(std::env::var_os("MKSS_TEST_GAME_DIR")?))
    }
    fn sizes_of(src: &Path, ch: &game::Character) -> (skin::Discovered, Vec<(String, u32, u32)>) {
        let found = skin::discover(src, ch).unwrap();
        let sizes = found.textures.iter().map(|t| (t.name.clone(), t.width, t.height)).collect();
        (found, sizes)
    }

    #[test]
    fn real_bbmario_eye_area_coverage() {
        let Some(src) = game_dir() else { return };
        let ch = game::scan(&src).unwrap().into_iter().find(|c| c.code == "bmr").unwrap();
        let (_, sizes) = sizes_of(&src, &ch);
        let map = analyze(&src, &ch, &sizes).unwrap();
        let all = map.facts.iter().find(|f| f.texture == "bbmario_all").unwrap();
        let mut in_rect = 0;
        let mut used_in_rect = 0;
        for y in 37..58 {
            for x in 142..153 {
                in_rect += 1;
                used_in_rect += all.used[(y * all.width + x) as usize] as u32;
            }
        }
        println!("bbmario_all used {}/{} texels; eye-drawing rect {used_in_rect}/{in_rect} used; smear tris {}", all.used_texels, all.width * all.height, all.smear_triangles);
        assert_eq!(used_in_rect, 0);
        assert_eq!(all.smear_triangles, 32, "8 per distinct full model (race 1P, 2P, 4P cpu, menu)");
        assert!(all.used_texels > 15000 && all.used_texels < 18500);
        let users = |t: &str| map.uses.iter().filter(|u| u.texture == t).map(|u| format!("{:?}:{}:{}", u.lod, u.model, u.material)).collect::<std::collections::BTreeSet<_>>();
        println!("bbmario_eye.0 users {:?}\nbb_mario_tx_64 users {:?}", users("bbmario_eye.0"), users("bb_mario_tx_64"));
        assert!(users("bbmario_eye.0").iter().all(|u| u.ends_with("bbmario_eye_tx")) && users("bbmario_eye.0").len() == 4);
        assert!(users("bb_mario_tx_64").iter().all(|u| u.contains("model_lod")) && !users("bb_mario_tx_64").iter().any(|u| u.starts_with("Menu")));
        assert!(map.uses.iter().all(|u| !u.texture.starts_with("bbmario_eye.") || u.texture == "bbmario_eye.0"), "frames 1..6 are only swapped in by PAT0");
        let race_keys: std::collections::BTreeSet<String> = map.pat0.iter().filter(|(l, _)| *l != Lod::Menu).flat_map(|(_, t)| t.keys.iter().map(|k| k.1.clone())).collect();
        println!("race PAT0 frames {race_keys:?}");
        assert!(race_keys.contains("bbmario_eye.4") && !race_keys.contains("bbmario_eye.3"));
        let wink = map.pat0.iter().find(|(l, t)| *l == Lod::Race1 && t.anim == "wink").unwrap();
        assert_eq!(wink.1.keys[0].1, "bbmario_eye.0", "rest frame");
        let eyes = crate::model::eye_groups(&map);
        assert_eq!(eyes.len(), 1);
        assert_eq!(eyes[0].frames.len(), 7);
    }

    #[test]
    fn real_kart_bike_share_links() {
        let Some(src) = game_dir() else { return };
        let mut report = Vec::new();
        for ch in game::scan(&src).unwrap() {
            let (found, sizes) = sizes_of(&src, &ch);
            let map = analyze_with(&src, &ch, &sizes, Some(&found.originals)).unwrap();
            if !["pc", "ds", "rs"].contains(&ch.code.as_str()) {
                assert!(map.shares.is_empty(), "{}: same atlas on kart and bike, no link", ch.code);
                continue;
            }
            assert_eq!(map.shares.len(), 2, "{}: kart <-> bike", ch.code);
            let ms = crate::model::load_sources(&src, &ch).unwrap();
            let parts = crate::parts::compute(&ms, &sizes);
            for l in &map.shares {
                let dst = &found.originals[&l.low];
                let srcimg = &found.originals[&l.highs[0].0];
                assert_eq!(derive_low(l, &dst.pixels, &[&srcimg.pixels], &[&srcimg.pixels]), dst.pixels, "{} {}: untouched must stay bit-identical", ch.code, l.low);
                let rs = resample_low(l, &dst.pixels, &[&srcimg.pixels]);
                let mut errs: Vec<f32> = (0..(l.lw * l.lh) as usize).filter(|&i| l.samples[i * 4] >= 0).map(|i| (0..3).map(|c| (rs[i * 4 + c] as f32 - dst.pixels[i * 4 + c] as f32).abs()).sum::<f32>() / 3.0).collect();
                errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mean = errs.iter().sum::<f32>() / errs.len().max(1) as f32;
                let p = |q: f32| errs[((errs.len() as f32 * q) as usize).min(errs.len() - 1)];
                let used = map.facts.iter().find(|f| f.texture == l.low).unwrap();
                let tp = parts.textures.iter().find(|t| t.texture == l.low).unwrap();
                let per: Vec<String> = tp
                    .parts
                    .iter()
                    .map(|pt| {
                        let mut n = 0u32;
                        for r in pt.runs.chunks(2) {
                            for i in r[0]..r[0] + r[1] {
                                n += u32::from(l.samples[i as usize * 4] >= 0);
                            }
                        }
                        format!("{}{} {}%", pt.key, if pt.n > 0 { pt.n.to_string() } else { String::new() }, (100 * n + pt.pixels / 2) / pt.pixels.max(1))
                    })
                    .collect();
                let line = format!(
                    "{} {} <- {}: linked {}/{} used texels ({:.1}%); resampled-original error on linked texels mean {mean:.2} p50 {:.1} p90 {:.1} p99 {:.1} max {:.1}; dist {:.5}\n      parts: {}",
                    ch.code,
                    l.low,
                    l.highs[0].0,
                    l.mapped,
                    used.used_texels,
                    100.0 * l.mapped as f32 / used.used_texels as f32,
                    p(0.5),
                    p(0.9),
                    p(0.99),
                    p(1.0),
                    l.dist_rel,
                    per.join(", ")
                );
                assert!(l.mapped as f32 > used.used_texels as f32 * 0.25, "{line}");
                assert!(mean < 3.0 && p(0.9) < 6.0 && p(0.99) < 32.0, "{line}");
                assert!(l.dist_rel < 0.001, "{line}");
                report.push(line);
            }
        }
        println!("{}", report.join("\n"));
        assert_eq!(report.len(), 6);
    }

    #[test]
    fn real_tx64_correspondence_quality() {
        let Some(src) = game_dir() else { return };
        let mut lines = Vec::new();
        for ch in game::scan(&src).unwrap() {
            let (found, sizes) = sizes_of(&src, &ch);
            let map = analyze(&src, &ch, &sizes).unwrap();
            let lows: Vec<&TexFacts> = map.facts.iter().filter(|f| f.role == "low").collect();
            assert!(!lows.is_empty(), "{}: no low-detail texture", ch.code);
            for f in lows {
                let l = map.links.iter().find(|l| l.low == f.texture).unwrap_or_else(|| panic!("{} {}: no link", ch.code, f.texture));
                let lo = &found.originals[&l.low];
                let hs: Vec<&[u8]> = l.highs.iter().map(|h| found.originals[&h.0].pixels.as_slice()).collect();
                assert_eq!(derive_low(l, &lo.pixels, &hs, &hs), lo.pixels, "{} {}", ch.code, l.low);
                let rs = resample_low(l, &lo.pixels, &hs);
                let (mut e, mut n) = (0f64, 0u32);
                for i in 0..(l.lw * l.lh) as usize {
                    if l.samples[i * 4] >= 0 && lo.pixels[i * 4 + 3] > 0 {
                        e += (0..3).map(|c| (rs[i * 4 + c] as f64 - lo.pixels[i * 4 + c] as f64).abs()).sum::<f64>() / 3.0;
                        n += 1;
                    }
                }
                let err = e / n.max(1) as f64;
                let share = l.mapped as f32 / (l.lw * l.lh) as f32;
                lines.push(format!("{} {} {}x{}: mapped {:.0}% dist {:.2}% err {:.1} from {:?}", ch.code, l.low, l.lw, l.lh, share * 100.0, l.dist_rel * 100.0, err, l.highs.iter().map(|h| h.0.as_str()).collect::<Vec<_>>()));
                assert!(share > 0.5, "{}", lines.last().unwrap());
                if !(ch.code == "ds" && l.low == "daisy_bike_suit_tx_64") {
                    assert!(l.dist_rel < 0.035 && err < 50.0, "{}", lines.last().unwrap());
                }
            }
        }
        println!("{}", lines.join("\n"));
    }
}
