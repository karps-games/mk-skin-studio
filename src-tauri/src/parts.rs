use crate::mdl0::{Model, Sampler};
use std::collections::HashMap;

pub struct ModelSource {
    pub kind: &'static str,
    pub file: String,
    pub model: Model,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub key: String,
    pub n: u32,
    pub material: String,
    pub pixels: u32,
    pub bbox: [u32; 4],
    #[serde(skip)]
    pub runs: Vec<u32>,
    #[serde(skip)]
    pub outline: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct TextureParts {
    pub texture: String,
    pub width: u32,
    pub height: u32,
    pub parts: Vec<Part>,
    pub model: Option<usize>,
    pub alias_of: Option<String>,
}

pub struct Computed {
    pub textures: Vec<TextureParts>,
    pub tri_parts: Vec<Vec<Vec<u16>>>,
}

pub fn wrap(i: i64, n: u32, mode: u32) -> u32 {
    let n64 = n as i64;
    match mode {
        1 => i.rem_euclid(n64) as u32,
        2 => {
            let m = i.rem_euclid(2 * n64);
            (if m < n64 { m } else { 2 * n64 - 1 - m }) as u32
        }
        _ => i.clamp(0, n64 - 1) as u32,
    }
}

fn frame_base(name: &str) -> Option<String> {
    let (stem, num) = name.rsplit_once('.')?;
    if num.is_empty() || !num.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{stem}.0"))
}

fn raster(tri: [[f32; 2]; 3], w: u32, h: u32, ws: u32, wt: u32, mut put: impl FnMut(u32)) -> bool {
    let minx = tri.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
    let maxx = tri.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
    let miny = tri.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let maxy = tri.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    if !(minx.is_finite() && maxx.is_finite() && miny.is_finite() && maxy.is_finite()) || maxx - minx > 4.0 * w as f32 || maxy - miny > 4.0 * h as f32 {
        return false;
    }
    let area = (tri[1][0] - tri[0][0]) * (tri[2][1] - tri[0][1]) - (tri[1][1] - tri[0][1]) * (tri[2][0] - tri[0][0]);
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    let edges: Vec<([f32; 2], [f32; 2], f32)> = (0..3)
        .map(|i| {
            let a = tri[i];
            let b = tri[(i + 1) % 3];
            let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt().max(1e-6);
            (a, b, len)
        })
        .collect();
    let mut any = false;
    let (x0, x1) = ((minx - 0.5).floor() as i64, (maxx + 0.5).ceil() as i64);
    let (y0, y1) = ((miny - 0.5).floor() as i64, (maxy + 0.5).ceil() as i64);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let inside = edges.iter().all(|(a, b, len)| {
                let e = ((b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])) * sign / len;
                e > -0.49
            });
            if inside {
                any = true;
                put(wrap(y, h, wt) * w + wrap(x, w, ws));
            }
        }
    }
    any
}

fn keyword_key(material: &str) -> Option<&'static str> {
    let m = material.to_ascii_lowercase();
    const TABLE: &[(&str, &str)] = &[
        ("eye", "eyes"),
        ("hair", "hair"),
        ("braid", "hair"),
        ("cap", "cap"),
        ("hat", "cap"),
        ("crown", "crown"),
        ("shell", "shell"),
        ("koura", "shell"),
        ("dress", "dress"),
        ("emblem", "emblem"),
        ("mark", "emblem"),
        ("glove", "gloves"),
        ("hand", "gloves"),
        ("shoe", "shoes"),
        ("boot", "shoes"),
        ("skin", "skin"),
        ("face", "face"),
    ];
    TABLE.iter().find(|(k, _)| m.contains(k)).map(|(_, v)| *v)
}

struct Island {
    material: String,
    pixels: Vec<u32>,
    tris: Vec<(usize, usize)>,
    centroid: [f32; 3],
    normal: [f32; 3],
    votes: HashMap<&'static str, f32>,
}

fn tri_info(m: &Model, si: usize, t: usize) -> ([f32; 3], [f32; 3]) {
    let s = &m.submeshes[si];
    let mut c = [0f32; 3];
    let mut n = [0f32; 3];
    for k in 0..3 {
        let v = s.indices[t * 3 + k] as usize;
        for a in 0..3 {
            c[a] += s.positions[v * 3 + a] / 3.0;
            n[a] += s.normals.get(v * 3 + a).copied().unwrap_or(0.0) / 3.0;
        }
    }
    (c, n)
}

fn tri_area(m: &Model, si: usize, t: usize) -> f32 {
    let s = &m.submeshes[si];
    let p = |k: usize| {
        let v = s.indices[t * 3 + k] as usize * 3;
        [s.positions[v], s.positions[v + 1], s.positions[v + 2]]
    };
    let (a, b, c) = (p(0), p(1), p(2));
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let x = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt() + 1e-6
}

fn bbox(m: &Model) -> ([f32; 3], [f32; 3]) {
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

fn region(c: [f32; 3], normal: [f32; 3], lo: [f32; 3], hi: [f32; 3], forward: f32) -> &'static str {
    let span = |a: usize| (hi[a] - lo[a]).max(1e-3);
    let yn = (c[1] - lo[1]) / span(1);
    let xn = c[0] / (span(0) / 2.0);
    let nl = (normal.iter().map(|v| v * v).sum::<f32>()).sqrt().max(1e-6);
    let facing = normal[2] / nl * forward;
    if yn >= 0.70 && xn < 0.5 {
        if facing > 0.5 && xn < 0.35 { "face" } else { "head" }
    } else if xn >= 0.68 {
        "hands"
    } else if xn >= 0.40 && yn >= 0.35 {
        "arms"
    } else if yn < 0.16 {
        "feet"
    } else if yn < 0.40 {
        "legs"
    } else {
        "torso"
    }
}

fn geometric_key(isl: &Island) -> &'static str {
    isl.votes.iter().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).map(|(k, _)| *k).unwrap_or("other")
}

fn runs_and_outline(mask: &[bool], w: u32, h: u32) -> (Vec<u32>, Vec<u16>, [u32; 4], u32) {
    let mut runs = Vec::new();
    let mut i = 0usize;
    let mut count = 0;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    while i < mask.len() {
        if mask[i] {
            let s = i;
            while i < mask.len() && mask[i] {
                let (x, y) = ((i as u32) % w, (i as u32) / w);
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
                i += 1;
            }
            runs.extend_from_slice(&[s as u32, (i - s) as u32]);
            count += (i - s) as u32;
        } else {
            i += 1;
        }
    }
    let at = |x: i64, y: i64| x >= 0 && y >= 0 && x < w as i64 && y < h as i64 && mask[(y as u32 * w + x as u32) as usize];
    let mut out = Vec::new();
    for y in 0..=h as i64 {
        let mut start: Option<i64> = None;
        let mut kind = 0;
        for x in 0..=w as i64 {
            let k = if x < w as i64 { (at(x, y - 1) as i32) - (at(x, y) as i32) } else { 0 };
            if k != kind {
                if let Some(s) = start {
                    out.extend_from_slice(&[s as u16, y as u16, x as u16, y as u16]);
                }
                start = if k != 0 { Some(x) } else { None };
                kind = k;
            }
        }
    }
    for x in 0..=w as i64 {
        let mut start: Option<i64> = None;
        let mut kind = 0;
        for y in 0..=h as i64 {
            let k = if y < h as i64 { (at(x - 1, y) as i32) - (at(x, y) as i32) } else { 0 };
            if k != kind {
                if let Some(s) = start {
                    out.extend_from_slice(&[x as u16, s as u16, x as u16, y as u16]);
                }
                start = if k != 0 { Some(y) } else { None };
                kind = k;
            }
        }
    }
    (runs, out, [x0.min(x1), y0.min(y1), x1, y1], count)
}

pub fn compute(models: &[ModelSource], textures: &[(String, u32, u32)]) -> Computed {
    let mut tri_parts: Vec<Vec<Vec<u16>>> = models.iter().map(|m| m.model.submeshes.iter().map(|s| vec![u16::MAX; s.triangles()]).collect()).collect();
    let mut out = Vec::new();
    let sampler_of = |m: &Model, si: usize| -> Option<Sampler> { m.submeshes[si].texture(m).cloned() };
    let referenced = |name: &str| models.iter().any(|ms| (0..ms.model.submeshes.len()).any(|si| sampler_of(&ms.model, si).is_some_and(|s| s.texture == name)));
    for (name, w, h) in textures {
        let (w, h) = (*w, *h);
        let mapped = if referenced(name) {
            Some((name.clone(), None))
        } else {
            frame_base(name).filter(|b| referenced(b)).map(|b| (b.clone(), Some(b)))
        };
        let Some((mapped, alias_of)) = mapped else {
            out.push(TextureParts { texture: name.clone(), width: w, height: h, parts: Vec::new(), model: None, alias_of: None });
            continue;
        };
        let mi = models.iter().position(|ms| (0..ms.model.submeshes.len()).any(|si| sampler_of(&ms.model, si).is_some_and(|s| s.texture == mapped))).unwrap();
        let model = &models[mi].model;
        let (lo, hi) = bbox(model);
        let eye_z: Vec<f32> = model
            .submeshes
            .iter()
            .filter(|s| s.material.to_ascii_lowercase().contains("eye"))
            .flat_map(|s| s.positions.chunks_exact(3).map(|p| p[2]).collect::<Vec<_>>())
            .collect();
        let forward = if eye_z.is_empty() { 1.0 } else if eye_z.iter().sum::<f32>() / eye_z.len() as f32 >= (lo[2] + hi[2]) / 2.0 { 1.0 } else { -1.0 };
        let mut mats: Vec<String> = Vec::new();
        let mut tri_px: Vec<(usize, usize, usize, Vec<u32>)> = Vec::new();
        for (si, s) in model.submeshes.iter().enumerate() {
            let Some(smp) = sampler_of(model, si).filter(|x| x.texture == mapped) else { continue };
            let mi = match mats.iter().position(|m| *m == s.material) {
                Some(i) => i,
                None => {
                    mats.push(s.material.clone());
                    mats.len() - 1
                }
            };
            for t in 0..s.triangles() {
                let mut tri = [[0f32; 2]; 3];
                for k in 0..3 {
                    let v = s.indices[t * 3 + k] as usize;
                    tri[k] = [s.uvs[v * 2] * w as f32, s.uvs[v * 2 + 1] * h as f32];
                }
                let mut px = Vec::new();
                if !raster(tri, w, h, smp.wrap_s, smp.wrap_t, |p| px.push(p)) {
                    let cx = (tri[0][0] + tri[1][0] + tri[2][0]) / 3.0;
                    let cy = (tri[0][1] + tri[1][1] + tri[2][1]) / 3.0;
                    if cx.is_finite() && cy.is_finite() {
                        px.push(wrap(cy.floor() as i64, h, smp.wrap_t) * w + wrap(cx.floor() as i64, w, smp.wrap_s));
                    }
                }
                tri_px.push((mi, si, t, px));
            }
        }
        fn find(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        let mut parent: Vec<usize> = (0..tri_px.len()).collect();
        let mut seen: HashMap<(usize, i64, i64), usize> = HashMap::new();
        for (i, (mi, si, t, _)) in tri_px.iter().enumerate() {
            let s = &model.submeshes[*si];
            for k in 0..3 {
                let v = s.indices[t * 3 + k] as usize;
                let key = (*mi, (s.uvs[v * 2] * 8192.0).round() as i64, (s.uvs[v * 2 + 1] * 8192.0).round() as i64);
                match seen.get(&key) {
                    Some(&j) => {
                        let (ra, rb) = (find(&mut parent, i), find(&mut parent, j));
                        parent[ra] = rb;
                    }
                    None => {
                        seen.insert(key, i);
                    }
                }
            }
        }
        let mut islands: Vec<Island> = Vec::new();
        let mut root_island: HashMap<usize, usize> = HashMap::new();
        for i in 0..tri_px.len() {
            let r = find(&mut parent, i);
            let id = match root_island.get(&r) {
                Some(&id) => id,
                None => {
                    islands.push(Island { material: mats[tri_px[i].0].clone(), pixels: Vec::new(), tris: Vec::new(), centroid: [0.0; 3], normal: [0.0; 3], votes: HashMap::new() });
                    root_island.insert(r, islands.len() - 1);
                    islands.len() - 1
                }
            };
            let (_, si, t, px) = &tri_px[i];
            islands[id].tris.push((*si, *t));
            islands[id].pixels.extend_from_slice(px);
        }
        for isl in islands.iter_mut() {
            isl.pixels.sort_unstable();
            isl.pixels.dedup();
        }
        let mut owner = vec![u32::MAX; (w * h) as usize];
        let mut overlap: HashMap<(usize, usize), usize> = HashMap::new();
        for (id, isl) in islands.iter().enumerate() {
            for &p in &isl.pixels {
                let o = &mut owner[p as usize];
                if *o == u32::MAX {
                    *o = id as u32;
                } else if *o as usize != id {
                    *overlap.entry((*o as usize, id)).or_insert(0) += 1;
                }
            }
        }
        let mut ip: Vec<usize> = (0..islands.len()).collect();
        for (&(a, b), &c) in &overlap {
            if islands[a].material == islands[b].material && c * 2 >= islands[a].pixels.len().min(islands[b].pixels.len()) {
                let (ra, rb) = (find(&mut ip, a), find(&mut ip, b));
                ip[ra] = rb;
            }
        }
        for i in 0..islands.len() {
            let r = find(&mut ip, i);
            if r != i {
                let px = std::mem::take(&mut islands[i].pixels);
                let tr = std::mem::take(&mut islands[i].tris);
                islands[r].pixels.extend(px);
                islands[r].tris.extend(tr);
            }
        }
        islands.retain(|isl| !isl.tris.is_empty());
        for isl in islands.iter_mut() {
            isl.pixels.sort_unstable();
            isl.pixels.dedup();
        }
        let xc = (lo[0] + hi[0]) / 2.0;
        for isl in islands.iter_mut() {
            let n = isl.tris.len().max(1) as f32;
            for &(si, t) in &isl.tris {
                let (mut c, nn) = tri_info(model, si, t);
                c[0] = (c[0] - xc).abs();
                *isl.votes.entry(region(c, nn, lo, hi, forward)).or_insert(0.0) += tri_area(model, si, t);
                for a in 0..3 {
                    isl.centroid[a] += c[a] / n;
                    isl.normal[a] += nn[a] / n;
                }
            }
        }
        let total: usize = islands.iter().map(|i| i.pixels.len()).sum();
        let min_px = ((total as f32 * 0.004) as usize).max(24);
        const MAX_PARTS: usize = 24;
        loop {
            let alive: Vec<usize> = (0..islands.len()).filter(|&i| !islands[i].pixels.is_empty()).collect();
            if alive.len() <= 1 {
                break;
            }
            let smallest = *alive.iter().min_by_key(|&&i| (islands[i].pixels.len(), i)).unwrap();
            let s = &islands[smallest];
            if s.pixels.len() >= min_px && !s.tris.is_empty() && alive.len() <= MAX_PARTS {
                break;
            }
            let d = |j: usize| -> f32 {
                let o = &islands[j];
                let same = if o.material == s.material { 0.0 } else { 1e6 };
                same + (0..3).map(|a| (o.centroid[a] - s.centroid[a]).powi(2)).sum::<f32>()
            };
            let target = *alive.iter().filter(|&&j| j != smallest).min_by(|&&a, &&b| d(a).partial_cmp(&d(b)).unwrap_or(std::cmp::Ordering::Equal)).unwrap();
            let moved = std::mem::take(&mut islands[smallest].pixels);
            let mtris = std::mem::take(&mut islands[smallest].tris);
            let mvotes = std::mem::take(&mut islands[smallest].votes);
            let sc = islands[smallest].centroid;
            let o = &mut islands[target];
            let (a, b) = (o.tris.len() as f32, mtris.len() as f32);
            if a + b > 0.0 {
                for k in 0..3 {
                    o.centroid[k] = (o.centroid[k] * a + sc[k] * b) / (a + b);
                }
            }
            o.pixels.extend(moved);
            o.tris.extend(mtris);
            for (k, v) in mvotes {
                *o.votes.entry(k).or_insert(0.0) += v;
            }
        }
        let mut alive: Vec<usize> = (0..islands.len()).filter(|&i| !islands[i].pixels.is_empty()).collect();
        alive.sort_by_key(|&i| std::cmp::Reverse(islands[i].pixels.len()));
        let mut counts: HashMap<&'static str, u32> = HashMap::new();
        let mut parts = Vec::new();
        let mut label = vec![u16::MAX; (w * h) as usize];
        for (pi, &i) in alive.iter().enumerate() {
            let isl = &islands[i];
            let key = if mapped.to_ascii_lowercase().contains("eye") {
                "eyes"
            } else {
                keyword_key(&isl.material).unwrap_or_else(|| geometric_key(isl))
            };
            let n = counts.entry(key).or_insert(0);
            *n += 1;
            let mut mask = vec![false; (w * h) as usize];
            for &p in &isl.pixels {
                mask[p as usize] = true;
                if label[p as usize] == u16::MAX {
                    label[p as usize] = pi as u16;
                }
            }
            let (runs, outline, bb, count) = runs_and_outline(&mask, w, h);
            for &(si, t) in &isl.tris {
                tri_parts[mi][si][t] = pi as u16;
            }
            parts.push(Part { key: key.into(), n: *n, material: isl.material.clone(), pixels: count, bbox: bb, runs, outline });
        }
        for p in parts.iter_mut() {
            if counts.get(p.key.as_str()) == Some(&1) {
                p.n = 0;
            }
        }
        for (oi, ms) in models.iter().enumerate() {
            for (si, s) in ms.model.submeshes.iter().enumerate() {
                let Some(smp) = sampler_of(&ms.model, si).filter(|x| x.texture == mapped) else { continue };
                for t in 0..s.triangles() {
                    if tri_parts[oi][si][t] != u16::MAX {
                        continue;
                    }
                    let (mut cx, mut cy) = (0.0, 0.0);
                    for k in 0..3 {
                        let v = s.indices[t * 3 + k] as usize;
                        cx += s.uvs[v * 2] * w as f32 / 3.0;
                        cy += s.uvs[v * 2 + 1] * h as f32 / 3.0;
                    }
                    if cx.is_finite() && cy.is_finite() {
                        let p = wrap(cy.floor() as i64, h, smp.wrap_t) * w + wrap(cx.floor() as i64, w, smp.wrap_s);
                        tri_parts[oi][si][t] = label[p as usize];
                    }
                }
            }
        }
        out.push(TextureParts { texture: name.clone(), width: w, height: h, parts, model: Some(mi), alias_of });
    }
    Computed { textures: out, tri_parts }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdl0::{Material, Submesh};

    fn quad(sm: &mut Submesh, uv: [f32; 4], z: f32, y: f32) {
        let base = (sm.positions.len() / 3) as u32;
        let [u0, v0, u1, v1] = uv;
        for (u, v, x, yy) in [(u0, v0, -1.0, y + 1.0), (u1, v0, 1.0, y + 1.0), (u1, v1, 1.0, y), (u0, v1, -1.0, y)] {
            sm.positions.extend_from_slice(&[x, yy, z]);
            sm.normals.extend_from_slice(&[0.0, 0.0, 1.0]);
            sm.uvs.extend_from_slice(&[u, v]);
        }
        sm.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn model() -> Model {
        let mut sm = Submesh { material: "body_all_tx".into(), shape: "p0".into(), sampler: Some(0), ..Default::default() };
        quad(&mut sm, [0.0, 0.0, 0.5, 0.5], 0.0, 9.0);
        quad(&mut sm, [0.75, 0.75, 1.0, 1.0], 0.0, 0.0);
        quad(&mut sm, [0.1, 0.9, 0.11, 0.91], 0.0, 0.0);
        Model {
            name: "t".into(),
            materials: vec![Material { name: "body_all_tx".into(), samplers: vec![Sampler { texture: "body".into(), wrap_s: 1, wrap_t: 1, uv_set: Some(0), srt: None }] }],
            submeshes: vec![sm],
            skipped_shapes: 0,
        }
    }

    #[test]
    fn islands_masks_labels_and_tri_parts() {
        let src = vec![ModelSource { kind: "race", file: "x".into(), model: model() }];
        let c = compute(&src, &[("body".into(), 64, 64), ("body_eye.3".into(), 8, 8), ("other".into(), 8, 8)]);
        let t = &c.textures[0];
        assert_eq!(t.parts.len(), 2, "{:?}", t.parts);
        let p0 = &t.parts[0];
        assert!(p0.pixels >= 32 * 32 && p0.pixels <= 34 * 34, "{}", p0.pixels);
        assert_eq!(p0.key, "face");
        assert_eq!(t.parts[1].key, "feet");
        assert!(p0.bbox[0] == 0 && p0.bbox[1] == 0 && p0.bbox[2] >= 32 && p0.bbox[2] <= 33);
        let covered: u32 = p0.runs.chunks(2).map(|r| r[1]).sum();
        assert_eq!(covered, p0.pixels);
        assert!(p0.outline.chunks(4).all(|s| s[0] == s[2] || s[1] == s[3]));
        assert!(c.tri_parts[0][0].iter().all(|&p| p != u16::MAX));
        assert_eq!(&c.tri_parts[0][0][..2], &[0, 0]);
        assert!(c.textures[1].parts.is_empty() && c.textures[2].parts.is_empty());
    }

    #[test]
    fn wrap_modes() {
        assert_eq!(wrap(-1, 8, 1), 7);
        assert_eq!(wrap(9, 8, 1), 1);
        assert_eq!(wrap(8, 8, 2), 7);
        assert_eq!(wrap(-1, 8, 2), 0);
        assert_eq!(wrap(-5, 8, 0), 0);
        assert_eq!(wrap(12, 8, 0), 7);
    }

    #[test]
    fn mirrored_uvs_land_in_the_same_texels() {
        let mut m = model();
        m.materials[0].samplers[0].wrap_s = 2;
        let mut sm = Submesh { material: "body_all_tx".into(), shape: "p1".into(), sampler: Some(0), ..Default::default() };
        quad(&mut sm, [1.5, 0.0, 2.0, 0.5], 0.0, 9.0);
        m.submeshes = vec![sm];
        let c = compute(&[ModelSource { kind: "race", file: "x".into(), model: m }], &[("body".into(), 64, 64)]);
        let p = &c.textures[0].parts[0];
        assert_eq!(p.bbox[0], 0);
        assert!(p.bbox[2] <= 33);
    }
}
