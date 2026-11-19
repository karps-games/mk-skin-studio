use crate::brres::{be16, be32, cstr, group, offset};
use crate::mdl0::BoneInfo;

#[derive(Clone, Debug, PartialEq)]
pub enum Track {
    Const(f32),
    Keys(Vec<[f32; 3]>),
    Baked(Vec<f32>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub name: String,
    pub flags: u32,
    pub tracks: [Option<Track>; 9],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Anim {
    pub name: String,
    pub frames: u16,
    pub looping: bool,
    pub nodes: Vec<Node>,
}

fn f32at(b: &[u8], at: usize) -> Result<f32, String> {
    be32(b, at).map(f32::from_bits)
}

const OMIT: [u32; 9] = [0x8A, 0x9A, 0x9A, 0x126, 0x126, 0x126, 0x246, 0x246, 0x246];

fn keyed(b: &[u8], at: usize, fmt: u32) -> Result<Track, String> {
    let n = be16(b, at)? as usize;
    if n == 0 || n > 4096 {
        return Err(crate::brres::bad());
    }
    let mut keys = Vec::with_capacity(n);
    let (scale, add, base) = if fmt == 3 { (1.0, 0.0, at + 8) } else { (f32at(b, at + 8)?, f32at(b, at + 12)?, at + 16) };
    for i in 0..n {
        keys.push(match fmt {
            1 => {
                let x = be32(b, base + i * 4)?;
                let slope = (((x & 0xFFF) as i32) << 20 >> 20) as f32 / 32.0;
                [(x >> 24) as f32, ((x >> 12) & 0xFFF) as f32 * scale + add, slope]
            }
            2 => {
                let e = base + i * 6;
                [be16(b, e)? as i16 as f32 / 32.0, be16(b, e + 2)? as f32 * scale + add, be16(b, e + 4)? as i16 as f32 / 256.0]
            }
            _ => {
                let e = base + i * 12;
                [f32at(b, e)?, f32at(b, e + 4)?, f32at(b, e + 8)?]
            }
        });
    }
    Ok(Track::Keys(keys))
}

fn baked(b: &[u8], at: usize, fmt: u32, frames: usize) -> Result<Track, String> {
    let mut v = Vec::with_capacity(frames + 1);
    if fmt == 6 {
        for i in 0..=frames {
            v.push(f32at(b, at + i * 4)?);
        }
    } else {
        let (scale, add) = (f32at(b, at)?, f32at(b, at + 4)?);
        for i in 0..=frames {
            let raw = if fmt == 4 { *b.get(at + 8 + i).ok_or_else(crate::brres::bad)? as f32 } else { be16(b, at + 8 + i * 2)? as f32 };
            v.push(raw * scale + add);
        }
    }
    Ok(Track::Baked(v))
}

pub fn parse(b: &[u8], at: usize) -> Result<Anim, String> {
    if b.get(at..at + 4) != Some(b"CHR0") || be32(b, at + 8)? != 5 {
        return Err("E:anim.version".into());
    }
    let name = cstr(b, offset(at, be32(b, at + 0x18)?, b.len())?)?;
    let frames = be16(b, at + 0x20)?;
    let looping = be32(b, at + 0x24)? != 0;
    let mut nodes = Vec::new();
    for (node_name, n) in group(b, offset(at, be32(b, at + 0x10)?, b.len())?)? {
        let flags = be32(b, n + 4)?;
        let mut tracks: [Option<Track>; 9] = Default::default();
        let mut p = n + 8;
        for (i, slot) in tracks.iter_mut().enumerate() {
            if flags & OMIT[i] != 0 {
                continue;
            }
            let fmt = match i {
                0..=2 => (flags >> 25) & 3,
                3..=5 => (flags >> 27) & 7,
                _ => (flags >> 30) & 3,
            };
            *slot = Some(if flags & (1 << (13 + i)) != 0 || fmt == 0 {
                Track::Const(f32at(b, p)?)
            } else {
                let t = offset(n, be32(b, p)?, b.len())?;
                match fmt {
                    1..=3 => keyed(b, t, fmt)?,
                    4..=6 => baked(b, t, fmt, frames as usize)?,
                    _ => return Err(crate::brres::bad()),
                }
            });
            p += 4;
        }
        nodes.push(Node { name: node_name, flags, tracks });
    }
    Ok(Anim { name, frames, looping, nodes })
}

pub fn parse_all(b: &[u8]) -> Result<Vec<Anim>, String> {
    let root = be16(b, 12)? as usize;
    let mut out = Vec::new();
    for (folder, sub) in group(b, root + 8)? {
        if folder == "AnmChr(NW4R)" {
            for (_, at) in group(b, sub)? {
                out.push(parse(b, at)?);
            }
        }
    }
    Ok(out)
}

impl Track {
    pub fn at(&self, frame: f32) -> f32 {
        match self {
            Track::Const(v) => *v,
            Track::Baked(v) => {
                let f = frame.clamp(0.0, (v.len() - 1) as f32);
                let i = f.floor() as usize;
                let j = (i + 1).min(v.len() - 1);
                v[i] + (v[j] - v[i]) * (f - i as f32)
            }
            Track::Keys(k) => {
                if frame <= k[0][0] {
                    return k[0][1];
                }
                let last = k[k.len() - 1];
                if frame >= last[0] {
                    return last[1];
                }
                let i = k.partition_point(|x| x[0] <= frame) - 1;
                let ([f0, v0, s0], [f1, v1, s1]) = (k[i], k[i + 1]);
                let dt = f1 - f0;
                if dt <= 0.0 {
                    return v1;
                }
                let t = (frame - f0) / dt;
                let (t2, t3) = (t * t, t * t * t);
                v0 * (2.0 * t3 - 3.0 * t2 + 1.0) + v1 * (3.0 * t2 - 2.0 * t3) + dt * (s0 * (t3 - 2.0 * t2 + t) + s1 * (t3 - t2))
            }
        }
    }
}

pub fn mul(a: &[f32; 12], b: &[f32; 12]) -> [f32; 12] {
    let mut m = [0f32; 12];
    for r in 0..3 {
        for c in 0..4 {
            m[r * 4 + c] = a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c] + a[r * 4 + 2] * b[8 + c] + if c == 3 { a[r * 4 + 3] } else { 0.0 };
        }
    }
    m
}

pub fn srt(s: [f32; 3], r: [f32; 3], t: [f32; 3]) -> [f32; 12] {
    let (sx, cx) = r[0].to_radians().sin_cos();
    let (sy, cy) = r[1].to_radians().sin_cos();
    let (sz, cz) = r[2].to_radians().sin_cos();
    let rot = [
        [cy * cz, sx * sy * cz - cx * sz, cx * sy * cz + sx * sz],
        [cy * sz, sx * sy * sz + cx * cz, cx * sy * sz - sx * cz],
        [-sy, sx * cy, cx * cy],
    ];
    let mut m = [0f32; 12];
    for (row, rr) in rot.iter().enumerate() {
        for col in 0..3 {
            m[row * 4 + col] = rr[col] * s[col];
        }
        m[row * 4 + 3] = t[row];
    }
    m
}

type Srt = [[f32; 3]; 3];

fn world(bones: &[BoneInfo], local: &[Srt]) -> Vec<[f32; 12]> {
    let mut out: Vec<[f32; 12]> = vec![[0.0; 12]; bones.len()];
    let mut done = vec![false; bones.len()];
    fn solve(i: usize, bones: &[BoneInfo], local: &[Srt], out: &mut [[f32; 12]], done: &mut [bool], depth: usize) {
        if done[i] {
            return;
        }
        let [s, r, t] = local[i];
        out[i] = match bones[i].parent.filter(|&p| p != i && depth < 256) {
            Some(p) => {
                solve(p, bones, local, out, done, depth + 1);
                if bones[i].flags & 0x20 != 0 {
                    let ps = local[p][0];
                    let inv = srt(ps.map(|v| if v != 0.0 { 1.0 / v } else { 1.0 }), [0.0; 3], [0.0; 3]);
                    mul(&mul(&out[p], &srt([1.0; 3], [0.0; 3], t)), &mul(&inv, &srt(s, r, [0.0; 3])))
                } else {
                    mul(&out[p], &srt(s, r, t))
                }
            }
            None => srt(s, r, t),
        };
        done[i] = true;
    }
    for i in 0..bones.len() {
        solve(i, bones, local, &mut out, &mut done, 0);
    }
    out
}

pub fn bind_pose(bones: &[BoneInfo]) -> Vec<[f32; 12]> {
    let local: Vec<Srt> = bones.iter().map(|b| [b.scale, b.rotation, b.translation]).collect();
    world(bones, &local)
}

impl Anim {
    pub fn pose(&self, bones: &[BoneInfo], frame: f32) -> Vec<[f32; 12]> {
        let local: Vec<Srt> = bones
            .iter()
            .map(|b| {
                let Some(n) = self.nodes.iter().find(|n| n.name == b.name) else {
                    return [b.scale, b.rotation, b.translation];
                };
                let bind = [b.scale, b.rotation, b.translation];
                let model = [n.flags & 0x80 != 0, n.flags & 0x100 != 0, n.flags & 0x200 != 0];
                let mut v = [[1.0f32; 3], [0.0; 3], [0.0; 3]];
                for (k, item) in v.iter_mut().enumerate() {
                    for (axis, x) in item.iter_mut().enumerate() {
                        *x = match &n.tracks[k * 3 + axis] {
                            Some(t) => t.at(frame),
                            None if model[k] => bind[k][axis],
                            None => *x,
                        };
                    }
                }
                if n.flags & 0x10 != 0 && n.flags & 0x8 == 0 {
                    v[0] = [v[0][0]; 3];
                }
                v
            })
            .collect();
        world(bones, &local)
    }
}

pub fn skinning(bones: &[BoneInfo], pose: &[[f32; 12]]) -> Vec<[f32; 12]> {
    bones.iter().zip(pose).map(|(b, w)| mul(w, &b.inverse)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermite_keys_and_baked_tracks() {
        let k = Track::Keys(vec![[0.0, 0.0, 0.0], [10.0, 10.0, 0.0]]);
        assert_eq!(k.at(-5.0), 0.0);
        assert_eq!(k.at(10.0), 10.0);
        assert!((k.at(5.0) - 5.0).abs() < 1e-5);
        let lin = Track::Keys(vec![[0.0, 0.0, 1.0], [10.0, 10.0, 1.0]]);
        assert!((lin.at(2.5) - 2.5).abs() < 1e-4);
        let b = Track::Baked(vec![0.0, 2.0, 4.0]);
        assert!((b.at(1.5) - 3.0).abs() < 1e-6);
        assert_eq!(b.at(9.0), 4.0);
    }

    #[test]
    fn srt_composes_scale_rotation_translation() {
        let m = srt([2.0, 1.0, 1.0], [0.0, 0.0, 90.0], [5.0, 0.0, 0.0]);
        let p = [m[0] + m[3], m[4] + m[7], m[8] + m[11]];
        assert!((p[0] - 5.0).abs() < 1e-5 && (p[1] - 2.0).abs() < 1e-5 && p[2].abs() < 1e-5);
        let id = srt([1.0; 3], [0.0; 3], [0.0; 3]);
        assert_eq!(mul(&m, &id), m);
    }

    #[test]
    fn real_skeletons_and_animations() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(std::path::Path::new(&p))) else {
            return;
        };
        let (mut models, mut anims, mut worst) = (0, 0, 0f32);
        for ch in crate::game::scan(&dir).unwrap() {
            for rel in ch.files.iter().map(String::as_str).chain([crate::game::DRIVER_SZS]) {
                let b = crate::budget::driver_brres(&dir, rel, &ch.code).unwrap();
                let list = crate::mdl0::parse_all(&b).unwrap();
                for m in &list {
                    for (bone, w) in m.bones.iter().zip(bind_pose(&m.bones)) {
                        for k in 0..12 {
                            let d = (bone.model[k] - w[k]).abs() / bone.model[k].abs().max(1.0);
                            worst = worst.max(d);
                            assert!(d < 1e-3, "{rel} {} {} {k} {} {}", m.name, bone.name, bone.model[k], w[k]);
                        }
                    }
                    for (i, inf) in m.influences.iter().enumerate() {
                        let total: f32 = inf.iter().map(|x| x.1).sum();
                        assert!(inf.is_empty() || (total - 1.0).abs() < 1e-3, "{rel} {} mtx {i} {inf:?}", m.name);
                        assert!(inf.iter().all(|x| (x.0 as usize) < m.bones.len()));
                    }
                    models += 1;
                }
                for a in parse_all(&b).unwrap_or_else(|e| panic!("{rel} {e}")) {
                    let hits = |m: &crate::mdl0::Model| a.nodes.iter().filter(|n| m.bones.iter().any(|x| x.name == n.name)).count();
                    let model = list.iter().max_by_key(|m| hits(m)).unwrap();
                    assert!(hits(model) > 0, "{rel} {}", a.name);
                    for f in 0..=a.frames {
                        assert!(a.pose(&model.bones, f as f32).iter().flatten().all(|v| v.is_finite()), "{rel} {}", a.name);
                    }
                    anims += 1;
                }
            }
        }
        println!("{models} models, {anims} animations, worst bind error {worst:e}");
        assert!(anims > 1000);
    }
}
