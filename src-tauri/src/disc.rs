use std::io::Read;
use std::path::{Path, PathBuf};

const CACHE_SCHEMA: &str = "mkss-disc-v3-nod";

fn wanted(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    (p.starts_with("race/kart/") && p.ends_with(".szs") && !p[10..].contains('/'))
        || p == "scene/model/driver.szs"
        || p == crate::portraits::UI_ARCHIVE.to_ascii_lowercase()
}

fn canonical(path: &str) -> String {
    let p = path.to_ascii_lowercase();
    if p.starts_with("race/kart/") {
        format!("Race/Kart/{}", &path[10..])
    } else if p == crate::game::DRIVER_SZS.to_ascii_lowercase() {
        crate::game::DRIVER_SZS.into()
    } else if p == crate::portraits::UI_ARCHIVE.to_ascii_lowercase() {
        crate::portraits::UI_ARCHIVE.into()
    } else {
        path.into()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ImageKind {
    Iso,
    Wbfs,
    Rvz,
    Other,
}

pub fn classify(path: &Path) -> Result<ImageKind, String> {
    let mut head = vec![0u8; 0x400];
    let mut f = std::fs::File::open(path).map_err(|_| "E:game.read")?;
    let n = f.read(&mut head).map_err(|_| "E:game.read")?;
    head.truncate(n);
    if head.len() >= 4 && (&head[..4] == b"RVZ\x01" || &head[..4] == b"WIA\x01") {
        return Ok(ImageKind::Rvz);
    }
    let id_ok = |at: usize| head.get(at..at + 3) == Some(b"RMC");
    if head.len() >= 0x220 && &head[..4] == b"WBFS" {
        return if id_ok(0x200) { Ok(ImageKind::Wbfs) } else { Err("E:game.wrongGame".into()) };
    }
    if head.len() >= 0x20 && head[0x18..0x1C] == [0x5D, 0x1C, 0x9E, 0xA3] {
        return if id_ok(0) { Ok(ImageKind::Iso) } else { Err("E:game.wrongGame".into()) };
    }
    Ok(ImageKind::Other)
}

pub fn game_id(path: &Path) -> Result<String, String> {
    let disc = nod::Disc::new(path).map_err(|_| "E:game.unsupportedImage")?;
    Ok(disc.header().game_id_str().to_string())
}

fn cache_key(image: &Path) -> Result<String, String> {
    let m = std::fs::metadata(image).map_err(|_| "E:game.read")?;
    let mtime = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
    let canon = std::fs::canonicalize(image).unwrap_or_else(|_| image.to_path_buf());
    let key = format!("{CACHE_SCHEMA}|{}|{}|{}", canon.display(), m.len(), mtime);
    Ok(crate::export::sha256_hex(key.as_bytes())[..24].to_string())
}

fn safe_join(out: &Path, rel: &str) -> Option<PathBuf> {
    let mut p = out.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
            return None;
        }
        p.push(part);
    }
    Some(p)
}

pub fn extract(image: &Path, app_dir: &Path) -> Result<PathBuf, String> {
    if classify(image)? == ImageKind::Other {
        nod::Disc::new(image).map_err(|_| "E:game.unsupportedImage")?;
    }
    let games = app_dir.join("games");
    let dest = games.join(cache_key(image)?);
    let ready = dest.join("ready.txt");
    if ready.is_file() {
        if let Some(root) = crate::game::resolve_root(&dest.join("extracted")) {
            return Ok(root);
        }
    }

    let disc = nod::Disc::new(image).map_err(|_| "E:disc.open")?;
    if !disc.header().game_id_str().starts_with("RMC") {
        return Err("E:game.wrongGame".into());
    }
    let mut part = disc.open_partition_kind(nod::PartitionKind::Data).map_err(|_| "E:disc.open")?;
    let meta = part.meta().map_err(|_| "E:disc.extract")?;
    let fst = meta.fst().map_err(|_| "E:disc.extract")?;

    let stage = games.join(format!("{}.staging", dest.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&stage);
    let out = stage.join("extracted").join("files");
    std::fs::create_dir_all(&out).map_err(|_| "E:disc.storage")?;

    let mut dirs: Vec<(String, usize)> = Vec::new();
    let mut count = 0usize;
    for (i, node, name) in fst.iter() {
        if i == 0 {
            continue;
        }
        while dirs.last().is_some_and(|(_, end)| i >= *end) {
            dirs.pop();
        }
        let name = name.map_err(|_| "E:disc.extract")?;
        let path = dirs.iter().map(|(d, _)| d.as_str()).chain([name.as_ref()]).collect::<Vec<_>>().join("/");
        if node.is_dir() {
            dirs.push((name.to_string(), node.length() as usize));
            continue;
        }
        if !wanted(&path) {
            continue;
        }
        let dest_file = safe_join(&out, &canonical(&path)).ok_or("E:disc.extract")?;
        let mut buf = Vec::with_capacity(node.length() as usize);
        part.open_file(node).map_err(|_| "E:disc.extract")?.read_to_end(&mut buf).map_err(|_| "E:disc.extract")?;
        if buf.len() as u64 != node.length() as u64 {
            return Err("E:disc.extract".into());
        }
        std::fs::create_dir_all(dest_file.parent().unwrap()).map_err(|_| "E:disc.storage")?;
        std::fs::write(&dest_file, &buf).map_err(|_| "E:disc.storage")?;
        count += 1;
    }
    if count == 0 || crate::game::resolve_root(&stage.join("extracted")).is_none() {
        let _ = std::fs::remove_dir_all(&stage);
        return Err("E:disc.extract".into());
    }
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::rename(&stage, &dest).map_err(|_| "E:disc.storage")?;
    std::fs::write(&ready, b"ok").map_err(|_| "E:disc.storage")?;
    crate::game::resolve_root(&dest.join("extracted")).ok_or_else(|| "E:disc.extract".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wanted_files_only() {
        assert!(wanted("Race/Kart/ma_kart-mr.szs"));
        assert!(wanted("race/kart/sa_bike-bmr_4.szs"));
        assert!(wanted("Scene/Model/Driver.szs"));
        assert!(wanted(crate::portraits::UI_ARCHIVE));
        assert!(!wanted("Race/Kart/sub/x.szs"));
        assert!(!wanted("Race/Course/luigi_course.szs"));
        assert!(!wanted("Scene/Model/Kart/mr-allkart.szs"));
        assert!(!wanted("rel/StaticR.rel"));
        assert_eq!(canonical("race/KART/ma_kart-mr.szs"), "Race/Kart/ma_kart-mr.szs");
        assert_eq!(canonical("SCENE/model/driver.szs"), crate::game::DRIVER_SZS);
        assert_eq!(canonical("scene/ui/menusingle.szs"), crate::portraits::UI_ARCHIVE);
        assert_eq!(canonical("Race/Kart/sa_bike-bmr_4.szs"), "Race/Kart/sa_bike-bmr_4.szs");
    }

    #[test]
    fn safe_join_rejects_escapes() {
        let o = Path::new("out");
        assert!(safe_join(o, "Race/Kart/a.szs").is_some());
        assert!(safe_join(o, "../a").is_none());
        assert!(safe_join(o, "Race//a").is_none());
        assert!(safe_join(o, "C:/a").is_none());
        assert!(safe_join(o, "a\\b").is_none());
    }

    #[test]
    fn unknown_bytes_are_not_a_disc() {
        let dir = std::env::temp_dir().join(format!("mkss-disc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.iso");
        std::fs::write(&f, vec![7u8; 4096]).unwrap();
        assert_eq!(classify(&f).unwrap(), ImageKind::Other);
        assert_eq!(extract(&f, &dir).unwrap_err(), "E:game.unsupportedImage");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[ignore = "opt-in MKSS_TEST_IMAGE owned disc, MKSS_TEST_SCRATCH output"]
    fn real_image_extracts_and_caches() {
        let image = PathBuf::from(std::env::var_os("MKSS_TEST_IMAGE").expect("set MKSS_TEST_IMAGE"));
        let scratch = PathBuf::from(std::env::var_os("MKSS_TEST_SCRATCH").expect("set MKSS_TEST_SCRATCH")).join("disc-integration");
        let before = std::fs::metadata(&image).unwrap();
        let t0 = std::time::Instant::now();
        let root = extract(&image, &scratch).unwrap();
        println!("extract {:?}: {}", t0.elapsed(), root.display());
        assert!(game_id(&image).unwrap().starts_with("RMC"));
        assert_eq!(crate::game::scan(&root).unwrap().len(), 24);
        assert!(root.join(crate::portraits::UI_ARCHIVE).is_file());
        assert_eq!(crate::portraits::load(&root).len(), 24);
        assert_eq!(extract(&image, &scratch).unwrap(), root, "second call reuses the cache");
        if let Some(reference) = std::env::var_os("MKSS_TEST_GAME_DIR") {
            let reference = PathBuf::from(reference);
            for rel in ["Race/Kart/ma_kart-mr.szs", "Scene/Model/Driver.szs"] {
                assert_eq!(std::fs::read(root.join(rel)).unwrap(), std::fs::read(reference.join(rel)).unwrap(), "{rel}");
            }
        }
        let after = std::fs::metadata(&image).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    }
}
