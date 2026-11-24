use crate::brres::Archive;
use crate::chr0::{mul, srt};
use crate::mdl0::Model;
use crate::mdl0_layout::{Layout, Mesh};
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

pub const SHAPES: [&str; 5] = ["sphere", "box", "cylinder", "cone", "ring"];
pub const PREFIX: &str = "mkss_acc";
pub const LIMIT: usize = 16;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessory {
    pub shape: String,
    pub bone: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    pub color: [u8; 3],
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeView {
    pub name: String,
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
}

pub type Pose = HashMap<String, [f32; 12]>;

fn vertex(m: &mut Mesh, p: [f32; 3], n: [f32; 3], uv: [f32; 2]) -> u16 {
    m.positions.push(p);
    m.normals.push(n);
    m.colors.push([255; 4]);
    m.uvs.push(uv);
    (m.positions.len() - 1) as u16
}

fn grid(m: &mut Mesh, cols: usize, rows: usize, at: impl Fn(f32, f32) -> ([f32; 3], [f32; 3])) {
    let base = m.positions.len() as u16;
    for r in 0..=rows {
        for c in 0..=cols {
            let (u, v) = (c as f32 / cols as f32, r as f32 / rows as f32);
            let (p, n) = at(u, v);
            vertex(m, p, n, [u, v]);
        }
    }
    let w = cols as u16 + 1;
    for r in 0..rows as u16 {
        for c in 0..cols as u16 {
            let (a, b) = (base + r * w + c, base + (r + 1) * w + c);
            m.triangles.push([a, b, a + 1]);
            m.triangles.push([a + 1, b, b + 1]);
        }
    }
}

fn disc(m: &mut Mesh, y: f32, r: f32, up: bool, segments: usize) {
    let ny = if up { 1.0 } else { -1.0 };
    let center = vertex(m, [0.0, y, 0.0], [0.0, ny, 0.0], [0.5, 0.5]);
    let first = m.positions.len() as u16;
    for i in 0..segments {
        let a = i as f32 / segments as f32 * TAU;
        vertex(m, [r * a.cos(), y, r * a.sin()], [0.0, ny, 0.0], [0.5 + 0.5 * a.cos(), 0.5 + 0.5 * a.sin()]);
    }
    for i in 0..segments as u16 {
        let (a, b) = (first + i, first + (i + 1) % segments as u16);
        m.triangles.push(if up { [center, b, a] } else { [center, a, b] });
    }
}

pub fn shape(kind: &str) -> Option<Mesh> {
    let mut m = Mesh::default();
    match kind {
        "sphere" => grid(&mut m, 16, 10, |u, v| {
            let (a, b) = (u * TAU, v * PI);
            let n = [b.sin() * a.cos(), b.cos(), b.sin() * a.sin()];
            (n.map(|x| x * 0.5), n)
        }),
        "box" => {
            for axis in 0..3 {
                for sign in [-1.0f32, 1.0] {
                    let mut n = [0.0; 3];
                    n[axis] = sign;
                    let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
                    let (ua, va) = if sign > 0.0 { (va, ua) } else { (ua, va) };
                    grid(&mut m, 1, 1, |u, v| {
                        let mut p = [0.0; 3];
                        p[axis] = sign * 0.5;
                        p[ua] = u - 0.5;
                        p[va] = v - 0.5;
                        (p, n)
                    });
                }
            }
        }
        "cylinder" => {
            grid(&mut m, 16, 1, |u, v| {
                let a = u * TAU;
                ([0.5 * a.cos(), 0.5 - v, 0.5 * a.sin()], [a.cos(), 0.0, a.sin()])
            });
            disc(&mut m, 0.5, 0.5, true, 16);
            disc(&mut m, -0.5, 0.5, false, 16);
        }
        "cone" => {
            let k = 1.0 / 5f32.sqrt();
            grid(&mut m, 16, 1, |u, v| {
                let a = u * TAU;
                ([0.5 * v * a.cos(), 0.5 - v, 0.5 * v * a.sin()], [2.0 * k * a.cos(), k, 2.0 * k * a.sin()])
            });
            disc(&mut m, -0.5, 0.5, false, 16);
        }
        "ring" => grid(&mut m, 20, 8, |u, v| {
            let (a, b) = (u * TAU, v * TAU);
            let n = [b.cos() * a.cos(), b.sin(), b.cos() * a.sin()];
            let r = 0.375 + 0.125 * b.cos();
            ([r * a.cos(), 0.125 * b.sin(), r * a.sin()], n)
        }),
        _ => return None,
    }
    Some(orient(m))
}

fn orient(mut m: Mesh) -> Mesh {
    let (p, n) = (&m.positions, &m.normals);
    m.triangles = m
        .triangles
        .iter()
        .filter_map(|&t| {
            let [a, b, c] = t.map(|i| p[i as usize]);
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cross = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            if cross.iter().map(|x| x * x).sum::<f32>() < 1e-12 {
                return None;
            }
            let dot: f32 = t.iter().map(|&i| (0..3).map(|k| cross[k] * n[i as usize][k]).sum::<f32>()).sum();
            Some(if dot < 0.0 { [t[0], t[2], t[1]] } else { t })
        })
        .collect();
    m
}

pub fn shapes() -> Vec<ShapeView> {
    SHAPES
        .iter()
        .filter_map(|&name| {
            let m = shape(name)?;
            Some(ShapeView {
                name: name.into(),
                positions: m.positions.iter().flatten().copied().collect(),
                normals: m.normals.iter().flatten().copied().collect(),
                uvs: m.uvs.iter().flatten().copied().collect(),
                indices: m.triangles.iter().flatten().map(|&i| i as u32).collect(),
            })
        })
        .collect()
}

pub fn local(a: &Accessory) -> [f32; 12] {
    srt(a.scale, a.rotation, a.position)
}

fn apply(m: &[f32; 12], p: [f32; 3], w: f32) -> [f32; 3] {
    std::array::from_fn(|r| m[r * 4] * p[0] + m[r * 4 + 1] * p[1] + m[r * 4 + 2] * p[2] + m[r * 4 + 3] * w)
}

fn placed(unit: &Mesh, m: &[f32; 12]) -> Mesh {
    let mut out = unit.clone();
    out.positions = unit.positions.iter().map(|&p| apply(m, p, 1.0)).collect();
    out.normals = unit
        .normals
        .iter()
        .map(|&n| {
            let v = apply(m, n, 0.0);
            let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
            v.map(|x| x / len)
        })
        .collect();
    out
}

fn bounds(points: impl Iterator<Item = [f32; 3]>) -> ([f32; 3], [f32; 3]) {
    points.fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| (std::array::from_fn(|k| lo[k].min(p[k])), std::array::from_fn(|k| hi[k].max(p[k]))))
}

pub fn base_bone(name: &str) -> &str {
    name.strip_prefix("pcd_").unwrap_or(name)
}

fn color_texture(c: [u8; 3]) -> Result<Vec<u8>, String> {
    let pixels = [c[0], c[1], c[2], 255].repeat(64);
    crate::texture::encode(&crate::texture::Rgba8 { width: 8, height: 8, pixels }, 14, 1)
}

fn body(l: &Layout) -> Option<(String, String)> {
    l.shapes().into_iter().filter_map(|(s, n, m)| Some((n, s, m?))).max_by_key(|x| x.0).map(|(_, s, m)| (s, m))
}

pub fn decorate(brres: &[u8], list: &[Accessory], pose: Option<&Pose>) -> Result<Vec<u8>, String> {
    let mut a = Archive::parse(brres)?;
    for (i, acc) in list.iter().enumerate() {
        a.add_texture(&format!("{PREFIX}{i}"), 8, 8, 14, 1, &color_texture(acc.color)?)?;
    }
    for m in a.members.iter_mut().filter(|m| m.bytes.starts_with(b"MDL0")) {
        if !matches!(m.name.as_str(), "model" | "model_cpu" | "model_lod") {
            continue;
        }
        let mut l = Layout::parse(&m.bytes, &m.names)?;
        let posed = l.items(1).len() < 2;
        let (template, material) = body(&l).ok_or("E:model.unsupported")?;
        let mut changed = false;
        for (i, acc) in list.iter().enumerate() {
            let unit = shape(&acc.shape).ok_or("E:model.unsupported")?;
            let name = format!("{PREFIX}{i}");
            let local = local(acc);
            let base = base_bone(&acc.bone);
            let mut placements: Vec<(Mesh, [f32; 12], Option<u32>)> = Vec::new();
            if posed {
                let (Some(world), Some(owner)) = (pose.and_then(|p| p.get(base)), l.rigid_shapes().into_iter().find(|s| s.0 == template).map(|s| s.2)) else { continue };
                let (_, inv) = l.bone(&owner).ok_or("E:model.unsupported")?;
                let to_model = mul(world, &local);
                placements.push((placed(&unit, &mul(&inv, &to_model)), to_model, None));
            } else {
                for bone in [base.to_string(), format!("pcd_{base}")] {
                    let (Some(mtx), Some((model, _))) = (l.bone_matrix(&bone), l.bone(&bone)) else { continue };
                    placements.push((placed(&unit, &local), mul(&model, &local), Some(mtx)));
                }
            }
            if placements.is_empty() {
                continue;
            }
            let mat = l.add_material(&material, &name, &name)?;
            for (k, (mesh, to_model, mtx)) in placements.into_iter().enumerate() {
                let shape_name = if k == 0 { name.clone() } else { format!("{name}_{k}") };
                l.add_rigid_mesh(&template, &shape_name, &mesh, Some(mat), mtx)?;
                let (lo, hi) = bounds(unit.positions.iter().map(|&p| apply(&to_model, p, 1.0)));
                l.extend_bounds(lo, hi);
            }
            changed = true;
        }
        if changed {
            let (bytes, names) = l.write();
            m.bytes = bytes;
            m.names = names;
        }
    }
    Ok(a.write())
}

fn posed_points(m: &Model, skin: &[[f32; 12]]) -> Vec<[f32; 3]> {
    let mut out = Vec::new();
    for s in &m.submeshes {
        for (i, p) in s.positions.chunks(3).enumerate() {
            let p = [p[0], p[1], p[2]];
            let inf = s.matrices.get(i).and_then(|&x| usize::try_from(x).ok()).and_then(|x| m.influences.get(x));
            match inf.filter(|x| !x.is_empty()) {
                None => out.push(p),
                Some(inf) => {
                    let mut q = [0f32; 3];
                    for &(b, w) in inf {
                        let v = apply(&skin[b as usize], p, 1.0);
                        for k in 0..3 {
                            q[k] += w * v[k];
                        }
                    }
                    out.push(q);
                }
            }
        }
    }
    out
}

fn median_gap(sample: &[[f32; 3]], cloud: &[[f32; 3]]) -> f32 {
    let mut d: Vec<f32> = sample
        .iter()
        .map(|p| cloud.iter().map(|q| (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).fold(f32::MAX, f32::min))
        .collect();
    d.sort_by(f32::total_cmp);
    d.get(d.len() / 2).copied().unwrap_or(f32::MAX).sqrt()
}

pub fn driving_pose(full: &[u8], cpu: &[u8]) -> Result<(Pose, f32), String> {
    let model = crate::mdl0::parse_all(full)?.into_iter().find(|m| m.name == "model").ok_or("E:model.unsupported")?;
    let target = crate::mdl0::parse_all(cpu)?.into_iter().find(|m| m.name == "model_cpu").ok_or("E:model.unsupported")?;
    let all: Vec<[f32; 3]> = target.submeshes.iter().flat_map(|s| s.positions.chunks(3).map(|p| [p[0], p[1], p[2]])).collect();
    let step = (all.len() / 64).max(1);
    let sample: Vec<[f32; 3]> = all.iter().step_by(step).copied().collect();
    let mut best = (crate::chr0::bind_pose(&model.bones), f32::MAX);
    best.1 = median_gap(&sample, &posed_points(&model, &crate::chr0::skinning(&model.bones, &best.0)));
    for anim in crate::chr0::parse_all(full)? {
        for f in 0..=anim.frames {
            let world = anim.pose(&model.bones, f as f32);
            let gap = median_gap(&sample, &posed_points(&model, &crate::chr0::skinning(&model.bones, &world)));
            if gap < best.1 {
                best = (world, gap);
            }
        }
    }
    Ok((model.bones.iter().map(|b| b.name.clone()).zip(best.0).collect(), best.1))
}

pub fn replace_member(raw: &[u8], member: &str, data: &[u8]) -> Result<Vec<u8>, String> {
    let u = crate::u8arc::parse(raw)?;
    let target = u.find(member).ok_or("E:file.format")?.node;
    let payloads: Vec<Vec<u8>> = u
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| if n.is_dir { Vec::new() } else if i == target { data.to_vec() } else { raw[n.a as usize..(n.a + n.b) as usize].to_vec() })
        .collect();
    Ok(crate::u8arc::write(&u.nodes, &payloads))
}

pub fn full_file(rel: &str) -> String {
    rel.replace("_2.szs", ".szs").replace("_4.szs", ".szs")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(shape: &str, bone: &str) -> Accessory {
        Accessory { shape: shape.into(), bone: bone.into(), position: [1.0, 2.0, 3.0], rotation: [10.0, 20.0, 30.0], scale: [12.0, 8.0, 10.0], color: [200, 40, 40] }
    }

    #[test]
    fn shapes_are_small_closed_meshes() {
        for name in SHAPES {
            let m = shape(name).unwrap();
            let n = m.positions.len();
            assert!(n > 0 && n <= 256, "{name} {n}");
            assert_eq!(m.normals.len(), n);
            assert_eq!(m.uvs.len(), n);
            assert!(m.triangles.iter().flatten().all(|&i| (i as usize) < n), "{name}");
            assert!(m.positions.iter().flatten().all(|v| v.abs() <= 0.5 + 1e-5), "{name}");
            assert!(m.normals.iter().all(|v| ((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() - 1.0).abs() < 1e-3), "{name}");
            for t in &m.triangles {
                let [a, b, c] = t.map(|i| m.positions[i as usize]);
                let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let cross = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let nrm = m.normals[t[0] as usize];
                let area = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
                assert!(area > 0.0, "{name} degenerate");
                assert!(cross[0] * nrm[0] + cross[1] * nrm[1] + cross[2] * nrm[2] > 0.0, "{name} faces outward");
            }
        }
        assert!(shape("pyramid").is_none());
        assert_eq!(shapes().len(), SHAPES.len());
    }

    #[test]
    fn accessories_round_trip_as_json() {
        let a = acc("cone", "face_1");
        let json = serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"position\""));
        assert_eq!(serde_json::from_str::<Accessory>(&json).unwrap(), a);
        assert_eq!(base_bone("pcd_face_1"), "face_1");
        assert_eq!(full_file("Race/Kart/sa_kart-mr_4.szs"), "Race/Kart/sa_kart-mr.szs");
    }

    fn near(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
    }

    #[test]
    fn real_accessories_in_every_driver_model() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(std::path::Path::new(&p))) else {
            return;
        };
        let list = [acc("cone", "face_1"), acc("sphere", "wrist_r1"), acc("ring", "spin")];
        let mut checked = 0;
        for ch in crate::game::scan(&dir).unwrap() {
            let full = ch.files.iter().find(|f| full_file(f) == **f).unwrap().clone();
            let four = full.replace(".szs", "_4.szs");
            let read = |rel: &str| crate::budget::driver_brres(&dir, rel, &ch.code).unwrap();
            let (pose, gap) = driving_pose(&read(&full), &read(&four)).unwrap();
            let height = crate::mdl0::parse_all(&read(&full)).unwrap().into_iter().find(|m| m.name == "model").unwrap().submeshes.iter().flat_map(|s| s.positions.chunks(3).map(|p| p[1])).fold(f32::MIN, f32::max);
            assert!(gap < height * 0.01, "{} driving pose gap {gap} of {height}", ch.code);
            for rel in [full.clone(), full.replace(".szs", "_2.szs"), four.clone(), crate::game::DRIVER_SZS.to_string()] {
                let before_bytes = read(&rel);
                let out = decorate(&before_bytes, &list, Some(&pose)).unwrap_or_else(|e| panic!("{} {rel} {e}", ch.code));
                let before = crate::mdl0::parse_all(&before_bytes).unwrap();
                let after = crate::mdl0::parse_all(&out).unwrap_or_else(|e| panic!("{} {rel} {e}", ch.code));
                assert_eq!(before.len(), after.len());
                for (x, y) in before.iter().zip(&after) {
                    let rest: Vec<String> = y.submeshes.iter().filter(|s| !s.shape.starts_with(PREFIX)).map(|s| format!("{s:?}")).collect();
                    let orig: Vec<String> = x.submeshes.iter().map(|s| format!("{s:?}")).collect();
                    assert_eq!(rest, orig, "{} {rel} {}", ch.code, x.name);
                    let added: Vec<&crate::mdl0::Submesh> = y.submeshes.iter().filter(|s| s.shape.starts_with(PREFIX)).collect();
                    match x.name.as_str() {
                        "model" | "model_cpu" | "model_lod" => {}
                        _ => {
                            assert!(added.is_empty());
                            continue;
                        }
                    }
                    for (i, a) in list.iter().enumerate() {
                        let name = format!("{PREFIX}{i}");
                        let mine: Vec<&&crate::mdl0::Submesh> = added.iter().filter(|s| s.shape == name || s.shape.starts_with(&format!("{name}_"))).collect();
                        let skeletal = x.bones.len() > 1;
                        let has_bone = x.bones.iter().any(|b| base_bone(&b.name) == a.bone);
                        if (skeletal && !has_bone) || (!skeletal && !pose.contains_key(&a.bone)) {
                            assert!(mine.is_empty());
                            continue;
                        }
                        assert!(!mine.is_empty(), "{} {rel} {} {name}", ch.code, x.name);
                        let unit = shape(&a.shape).unwrap();
                        for s in mine {
                            assert_eq!(s.material, name);
                            assert_eq!(s.texture(y).map(|t| t.texture.as_str()), Some(name.as_str()));
                            assert!(s.positions.len() / 3 <= unit.positions.len() && s.indices.len() / 3 == unit.triangles.len());
                            let world = if skeletal {
                                let bone = y.bones.iter().find(|b| base_bone(&b.name) == a.bone && s.matrices.iter().all(|&m| y.influences[m as usize] == vec![(y.bones.iter().position(|q| q.name == b.name).unwrap() as u32, 1.0)])).unwrap_or_else(|| panic!("{} {rel} {name} bone", ch.code));
                                bone.model
                            } else {
                                pose[&a.bone]
                            };
                            let m = mul(&world, &local(a));
                            let wanted: Vec<[f32; 3]> = unit.positions.iter().map(|&q| apply(&m, q, 1.0)).collect();
                            for p in s.positions.chunks(3) {
                                let p = [p[0], p[1], p[2]];
                                assert!(wanted.iter().any(|&w| near(p, w, height * 0.002 + 0.05)), "{} {rel} {} {name} {p:?}", ch.code, x.name);
                            }
                        }
                    }
                }
                let packed = replace_member(&crate::skin::read_archive(&dir, &rel).unwrap(), &member_of(&dir, &rel, &ch.code), &out).unwrap();
                assert!(crate::u8arc::parse(&packed).is_ok());
                checked += 1;
            }
        }
        assert!(checked >= 96, "{checked}");
    }

    fn member_of(dir: &std::path::Path, rel: &str, code: &str) -> String {
        if rel == crate::game::DRIVER_SZS {
            crate::game::menu_member(&crate::u8arc::parse(&crate::skin::read_archive(dir, rel).unwrap()).unwrap(), code).unwrap()
        } else {
            crate::skin::DRIVER_MEMBER.to_string()
        }
    }
}
