use crate::brres::{be32, Archive};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub triangles: u32,
    pub vertices: u32,
    pub model_bytes: u32,
    pub texture_bytes: u32,
    pub file_bytes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub enum Version {
    Race1,
    Race2,
    Race4,
    Menu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum Level {
    Ok,
    Warn,
    Over,
}

pub const MARGIN: f32 = 23.0;

pub fn version_of(file: &str) -> Version {
    let stem = file.rsplit('/').next().unwrap_or(file).trim_end_matches(".szs");
    if stem.ends_with("_4") {
        Version::Race4
    } else if stem.ends_with("_2") {
        Version::Race2
    } else if file.ends_with(crate::game::DRIVER_SZS) {
        Version::Menu
    } else {
        Version::Race1
    }
}

pub fn measure(brres: &[u8]) -> Result<Stats, String> {
    let a = Archive::parse(brres)?;
    let mut s = Stats { file_bytes: brres.len() as u32, ..Stats::default() };
    for m in &a.members {
        let b = &m.bytes;
        if b.starts_with(b"MDL0") {
            s.model_bytes += b.len() as u32;
            s.vertices += be32(b, 0x5C)?;
            s.triangles += be32(b, 0x60)?;
        } else if b.starts_with(b"TEX0") {
            s.texture_bytes += b.len() as u32;
        }
    }
    Ok(s)
}

fn fields(s: &Stats) -> [u32; 5] {
    [s.triangles, s.vertices, s.model_bytes, s.texture_bytes, s.file_bytes]
}

pub fn ratio(s: &Stats, limit: &Stats) -> f32 {
    fields(s).iter().zip(fields(limit)).map(|(&v, l)| v as f32 / l.max(1) as f32).fold(0.0, f32::max)
}

pub fn level(s: &Stats, limit: &Stats) -> Level {
    let r = ratio(s, limit);
    if r <= 1.0 {
        Level::Ok
    } else if r <= 1.0 + MARGIN {
        Level::Warn
    } else {
        Level::Over
    }
}

pub fn widen(limit: &mut Stats, s: &Stats) {
    limit.triangles = limit.triangles.max(s.triangles);
    limit.vertices = limit.vertices.max(s.vertices);
    limit.model_bytes = limit.model_bytes.max(s.model_bytes);
    limit.texture_bytes = limit.texture_bytes.max(s.texture_bytes);
    limit.file_bytes = limit.file_bytes.max(s.file_bytes);
}

pub fn driver_brres(root: &Path, rel: &str, code: &str) -> Result<Vec<u8>, String> {
    let raw = crate::skin::read_archive(root, rel)?;
    let arc = crate::u8arc::parse(&raw)?;
    let member = if version_of(rel) == Version::Menu {
        [format!("{code}.brres"), format!("{code}_menu.brres")].into_iter().find(|n| arc.find(n).is_some()).ok_or("E:file.format")?
    } else {
        crate::skin::DRIVER_MEMBER.to_string()
    };
    let e = arc.find(&member).ok_or("E:file.format")?;
    Ok(raw[e.offset..e.offset + e.size].to_vec())
}

pub fn limits(root: &Path, characters: &[crate::game::Character]) -> Result<BTreeMap<Version, Stats>, String> {
    let mut out: BTreeMap<Version, Stats> = BTreeMap::new();
    for ch in characters {
        for rel in ch.files.iter().map(String::as_str).chain([crate::game::DRIVER_SZS]) {
            let s = measure(&driver_brres(root, rel, &ch.code)?)?;
            widen(out.entry(version_of(rel)).or_default(), &s);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_follow_file_names() {
        assert_eq!(version_of("Race/Kart/ma_kart-mr.szs"), Version::Race1);
        assert_eq!(version_of("Race/Kart/ma_kart-mr_2.szs"), Version::Race2);
        assert_eq!(version_of("Race/Kart/ma_kart-mr_4.szs"), Version::Race4);
        assert_eq!(version_of("Scene/Model/Driver.szs"), Version::Menu);
    }

    #[test]
    fn levels_use_the_largest_ratio() {
        let limit = Stats { triangles: 1000, vertices: 1000, model_bytes: 1000, texture_bytes: 1000, file_bytes: 1000 };
        let mut s = Stats { triangles: 1000, vertices: 10, model_bytes: 10, texture_bytes: 10, file_bytes: 10 };
        assert_eq!(level(&s, &limit), Level::Ok);
        s.model_bytes = 24000;
        assert_eq!(level(&s, &limit), Level::Warn);
        s.model_bytes = 24100;
        assert_eq!(level(&s, &limit), Level::Over);
        let mut wide = Stats::default();
        widen(&mut wide, &s);
        widen(&mut wide, &limit);
        assert_eq!(wide, Stats { triangles: 1000, vertices: 1000, model_bytes: 24100, texture_bytes: 1000, file_bytes: 1000 });
    }

    #[test]
    fn real_every_original_driver_fits() {
        let Some(dir) = std::env::var_os("MKSS_TEST_GAME_DIR").and_then(|p| crate::game::resolve_root(Path::new(&p))) else {
            return;
        };
        let chars = crate::game::scan(&dir).unwrap();
        let limits = limits(&dir, &chars).unwrap();
        for (v, l) in &limits {
            println!("{v:?} {l:?}");
        }
        for ch in &chars {
            for rel in ch.files.iter().map(String::as_str).chain([crate::game::DRIVER_SZS]) {
                let s = measure(&driver_brres(&dir, rel, &ch.code).unwrap()).unwrap();
                assert_eq!(level(&s, &limits[&version_of(rel)]), Level::Ok, "{rel} {}", ch.code);
            }
        }
        let mario = chars.iter().find(|c| c.code == "mr").unwrap();
        let s = measure(&driver_brres(&dir, &mario.files[0], "mr").unwrap()).unwrap();
        let l = limits[&version_of(&mario.files[0])];
        println!("mario {s:?} ratio {:.2}", ratio(&s, &l));
        let grown = Stats { model_bytes: l.model_bytes + l.model_bytes / 5, ..s };
        assert_eq!(level(&grown, &l), Level::Warn);
        let over = Stats { triangles: l.triangles * 25, ..s };
        assert_eq!(level(&over, &l), Level::Over);
    }
}
