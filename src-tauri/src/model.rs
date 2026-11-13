use crate::parts::{self, ModelSource};
use crate::{game, mdl0, skin, texmap, u8arc};
use base64::Engine;
use std::collections::HashSet;
use std::path::Path;

fn b64<T: Copy>(v: &[T], f: impl Fn(T) -> Vec<u8>) -> String {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for x in v {
        bytes.extend(f(*x));
    }
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
fn f32s(v: &[f32]) -> String {
    b64(v, |x| x.to_le_bytes().to_vec())
}
fn u32s(v: &[u32]) -> String {
    b64(v, |x| x.to_le_bytes().to_vec())
}
fn u16s(v: &[u16]) -> String {
    b64(v, |x| x.to_le_bytes().to_vec())
}
fn i32s(v: &[i32]) -> String {
    b64(v, |x| x.to_le_bytes().to_vec())
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubView {
    pub material: String,
    pub shape: String,
    pub texture: Option<String>,
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub xlu: bool,
    pub vertices: usize,
    pub triangles: usize,
    pub positions: String,
    pub normals: String,
    pub uvs: String,
    pub indices: String,
    pub parts: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshView {
    pub kind: String,
    pub file: String,
    pub name: String,
    pub vehicle: Option<String>,
    pub detail: String,
    pub bbox_min: [f32; 3],
    pub bbox_max: [f32; 3],
    pub submeshes: Vec<SubView>,
}

pub fn vehicle_of(kind: &str, file: &str) -> Option<&'static str> {
    if kind != "race" {
        return None;
    }
    let stem = file.rsplit(['/', '\\']).next().unwrap_or(file);
    let vehicle = stem.split('-').next().unwrap_or(stem);
    Some(if vehicle.contains("bike") { "bike" } else { "kart" })
}

pub fn detail_of(name: &str) -> &'static str {
    if name.to_ascii_lowercase().contains("lod") {
        "low"
    } else {
        "full"
    }
}

pub fn has_bike_outfit(sources: &[ModelSource]) -> bool {
    sources.iter().any(|s| vehicle_of(s.kind, &s.file) == Some("bike") && detail_of(&s.model.name) == "full")
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartView {
    #[serde(flatten)]
    pub part: parts::Part,
    pub runs: String,
    pub outline: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TexturePartsView {
    pub texture: String,
    pub width: u32,
    pub height: u32,
    pub model: Option<usize>,
    pub alias_of: Option<String>,
    pub parts: Vec<PartView>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactView {
    #[serde(flatten)]
    pub fact: texmap::TexFacts,
    pub used: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighView {
    pub texture: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkView {
    pub low: String,
    pub width: u32,
    pub height: u32,
    pub highs: Vec<HighView>,
    pub samples: String,
    pub mapped: u32,
    pub dist_rel: f32,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EyeView {
    pub frames: Vec<String>,
    pub default_frame: String,
    pub blink: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBundle {
    pub character: String,
    pub models: Vec<MeshView>,
    pub textures: Vec<TexturePartsView>,
    pub unreferenced: Vec<String>,
    pub facts: Vec<FactView>,
    pub links: Vec<LinkView>,
    pub shares: Vec<LinkView>,
    pub eyes: Vec<EyeView>,
}

fn textures_of(m: &mdl0::Model) -> HashSet<String> {
    m.submeshes.iter().filter_map(|s| s.texture(m).map(|t| t.texture.clone())).collect()
}

pub fn load_sources(root: &Path, ch: &game::Character) -> Result<Vec<ModelSource>, String> {
    let mut files: Vec<&String> = ch.files.iter().filter(|f| !f.ends_with("_2.szs") && !f.ends_with("_4.szs")).collect();
    files.sort_by_key(|f| (!f.contains("kart"), f.to_string()));
    let mut out = Vec::new();
    let mut covered: HashSet<String> = HashSet::new();
    let mut take = |kind: &'static str, rel: &str, b: &[u8], out: &mut Vec<ModelSource>| -> Result<(), String> {
        for model in mdl0::parse_all(b).map_err(|e| if e.contains("|file=") { e } else { format!("{e}|file={rel}") })? {
            let texs = textures_of(&model);
            if texs.iter().any(|t| !covered.contains(t)) {
                covered.extend(texs);
                out.push(ModelSource { kind, file: rel.to_string(), model });
            }
        }
        Ok(())
    };
    for rel in files {
        let raw = skin::read_archive(root, rel)?;
        let arc = u8arc::parse(&raw).map_err(|_| format!("E:game.badFile|file={rel}"))?;
        let Some(f) = arc.find(skin::DRIVER_MEMBER) else { continue };
        take("race", rel, &raw[f.offset..f.offset + f.size], &mut out)?;
    }
    let raw = skin::read_archive(root, game::DRIVER_SZS)?;
    let arc = u8arc::parse(&raw).map_err(|_| format!("E:game.badFile|file={}", game::DRIVER_SZS))?;
    if let Some(m) = game::menu_member(&arc, &ch.code) {
        if let Some(f) = arc.find(&m) {
            take("menu", game::DRIVER_SZS, &raw[f.offset..f.offset + f.size], &mut out)?;
        }
    }
    if out.is_empty() {
        return Err("E:model.none".into());
    }
    Ok(out)
}

pub fn eye_groups(map: &texmap::CharMap) -> Vec<EyeView> {
    let mut stems: Vec<String> = Vec::new();
    for f in map.facts.iter().filter(|f| f.role == "eye") {
        if let Some(b) = texmap::frame_base(&f.texture) {
            if !stems.contains(&b) {
                stems.push(b);
            }
        }
    }
    let mut out = Vec::new();
    for base in stems {
        let stem = base.trim_end_matches(".0").to_string();
        let mut frames: Vec<(u32, String)> = map
            .facts
            .iter()
            .filter_map(|f| f.texture.strip_prefix(&format!("{stem}.")).and_then(|n| n.parse::<u32>().ok()).map(|n| (n, f.texture.clone())))
            .collect();
        frames.sort();
        if frames.len() < 2 || frames[0].0 != 0 {
            continue;
        }
        let names: Vec<String> = frames.into_iter().map(|f| f.1).collect();
        let wink = map
            .pat0
            .iter()
            .filter(|(lod, t)| *lod == texmap::Lod::Race1 && t.anim == "wink" && t.keys.iter().any(|k| names.contains(&k.1)))
            .map(|(_, t)| t.keys.iter().map(|k| k.1.clone()).collect::<Vec<_>>())
            .next()
            .unwrap_or_else(|| names.clone());
        out.push(EyeView { default_frame: names[0].clone(), frames: names, blink: wink });
    }
    out
}

pub fn bundle(code: &str, sources: &[ModelSource], textures: &[(String, u32, u32)]) -> ModelBundle {
    bundle_with(code, sources, textures, None)
}

pub fn bundle_with(code: &str, sources: &[ModelSource], textures: &[(String, u32, u32)], map: Option<&texmap::CharMap>) -> ModelBundle {
    let computed = parts::compute(sources, textures);
    let models = sources
        .iter()
        .enumerate()
        .map(|(mi, src)| {
            let m = &src.model;
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for s in &m.submeshes {
                for p in s.positions.chunks_exact(3) {
                    for a in 0..3 {
                        lo[a] = lo[a].min(p[a]);
                        hi[a] = hi[a].max(p[a]);
                    }
                }
            }
            MeshView {
                kind: src.kind.into(),
                file: src.file.clone(),
                name: m.name.clone(),
                vehicle: vehicle_of(src.kind, &src.file).map(String::from),
                detail: detail_of(&m.name).into(),
                bbox_min: lo,
                bbox_max: hi,
                submeshes: m
                    .submeshes
                    .iter()
                    .enumerate()
                    .map(|(si, s)| {
                        let smp = s.texture(m);
                        SubView {
                            material: s.material.clone(),
                            shape: s.shape.clone(),
                            texture: smp.map(|x| x.texture.clone()),
                            wrap_s: smp.map_or(1, |x| x.wrap_s),
                            wrap_t: smp.map_or(1, |x| x.wrap_t),
                            xlu: s.xlu,
                            vertices: s.positions.len() / 3,
                            triangles: s.triangles(),
                            positions: f32s(&s.positions),
                            normals: f32s(&s.normals),
                            uvs: f32s(&s.uvs),
                            indices: u32s(&s.indices),
                            parts: u16s(&computed.tri_parts[mi][si]),
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    let unreferenced = computed.textures.iter().filter(|t| t.model.is_none()).map(|t| t.texture.clone()).collect();
    let textures = computed
        .textures
        .into_iter()
        .map(|t| TexturePartsView {
            texture: t.texture,
            width: t.width,
            height: t.height,
            model: t.model,
            alias_of: t.alias_of,
            parts: t.parts.into_iter().map(|p| PartView { runs: u32s(&p.runs), outline: u16s(&p.outline), part: p }).collect(),
        })
        .collect();
    let view = |l: &texmap::LodLink| LinkView {
        low: l.low.clone(),
        width: l.lw,
        height: l.lh,
        highs: l.highs.iter().map(|h| HighView { texture: h.0.clone(), width: h.1, height: h.2 }).collect(),
        samples: i32s(&l.samples),
        mapped: l.mapped,
        dist_rel: if l.dist_rel.is_finite() { l.dist_rel } else { 0.0 },
    };
    let (facts, links, shares, eyes) = match map {
        None => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        Some(m) => (
            m.facts.iter().map(|f| FactView { used: u32s(&texmap::runs(&f.used)), fact: f.clone() }).collect(),
            m.links.iter().map(view).collect(),
            m.shares.iter().map(view).collect(),
            eye_groups(m),
        ),
    };
    ModelBundle { character: code.into(), models, textures, unreferenced, facts, links, shares, eyes }
}

#[cfg(test)]
mod outfit_tests {
    use super::*;

    #[test]
    fn vehicle_and_detail_from_file_and_model_names() {
        assert_eq!(vehicle_of("race", "Race/Kart/ma_kart-pc.szs"), Some("kart"));
        assert_eq!(vehicle_of("race", "Race/Kart/mdf_kart_blue-pc_2.szs"), Some("kart"));
        assert_eq!(vehicle_of("race", "Race/Kart/sa_bike-bmr.szs"), Some("bike"));
        assert_eq!(vehicle_of("race", r"Race\Kart\mdf_bike_red-rs_4.szs"), Some("bike"));
        assert_eq!(vehicle_of("menu", "Scene/Model/Driver.szs"), None);
        assert_eq!(detail_of("model"), "full");
        assert_eq!(detail_of("model_cpu"), "full");
        assert_eq!(detail_of("model_lod"), "low");
    }
}
