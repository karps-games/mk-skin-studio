use crate::{brres, game, texture, texture::Rgba8, u8arc, yaz0};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub const DRIVER_MEMBER: &str = "driver_model.brres";

pub fn editable_name(name: &str) -> bool {
    !(name.starts_with("lm_") || name == "lm")
}

pub fn is_eye(name: &str) -> bool {
    name.to_ascii_lowercase().contains("eye")
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextureInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub images: u32,
    pub files: usize,
    pub menu: bool,
    pub eye: bool,
}

pub struct Discovered {
    pub textures: Vec<TextureInfo>,
    pub originals: HashMap<String, Rgba8>,
    pub menu_member: Option<String>,
}

pub fn read_archive(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(game::resolve(root, rel)).map_err(|_| format!("E:game.readFile|file={rel}"))?;
    if yaz0::is_yaz0(&bytes) {
        yaz0::decode(&bytes).map_err(|_| format!("E:game.badFile|file={rel}"))
    } else {
        Ok(bytes)
    }
}

fn member<'a>(raw: &'a [u8], arc: &u8arc::Archive, name: &str) -> Option<(usize, &'a [u8])> {
    let idx = arc.files.iter().position(|f| f.path.trim_start_matches("./") == name)?;
    let f = &arc.files[idx];
    Some((idx, &raw[f.offset..f.offset + f.size]))
}

pub fn discover(root: &Path, ch: &game::Character) -> Result<Discovered, String> {
    struct Acc {
        info: TextureInfo,
        area: u32,
        image: Rgba8,
    }
    let mut acc: BTreeMap<String, Acc> = BTreeMap::new();
    let mut visit = |brres_bytes: &[u8], menu: bool, rel: &str| -> Result<(), String> {
        let texs = brres::textures(brres_bytes).map_err(|_| format!("E:game.badFile|file={rel}"))?;
        for t in texs {
            if !editable_name(&t.name) || !matches!(t.format, 0..=6 | 14) {
                continue;
            }
            let area = t.width * t.height;
            let entry = acc.entry(t.name.clone());
            let decode = || texture::decode(&brres_bytes[t.data..t.data + t.len], t.width, t.height, t.format);
            match entry {
                std::collections::btree_map::Entry::Vacant(v) => {
                    let image = decode().map_err(|_| format!("E:game.badFile|file={rel}"))?;
                    v.insert(Acc {
                        info: TextureInfo {
                            name: t.name.clone(),
                            width: t.width,
                            height: t.height,
                            format: brres::format_name(t.format).into(),
                            images: t.images,
                            files: usize::from(!menu),
                            menu,
                            eye: is_eye(&t.name),
                        },
                        area,
                        image,
                    });
                }
                std::collections::btree_map::Entry::Occupied(mut o) => {
                    let a = o.get_mut();
                    a.info.files += usize::from(!menu);
                    a.info.menu |= menu;
                    if area > a.area {
                        a.image = decode().map_err(|_| format!("E:game.badFile|file={rel}"))?;
                        a.area = area;
                        a.info.width = t.width;
                        a.info.height = t.height;
                        a.info.format = brres::format_name(t.format).into();
                        a.info.images = t.images;
                    }
                }
            }
        }
        Ok(())
    };
    for rel in &ch.files {
        let raw = read_archive(root, rel)?;
        let arc = u8arc::parse(&raw).map_err(|_| format!("E:game.badFile|file={rel}"))?;
        let (_, b) = member(&raw, &arc, DRIVER_MEMBER).ok_or_else(|| format!("E:game.badFile|file={rel}"))?;
        visit(b, false, rel)?;
    }
    let raw = read_archive(root, game::DRIVER_SZS)?;
    let arc = u8arc::parse(&raw).map_err(|_| format!("E:game.badFile|file={}", game::DRIVER_SZS))?;
    let menu_member = game::menu_member(&arc, &ch.code);
    if let Some(m) = &menu_member {
        let (_, b) = member(&raw, &arc, m).unwrap();
        visit(b, true, game::DRIVER_SZS)?;
    }
    if acc.is_empty() {
        return Err("E:char.noTextures".into());
    }
    let mut list: Vec<Acc> = acc.into_values().collect();
    list.sort_by(|a, b| {
        (a.info.eye, std::cmp::Reverse(a.area), &a.info.name).cmp(&(b.info.eye, std::cmp::Reverse(b.area), &b.info.name))
    });
    let originals = list.iter().map(|a| (a.info.name.clone(), a.image.clone())).collect();
    Ok(Discovered { textures: list.into_iter().map(|a| a.info).collect(), originals, menu_member })
}

pub fn patch_archive(raw: &mut [u8], brres_name: &str, edited: &HashMap<String, Rgba8>, cache: &mut HashMap<(String, u32, u32, u32, u32), Vec<u8>>) -> Result<usize, String> {
    let arc = u8arc::parse(raw)?;
    let idx = arc.files.iter().position(|f| f.path.trim_start_matches("./") == brres_name).ok_or("E:file.format")?;
    if !u8arc::exclusive(&arc, idx) {
        return Err("E:file.format".into());
    }
    let f = arc.files[idx].clone();
    let brres_bytes = &mut raw[f.offset..f.offset + f.size];
    let texs = brres::textures(brres_bytes)?;
    let mut n = 0;
    for t in texs {
        let Some(img) = edited.get(&t.name) else { continue };
        let key = (t.name.clone(), t.width, t.height, t.format, t.images);
        if !cache.contains_key(&key) {
            let sized = texture::resize(img, t.width, t.height);
            cache.insert(key.clone(), texture::encode(&sized, t.format, t.images)?);
        }
        let enc = &cache[&key];
        if enc.len() != t.len {
            return Err("E:build.size".into());
        }
        brres_bytes[t.data..t.data + t.len].copy_from_slice(enc);
        n += 1;
    }
    Ok(n)
}

pub struct BuiltFile {
    pub path: String,
    pub data: Vec<u8>,
    pub textures: usize,
}

pub fn build(
    root: &Path,
    ch: &game::Character,
    edited: &HashMap<String, Rgba8>,
    progress: impl Fn(usize, usize, &str) + Sync,
) -> Result<Vec<BuiltFile>, String> {
    build_pack(root, &[(ch.clone(), edited.clone())], progress)
}

pub fn build_pack(
    root: &Path,
    chars: &[(game::Character, HashMap<String, Rgba8>)],
    progress: impl Fn(usize, usize, &str) + Sync,
) -> Result<Vec<BuiltFile>, String> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Mutex;
    if chars.iter().all(|(_, e)| e.is_empty()) {
        return Err("E:build.nothing".into());
    }
    let mut jobs: Vec<(Option<usize>, String)> = Vec::new();
    for (ci, (ch, edited)) in chars.iter().enumerate() {
        if !edited.is_empty() {
            jobs.extend(ch.files.iter().map(|f| (Some(ci), f.clone())));
        }
    }
    jobs.push((None, game::DRIVER_SZS.to_string()));
    let total = jobs.len();
    type Cache = HashMap<(String, u32, u32, u32, u32), Vec<u8>>;
    let race = |ci: usize, rel: &str, caches: &mut HashMap<usize, Cache>| -> Result<Option<BuiltFile>, String> {
        let mut raw = read_archive(root, rel)?;
        let before = raw.len();
        let n = patch_archive(&mut raw, DRIVER_MEMBER, &chars[ci].1, caches.entry(ci).or_default()).map_err(|e| if e.contains('|') { e } else { format!("{e}|file={rel}") })?;
        if raw.len() != before {
            return Err(format!("E:build.size|file={rel}"));
        }
        Ok(if n > 0 { Some(BuiltFile { path: rel.to_string(), data: yaz0::encode(&raw)?, textures: n }) } else { None })
    };
    let menu = |rel: &str| -> Result<Option<BuiltFile>, String> {
        let mut raw = read_archive(root, rel)?;
        let before = raw.len();
        let mut n = 0;
        for (ch, edited) in chars {
            if edited.is_empty() {
                continue;
            }
            let member = game::menu_member(&u8arc::parse(&raw).map_err(|e| format!("{e}|file={rel}"))?, &ch.code);
            if let Some(m) = member {
                n += patch_archive(&mut raw, &m, edited, &mut HashMap::new()).map_err(|e| if e.contains('|') { e } else { format!("{e}|file={rel}") })?;
            }
        }
        if raw.len() != before {
            return Err(format!("E:build.size|file={rel}"));
        }
        Ok(if n > 0 { Some(BuiltFile { path: rel.to_string(), data: yaz0::encode(&raw)?, textures: n }) } else { None })
    };
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let results: Mutex<Vec<Option<Result<Option<BuiltFile>, String>>>> = Mutex::new((0..total).map(|_| None).collect());
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
    progress(0, total, &jobs[0].1);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut caches = HashMap::new();
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= total || failed.load(Ordering::SeqCst) {
                        break;
                    }
                    let (ci, rel) = &jobs[i];
                    let r = match ci {
                        Some(ci) => race(*ci, rel, &mut caches),
                        None => menu(rel),
                    };
                    if r.is_err() {
                        failed.store(true, Ordering::SeqCst);
                    }
                    results.lock().unwrap()[i] = Some(r);
                    progress(done.fetch_add(1, Ordering::SeqCst) + 1, total, rel);
                }
            });
        }
    });
    let mut out = Vec::new();
    for r in results.into_inner().unwrap().into_iter().flatten() {
        if let Some(b) = r? {
            out.push(b);
        }
    }
    Ok(out)
}
