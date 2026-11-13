use base64::Engine;
use std::io::{Read, Write};
use std::path::Path;

pub const MANIFEST: &str = "project.json";
pub const FORMAT2: &str = "mkss-project/2";
pub const FORMATS1: [&str; 2] = ["mkss-project/1", "mkss-project/1.1"];
const MAX_FILES: usize = 4096;
const MAX_TOTAL: u64 = 512 << 20;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub name: String,
    pub data: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub manifest: String,
    pub files: Vec<ProjectFile>,
}

pub fn safe_name(n: &str) -> bool {
    !n.is_empty()
        && n.len() <= 200
        && n != MANIFEST
        && n.split('/').all(|seg| !seg.is_empty() && seg != "." && seg != ".." && seg.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-')))
}

pub fn write(out: &Path, p: &Project) -> Result<u64, String> {
    serde_json::from_str::<serde_json::Value>(&p.manifest).map_err(|_| "E:project.format")?;
    if p.files.len() > MAX_FILES {
        return Err("E:project.format".into());
    }
    let parent = out.parent().ok_or("E:export.write")?;
    std::fs::create_dir_all(parent).map_err(|_| "E:export.write")?;
    let tmp = parent.join(format!(".{}.mkss-tmp", out.file_name().and_then(|s| s.to_str()).unwrap_or("project")));
    let result = (|| -> Result<(), String> {
        let file = std::fs::File::create(&tmp).map_err(|_| "E:export.write")?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(MANIFEST, opts).map_err(|_| "E:export.write")?;
        zip.write_all(p.manifest.as_bytes()).map_err(|_| "E:export.write")?;
        let mut total = 0u64;
        for f in &p.files {
            if !safe_name(&f.name) {
                return Err("E:project.format".into());
            }
            let bytes = base64::engine::general_purpose::STANDARD.decode(&f.data).map_err(|_| "E:project.format")?;
            total += bytes.len() as u64;
            if total > MAX_TOTAL {
                return Err("E:project.tooLarge".into());
            }
            let o = if f.name.ends_with(".png") { opts.compression_method(zip::CompressionMethod::Stored) } else { opts };
            zip.start_file(f.name.as_str(), o).map_err(|_| "E:export.write")?;
            zip.write_all(&bytes).map_err(|_| "E:export.write")?;
        }
        zip.finish().map_err(|_| "E:export.write")?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if out.exists() {
        std::fs::remove_file(out).map_err(|_| "E:export.write")?;
    }
    std::fs::rename(&tmp, out).map_err(|_| "E:export.write")?;
    Ok(std::fs::metadata(out).map_err(|_| "E:export.write")?.len())
}

pub fn read(path: &Path) -> Result<Project, String> {
    let file = std::fs::File::open(path).map_err(|_| "E:project.read")?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "E:project.format")?;
    if zip.len() > MAX_FILES + 1 {
        return Err("E:project.format".into());
    }
    let mut manifest = None;
    let mut files = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|_| "E:project.format")?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().map_err(|_| "E:project.format")?.to_string();
        total += f.size();
        if total > MAX_TOTAL {
            return Err("E:project.tooLarge".into());
        }
        let mut bytes = Vec::with_capacity(f.size() as usize);
        f.by_ref().take(MAX_TOTAL).read_to_end(&mut bytes).map_err(|_| "E:project.format")?;
        if name == MANIFEST {
            manifest = Some(String::from_utf8(bytes).map_err(|_| "E:project.format")?);
        } else if safe_name(&name) {
            files.push(ProjectFile { name, data: base64::engine::general_purpose::STANDARD.encode(bytes) });
        }
    }
    let manifest = manifest.ok_or("E:project.format")?;
    serde_json::from_str::<serde_json::Value>(&manifest).map_err(|_| "E:project.format")?;
    Ok(Project { manifest, files })
}

pub const META: &str = "meta.json";
pub const DATA: &str = "project.mkss";
const CHARS: &str = "characters/";

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharEntry {
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub saved_at: u64,
    #[serde(default)]
    pub edits: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub updated: u64,
    #[serde(default)]
    pub game: String,
    #[serde(default)]
    pub game_path: String,
    #[serde(default)]
    pub game_id: String,
    #[serde(default)]
    pub characters: Vec<CharEntry>,
    #[serde(default)]
    pub imported_sha: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest2 {
    format: String,
    #[serde(default)]
    tool: String,
    #[serde(default)]
    tool_version: String,
    name: String,
    #[serde(default)]
    created: u64,
    #[serde(default)]
    updated: u64,
    #[serde(default)]
    game: String,
    #[serde(default)]
    game_path: String,
    #[serde(default)]
    game_id: String,
    #[serde(default)]
    characters: Vec<CharEntry>,
    #[serde(default)]
    imported_sha: String,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

pub fn safe_code(code: &str) -> bool {
    !code.is_empty() && code.len() <= 8 && code.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
}

pub fn clean_display_name(name: &str) -> Result<String, String> {
    let s: String = name.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(80).collect();
    if s.is_empty() { Err("E:project.name".into()) } else { Ok(s) }
}

fn new_id(root: &Path) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    loop {
        let n = now_ms().wrapping_mul(1000) + (std::process::id() as u64 % 1000) * 7 + SEQ.fetch_add(1, Ordering::SeqCst);
        let mut v = n;
        let mut s = String::new();
        while v > 0 {
            let d = (v % 36) as u8;
            s.insert(0, if d < 10 { (b'0' + d) as char } else { (b'a' + d - 10) as char });
            v /= 36;
        }
        let id = format!("p{s}");
        if !root.join(&id).exists() {
            return id;
        }
    }
}

fn dir_of(root: &Path, id: &str) -> Result<std::path::PathBuf, String> {
    if !safe_id(id) {
        return Err("E:project.unknown".into());
    }
    Ok(root.join(id))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("E:export.write")?;
    std::fs::create_dir_all(parent).map_err(|_| "E:export.write")?;
    let tmp = parent.join(format!(".{}.tmp", path.file_name().and_then(|s| s.to_str()).unwrap_or("file")));
    std::fs::write(&tmp, bytes).map_err(|_| "E:export.write")?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|_| "E:export.write")?;
    }
    std::fs::rename(&tmp, path).map_err(|_| "E:export.write".into())
}

fn meta_of(id: &str, m: &Manifest2) -> Meta {
    Meta {
        id: id.into(),
        name: m.name.clone(),
        created: m.created,
        updated: m.updated,
        game: m.game.clone(),
        game_path: m.game_path.clone(),
        game_id: m.game_id.clone(),
        characters: m.characters.clone(),
        imported_sha: m.imported_sha.clone(),
    }
}

fn load(root: &Path, id: &str) -> Result<(Manifest2, Vec<ProjectFile>), String> {
    let p = read(&dir_of(root, id)?.join(DATA)).map_err(|e| if e == "E:project.read" { "E:project.unknown".to_string() } else { e })?;
    let m: Manifest2 = serde_json::from_str(&p.manifest).map_err(|_| "E:project.format")?;
    if m.format != FORMAT2 {
        return Err("E:project.format".into());
    }
    Ok((m, p.files))
}

fn store(root: &Path, id: &str, m: &Manifest2, files: Vec<ProjectFile>) -> Result<Meta, String> {
    let dir = dir_of(root, id)?;
    let manifest = serde_json::to_string(m).map_err(|_| "E:export.write")?;
    write(&dir.join(DATA), &Project { manifest, files })?;
    let meta = meta_of(id, m);
    write_atomic(&dir.join(META), &serde_json::to_vec_pretty(&meta).map_err(|_| "E:export.write")?)?;
    Ok(meta)
}

#[derive(Clone, Debug, Default)]
pub struct GameRef {
    pub label: String,
    pub path: String,
    pub id: String,
}

impl GameRef {
    fn apply(&self, m: &mut Manifest2) {
        m.game = self.label.chars().take(200).collect();
        m.game_path = self.path.chars().take(1024).collect();
        m.game_id = self.id.chars().filter(|c| c.is_ascii_alphanumeric()).take(6).collect();
    }
}

fn manifest2(name: String, game: &GameRef) -> Manifest2 {
    let t = now_ms();
    let mut m = Manifest2 {
        format: FORMAT2.into(),
        tool: crate::export::TOOL.into(),
        tool_version: crate::export::VERSION.into(),
        name,
        created: t,
        updated: t,
        game: String::new(),
        game_path: String::new(),
        game_id: String::new(),
        characters: vec![],
        imported_sha: String::new(),
    };
    game.apply(&mut m);
    m
}

pub fn list(root: &Path) -> Vec<Meta> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(root) else { return out };
    for e in rd.flatten() {
        let id = e.file_name().to_string_lossy().into_owned();
        if !safe_id(&id) || !e.path().join(DATA).is_file() {
            continue;
        }
        let meta = std::fs::read(e.path().join(META)).ok().and_then(|b| serde_json::from_slice::<Meta>(&b).ok()).filter(|m| m.id == id);
        match meta {
            Some(m) => out.push(m),
            None => {
                if let Ok((m, _)) = load(root, &id) {
                    let meta = meta_of(&id, &m);
                    let _ = write_atomic(&e.path().join(META), &serde_json::to_vec_pretty(&meta).unwrap_or_default());
                    out.push(meta);
                }
            }
        }
    }
    out.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.name.cmp(&b.name)));
    out
}

pub fn create(root: &Path, name: &str, game: &GameRef) -> Result<Meta, String> {
    let id = new_id(root);
    store(root, &id, &manifest2(clean_display_name(name)?, game), vec![])
}

pub fn set_game(root: &Path, id: &str, game: &GameRef) -> Result<Meta, String> {
    let (mut m, files) = load(root, id)?;
    game.apply(&mut m);
    store(root, id, &m, files)
}

pub fn rename(root: &Path, id: &str, name: &str) -> Result<Meta, String> {
    let (mut m, files) = load(root, id)?;
    m.name = clean_display_name(name)?;
    m.updated = now_ms();
    store(root, id, &m, files)
}

pub fn duplicate(root: &Path, id: &str, name: &str) -> Result<Meta, String> {
    let (mut m, files) = load(root, id)?;
    let t = now_ms();
    m.name = clean_display_name(name)?;
    m.created = t;
    m.updated = t;
    m.imported_sha.clear();
    store(root, &new_id(root), &m, files)
}

pub fn delete(root: &Path, id: &str) -> Result<(), String> {
    let dir = dir_of(root, id)?;
    if !dir.join(DATA).is_file() {
        return Err("E:project.unknown".into());
    }
    std::fs::remove_dir_all(&dir).map_err(|_| "E:export.write".into())
}

pub fn export_file(root: &Path, id: &str, out: &Path) -> Result<u64, String> {
    let (m, files) = load(root, id)?;
    write(out, &Project { manifest: serde_json::to_string(&m).map_err(|_| "E:export.write")?, files })
}

pub fn import(root: &Path, path: &Path, fallback_name: &str) -> Result<Meta, String> {
    let sha = file_sha(path)?;
    import_with(root, path, fallback_name, &sha)
}

fn file_sha(path: &Path) -> Result<String, String> {
    use sha2::Digest;
    let bytes = std::fs::read(path).map_err(|_| "E:project.read")?;
    Ok(sha2::Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect())
}

pub fn open_file(root: &Path, path: &Path, fallback_name: &str) -> Result<(Meta, bool), String> {
    if let (Ok(p), Ok(r)) = (path.canonicalize(), root.canonicalize()) {
        if let Ok(rest) = p.strip_prefix(&r) {
            let parts: Vec<_> = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            if parts.len() == 2 && parts[1] == DATA {
                if let Some(m) = list(root).into_iter().find(|m| m.id == parts[0]) {
                    return Ok((m, true));
                }
            }
        }
    }
    let sha = file_sha(path)?;
    if let Some(m) = list(root).into_iter().find(|m| m.imported_sha == sha) {
        return Ok((m, true));
    }
    Ok((import_with(root, path, fallback_name, &sha)?, false))
}

fn import_with(root: &Path, path: &Path, fallback_name: &str, sha: &str) -> Result<Meta, String> {
    let p = read(path)?;
    let v: serde_json::Value = serde_json::from_str(&p.manifest).map_err(|_| "E:project.format")?;
    let format = v.get("format").and_then(|f| f.as_str()).unwrap_or_default();
    if format == FORMAT2 {
        let mut m: Manifest2 = serde_json::from_value(v).map_err(|_| "E:project.format")?;
        m.name = clean_display_name(&m.name).or_else(|_| clean_display_name(fallback_name))?;
        m.characters.retain(|c| safe_code(&c.code));
        let files = p.files.into_iter().filter(|f| m.characters.iter().any(|c| f.name.starts_with(&format!("{CHARS}{}/", c.code)))).collect();
        m.updated = now_ms();
        m.imported_sha = sha.into();
        return store(root, &new_id(root), &m, files);
    }
    if !FORMATS1.contains(&format) {
        return Err("E:project.format".into());
    }
    let code = v.pointer("/character/code").and_then(|c| c.as_str()).unwrap_or_default().to_string();
    let cname = v.pointer("/character/name").and_then(|c| c.as_str()).unwrap_or(&code).to_string();
    if !safe_code(&code) {
        return Err("E:project.format".into());
    }
    let mut m = manifest2(clean_display_name(fallback_name).unwrap_or_else(|_| "My skin pack".into()), &GameRef::default());
    m.characters.push(CharEntry { code: code.clone(), name: cname, saved_at: m.updated, edits: vec![] });
    m.imported_sha = sha.into();
    let mut files = vec![ProjectFile { name: format!("{CHARS}{code}/{MANIFEST}"), data: base64::engine::general_purpose::STANDARD.encode(p.manifest.as_bytes()) }];
    files.extend(p.files.into_iter().map(|f| ProjectFile { name: format!("{CHARS}{code}/{}", f.name), data: f.data }));
    store(root, &new_id(root), &m, files)
}

pub fn char_read(root: &Path, id: &str, code: &str) -> Result<Option<Project>, String> {
    if !safe_code(code) {
        return Err("E:char.unknown".into());
    }
    let (_, files) = load(root, id)?;
    let prefix = format!("{CHARS}{code}/");
    let mut manifest = None;
    let mut out = Vec::new();
    for f in files {
        let Some(rest) = f.name.strip_prefix(&prefix) else { continue };
        if rest == MANIFEST {
            let bytes = base64::engine::general_purpose::STANDARD.decode(&f.data).map_err(|_| "E:project.format")?;
            manifest = Some(String::from_utf8(bytes).map_err(|_| "E:project.format")?);
        } else {
            out.push(ProjectFile { name: rest.to_string(), data: f.data });
        }
    }
    Ok(manifest.map(|manifest| Project { manifest, files: out }))
}

pub fn char_write(root: &Path, id: &str, code: &str, name: &str, project: Option<&Project>, edits: &[String]) -> Result<Meta, String> {
    if !safe_code(code) {
        return Err("E:char.unknown".into());
    }
    let (mut m, mut files) = load(root, id)?;
    let prefix = format!("{CHARS}{code}/");
    files.retain(|f| !f.name.starts_with(&prefix));
    m.characters.retain(|c| c.code != code);
    let t = now_ms();
    if let Some(p) = project {
        let v: serde_json::Value = serde_json::from_str(&p.manifest).map_err(|_| "E:project.format")?;
        let has_work = v.get("textures").and_then(|x| x.as_array()).is_some_and(|a| !a.is_empty());
        if has_work {
            for f in &p.files {
                if !safe_name(&f.name) {
                    return Err("E:project.format".into());
                }
            }
            files.push(ProjectFile { name: format!("{prefix}{MANIFEST}"), data: base64::engine::general_purpose::STANDARD.encode(p.manifest.as_bytes()) });
            files.extend(p.files.iter().map(|f| ProjectFile { name: format!("{prefix}{}", f.name), data: f.data.clone() }));
            let mut tags: Vec<String> = edits.iter().filter(|e| matches!(e.as_str(), "body" | "eyes" | "bike" | "low")).cloned().collect();
            tags.dedup();
            m.characters.push(CharEntry { code: code.into(), name: clean_display_name(name).unwrap_or_else(|_| code.into()), saved_at: t, edits: tags });
        }
    }
    let order = |c: &str| crate::game::CHARACTERS.iter().position(|(k, _)| *k == c).unwrap_or(usize::MAX);
    m.characters.sort_by_key(|c| order(&c.code));
    m.updated = t;
    store(root, id, &m, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_round_trip() {
        assert!(safe_name("layers/mario_all/l2.png"));
        for bad in ["../x.png", "/abs.png", "a//b", "a/../b", "c:\\x", "project.json", ""] {
            assert!(!safe_name(bad), "{bad}");
        }
        let dir = std::env::temp_dir().join(format!("mkss-proj-{}", std::process::id()));
        let out = dir.join("t.mkss");
        let p = Project {
            manifest: r#"{"format":"mkss-project/1"}"#.into(),
            files: vec![ProjectFile { name: "layers/a/l1.png".into(), data: base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3]) }],
        };
        write(&out, &p).unwrap();
        let r = read(&out).unwrap();
        assert_eq!(r.manifest, p.manifest);
        assert_eq!(r.files.len(), 1);
        assert_eq!(r.files[0].name, "layers/a/l1.png");
        assert_eq!(r.files[0].data, p.files[0].data);
        let bad = Project { manifest: "{}".into(), files: vec![ProjectFile { name: "../evil".into(), data: String::new() }] };
        assert!(write(&dir.join("bad.mkss"), &bad).is_err());
        assert!(write(&dir.join("bad2.mkss"), &Project { manifest: "not json".into(), files: vec![] }).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn store_root(name: &str) -> std::path::PathBuf {
        let base = std::env::var_os("MKSS_TEST_SCRATCH").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
        let d = base.join("mkss-tests").join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn char_project(code: &str, marker: u8) -> Project {
        Project {
            manifest: format!(r#"{{"format":"mkss-project/1.1","character":{{"code":"{code}","name":"N"}},"textures":[{{"name":"t"}}]}}"#),
            files: vec![ProjectFile { name: "layers/t/2.png".into(), data: base64::engine::general_purpose::STANDARD.encode([marker, 1, 2]) }],
        }
    }

    #[test]
    fn project_store_crud_round_trip() {
        let root = store_root("store-crud").join("projects");
        assert!(list(&root).is_empty());
        let game = GameRef { label: "Mario Kart Wii (USA).wbfs".into(), path: "D:/Games/Mario Kart Wii (USA).wbfs".into(), id: "RMCE01".into() };
        let a = create(&root, "  My skins ", &game).unwrap();
        assert_eq!(a.name, "My skins");
        assert!(a.characters.is_empty() && a.game.ends_with(".wbfs") && a.game_id == "RMCE01" && a.game_path.starts_with("D:/"));
        assert!(create(&root, "   ", &GameRef::default()).is_err(), "empty names are refused");
        char_write(&root, &a.id, "pc", "Peach", Some(&char_project("pc", 7)), &["bike".into(), "evil".into()]).unwrap();
        let m = char_write(&root, &a.id, "mr", "Mario", Some(&char_project("mr", 9)), &["body".into(), "eyes".into()]).unwrap();
        assert_eq!(m.characters[1].edits, ["bike"], "unknown edit tags are dropped");
        assert_eq!(m.characters[0].edits, ["body", "eyes"]);
        assert_eq!(m.characters.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(), ["mr", "pc"], "game order");
        let mr = char_read(&root, &a.id, "mr").unwrap().unwrap();
        assert!(mr.manifest.contains(r#""code":"mr""#));
        assert_eq!(mr.files.len(), 1);
        assert_eq!(mr.files[0].name, "layers/t/2.png");
        assert_eq!(mr.files[0].data, char_project("mr", 9).files[0].data);
        assert!(char_read(&root, &a.id, "dk").unwrap().is_none());
        let m = char_write(&root, &a.id, "mr", "Mario", Some(&char_project("mr", 10)), &["body".into()]).unwrap();
        assert_eq!(m.characters.len(), 2);
        assert_eq!(char_read(&root, &a.id, "mr").unwrap().unwrap().files[0].data, char_project("mr", 10).files[0].data);
        assert_eq!(char_read(&root, &a.id, "pc").unwrap().unwrap().files[0].data, char_project("pc", 7).files[0].data, "other character untouched");
        let empty = Project { manifest: r#"{"format":"mkss-project/1.1","character":{"code":"pc","name":"Peach"},"textures":[]}"#.into(), files: vec![] };
        let m = char_write(&root, &a.id, "pc", "Peach", Some(&empty), &[]).unwrap();
        assert_eq!(m.characters.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(), ["mr"]);
        assert!(char_read(&root, &a.id, "pc").unwrap().is_none());
        let r = rename(&root, &a.id, "Blue team").unwrap();
        assert_eq!(r.name, "Blue team");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let d = duplicate(&root, &a.id, "Blue team (copy)").unwrap();
        assert_ne!(d.id, a.id);
        assert_eq!(char_read(&root, &d.id, "mr").unwrap().unwrap().files[0].data, char_project("mr", 10).files[0].data);
        let l = list(&root);
        assert_eq!(l.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["Blue team (copy)", "Blue team"]);
        std::fs::remove_file(root.join(&a.id).join(META)).unwrap();
        assert!(list(&root).iter().any(|m| m.id == a.id && m.name == "Blue team" && m.characters.len() == 1));
        let out = root.parent().unwrap().join("Blue team.mkss");
        export_file(&root, &a.id, &out).unwrap();
        let i = import(&root, &out, "ignored").unwrap();
        assert_eq!((i.name.as_str(), i.characters.len()), ("Blue team", 1));
        assert_eq!(char_read(&root, &i.id, "mr").unwrap().unwrap().files[0].data, char_project("mr", 10).files[0].data);
        delete(&root, &a.id).unwrap();
        assert!(!root.join(&a.id).exists());
        assert!(delete(&root, "../evil").is_err() && delete(&root, &a.id).is_err());
        assert!(char_read(&root, &d.id, "../x").is_err());
        assert_eq!(list(&root).len(), 2);
    }

    #[test]
    fn project_import_single_character_format_1() {
        let root = store_root("store-import1");
        for (format, file) in [("mkss-project/1", "old-v02.mkss"), ("mkss-project/1.1", "Mario skin.mkss")] {
            let old = Project {
                manifest: format!(r#"{{"format":"{format}","tool":"MK Skin Studio","toolVersion":"0.3.1","character":{{"code":"mr","name":"Mario"}},"savedAt":"2026-01-01T00:00:00Z","textures":[{{"name":"mario_all"}}]}}"#),
                files: vec![ProjectFile { name: "layers/mario_all/2.png".into(), data: base64::engine::general_purpose::STANDARD.encode([4u8, 5, 6]) }],
            };
            let path = root.join(file);
            write(&path, &old).unwrap();
            let m = import(&root.join("projects"), &path, "Mario skin").unwrap();
            assert_eq!(m.name, "Mario skin");
            assert_eq!(m.characters, vec![CharEntry { code: "mr".into(), name: "Mario".into(), saved_at: m.updated, edits: vec![] }]);
            assert!(m.game_path.is_empty(), "an imported 1.x project has no game yet");
            let g = set_game(&root.join("projects"), &m.id, &GameRef { label: "x.wbfs".into(), path: "D:/x.wbfs".into(), id: "RMCE01".into() }).unwrap();
            assert_eq!((g.game_path.as_str(), g.game_id.as_str(), g.characters.len()), ("D:/x.wbfs", "RMCE01", 1));
            let c = char_read(&root.join("projects"), &m.id, "mr").unwrap().unwrap();
            assert_eq!(c.manifest, old.manifest, "{format}: the character manifest is kept verbatim");
            assert_eq!(c.files[0].name, "layers/mario_all/2.png");
            assert_eq!(c.files[0].data, old.files[0].data);
        }
        let junk = root.join("junk.mkss");
        write(&junk, &Project { manifest: r#"{"format":"other"}"#.into(), files: vec![] }).unwrap();
        assert!(import(&root.join("projects"), &junk, "x").is_err());
    }
}
