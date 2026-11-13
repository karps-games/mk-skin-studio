pub mod brres;
pub mod disc;
pub mod discovery;
pub mod export;
pub mod game;
pub mod mdl0;
pub mod model;
pub mod parts;
pub mod portraits;
pub mod project;
pub mod skin;
pub mod texmap;
pub mod texture;
pub mod u8arc;
pub mod yaz0;
#[cfg(target_os = "windows")]
mod window_size;
#[cfg(test)]
mod tests;

use base64::Engine;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use texture::Rgba8;

#[derive(Clone)]
struct Edit {
    image: Rgba8,
    summary: export::EditSummary,
}

struct Current {
    ch: game::Character,
    textures: Vec<skin::TextureInfo>,
    originals: HashMap<String, Rgba8>,
    edits: HashMap<String, Edit>,
    model: Option<Arc<model::ModelBundle>>,
}

impl Current {
    fn original(&self, name: &str) -> Result<&Rgba8, String> {
        self.originals.get(name).ok_or_else(|| "E:tex.unknown".into())
    }
}

struct Staged {
    ch: game::Character,
    textures: Vec<skin::TextureInfo>,
    edits: HashMap<String, Edit>,
}

#[derive(Default)]
struct Session {
    root: Option<PathBuf>,
    characters: Vec<game::Character>,
    current: Option<Current>,
    pack: Vec<Staged>,
}

#[derive(Default)]
struct AppState(Mutex<Session>);

fn lock<'a>(state: &'a tauri::State<'_, AppState>) -> std::sync::MutexGuard<'a, Session> {
    state.0.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CharacterSummary {
    code: String,
    name: String,
    files: usize,
    portrait: Option<String>,
}

#[cfg(test)]
mod portrait_contract_tests {
    #[test]
    fn character_summary_serializes_optional_portrait() {
        let summary = super::CharacterSummary { code: "mr".into(), name: "Mario".into(), files: 48, portrait: None };
        let json = serde_json::to_value(summary).unwrap();
        assert_eq!(json.get("portrait"), Some(&serde_json::Value::Null));
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GameInfo {
    root: String,
    source: &'static str,
    id: Option<String>,
    characters: Vec<CharacterSummary>,
}

fn folder_game_id(root: &std::path::Path) -> Option<String> {
    let mut f = std::fs::File::open(game::resolve(root.parent()?, "sys/boot.bin")).ok()?;
    let mut id = [0u8; 6];
    std::io::Read::read_exact(&mut f, &mut id).ok()?;
    id.iter().all(|c| c.is_ascii_alphanumeric()).then(|| String::from_utf8_lossy(&id).into_owned())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TextureView {
    #[serde(flatten)]
    info: skin::TextureInfo,
    preview: String,
    sha256: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CharacterView {
    code: String,
    name: String,
    files: usize,
    menu: bool,
    textures: Vec<TextureView>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageFile {
    name: String,
    width: u32,
    height: u32,
    sha256: String,
    data_url: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResult {
    path: String,
    bytes: u64,
    files: usize,
    textures: usize,
}

#[derive(Clone, serde::Serialize)]
struct Progress {
    done: usize,
    total: usize,
    file: String,
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|_| "E:internal".to_string())?
}

#[tauri::command]
async fn scan_for_game(last_path: Option<String>) -> Result<discovery::DiscoveryResult, String> {
    tauri::async_runtime::spawn_blocking(move || discovery::scan(last_path.as_deref().map(std::path::Path::new)))
        .await
        .map_err(|_| "E:game.scan".to_string())
}

#[tauri::command]
async fn open_game(app: tauri::AppHandle, path: String) -> Result<GameInfo, String> {
    let app2 = app.clone();
    let (root, source, id, chars, mut portraits) = blocking(move || {
        let p = PathBuf::from(&path);
        let (root, source, id) = if p.is_dir() {
            let root = game::resolve_root(&p).ok_or("E:game.notFound")?;
            let id = folder_game_id(&root);
            (root, "folder", id)
        } else if p.is_file() {
            let dir = app2.path().app_local_data_dir().map_err(|_| "E:disc.storage")?;
            (disc::extract(&p, &dir)?, "image", disc::game_id(&p).ok())
        } else {
            return Err("E:game.notFound".into());
        };
        let chars = game::scan(&root)?;
        let portraits = portraits::load(&root);
        Ok((root, source, id, chars, portraits))
    })
    .await?;
    let state = app.state::<AppState>();
    let mut s = lock(&state);
    s.root = Some(root.clone());
    s.characters = chars.clone();
    s.current = None;
    s.pack.clear();
    Ok(GameInfo {
        root: root.display().to_string(),
        source,
        id,
        characters: chars.iter().map(|c| CharacterSummary { code: c.code.clone(), name: c.name.clone(), files: c.files.len(), portrait: portraits.remove(&c.code) }).collect(),
    })
}

#[tauri::command]
async fn load_character(app: tauri::AppHandle, code: String) -> Result<CharacterView, String> {
    let (root, ch) = {
        let state = app.state::<AppState>();
        let s = lock(&state);
        let root = s.root.clone().ok_or("E:game.none")?;
        let ch = s.characters.iter().find(|c| c.code == code).cloned().ok_or("E:char.unknown")?;
        (root, ch)
    };
    let ch2 = ch.clone();
    let (found, textures) = blocking(move || {
        let found = skin::discover(&root, &ch2)?;
        let mut textures = Vec::new();
        for t in &found.textures {
            let img = &found.originals[&t.name];
            textures.push(TextureView { info: t.clone(), preview: texture::data_url(img)?, sha256: export::sha256_hex(&img.pixels) });
        }
        Ok((found, textures))
    })
    .await?;
    let view = CharacterView { code: ch.code.clone(), name: ch.name.clone(), files: ch.files.len(), menu: found.menu_member.is_some(), textures };
    let state = app.state::<AppState>();
    lock(&state).current = Some(Current { ch, textures: found.textures, originals: found.originals, edits: HashMap::new(), model: None });
    Ok(view)
}

#[tauri::command]
async fn model_load(app: tauri::AppHandle, character: String) -> Result<Arc<model::ModelBundle>, String> {
    let (root, ch, sizes, originals, cached) = {
        let state = app.state::<AppState>();
        let s = lock(&state);
        let cur = s.current.as_ref().ok_or("E:char.none")?;
        if cur.ch.code != character {
            return Err("E:char.none".into());
        }
        let sizes: Vec<(String, u32, u32)> = cur.textures.iter().map(|t| (t.name.clone(), t.width, t.height)).collect();
        (s.root.clone().ok_or("E:game.none")?, cur.ch.clone(), sizes, cur.originals.clone(), cur.model.clone())
    };
    if let Some(m) = cached {
        return Ok(m);
    }
    let code = ch.code.clone();
    let bundle = blocking(move || {
        let sources = model::load_sources(&root, &ch)?;
        let map = texmap::analyze_with(&root, &ch, &sizes, Some(&originals)).ok();
        Ok(Arc::new(model::bundle_with(&ch.code, &sources, &sizes, map.as_ref())))
    })
    .await?;
    let state = app.state::<AppState>();
    let mut s = lock(&state);
    if let Some(cur) = s.current.as_mut().filter(|c| c.ch.code == code) {
        cur.model = Some(bundle.clone());
    }
    Ok(bundle)
}

#[tauri::command]
async fn commit_texture(app: tauri::AppHandle, name: String, png: Option<String>, summary: export::EditSummary) -> Result<bool, String> {
    let orig = {
        let state = app.state::<AppState>();
        let s = lock(&state);
        s.current.as_ref().ok_or("E:char.none")?.original(&name)?.clone()
    };
    let decoded = match png {
        None => None,
        Some(b64) => blocking(move || {
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim_start_matches("data:image/png;base64,")).map_err(|_| "E:image.decode")?;
            let img = texture::load(&bytes)?;
            if (img.width, img.height) != (orig.width, orig.height) {
                return Err("E:image.size".into());
            }
            Ok(if img.pixels == orig.pixels { None } else { Some(img) })
        })
        .await?,
    };
    let state = app.state::<AppState>();
    let mut s = lock(&state);
    let cur = s.current.as_mut().ok_or("E:char.none")?;
    match decoded {
        Some(image) => {
            cur.edits.insert(name, Edit { image, summary });
            Ok(true)
        }
        None => {
            cur.edits.remove(&name);
            Ok(false)
        }
    }
}

#[tauri::command]
async fn read_image(path: String) -> Result<ImageFile, String> {
    blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|_| "E:image.read")?;
        if meta.len() > texture::MAX_INPUT_BYTES {
            return Err("E:image.tooLarge".into());
        }
        let bytes = std::fs::read(&path).map_err(|_| "E:image.read")?;
        let img = texture::load(&bytes)?;
        let name = PathBuf::from(&path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(ImageFile { name, width: img.width, height: img.height, sha256: export::sha256_hex(&bytes), data_url: texture::data_url(&img)? })
    })
    .await
}

#[tauri::command]
fn export_original(state: tauri::State<'_, AppState>, name: String, path: String) -> Result<(), String> {
    let s = lock(&state);
    let cur = s.current.as_ref().ok_or("E:char.none")?;
    let png = texture::png(cur.original(&name)?)?;
    std::fs::write(&path, png).map_err(|_| "E:export.write".into())
}

fn edit_records(textures: &[skin::TextureInfo], edits: &HashMap<String, Edit>) -> Vec<export::EditRecord> {
    let mut v: Vec<export::EditRecord> = edits
        .iter()
        .map(|(name, e)| {
            let info = textures.iter().find(|t| &t.name == name);
            export::EditRecord {
                texture: name.clone(),
                width: info.map_or(0, |t| t.width),
                height: info.map_or(0, |t| t.height),
                format: info.map_or(String::new(), |t| t.format.clone()),
                kind: if e.summary.derived_from.is_some() { "derived".into() } else { "layers".into() },
                summary: e.summary.clone(),
            }
        })
        .collect();
    v.sort_by(|a, b| a.texture.cmp(&b.texture));
    v
}

#[tauri::command]
fn pack_begin(state: tauri::State<'_, AppState>) {
    lock(&state).pack.clear();
}

#[tauri::command]
fn pack_stage(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let mut s = lock(&state);
    let cur = s.current.as_ref().ok_or("E:char.none")?;
    let staged = Staged { ch: cur.ch.clone(), textures: cur.textures.clone(), edits: cur.edits.clone() };
    let n = staged.edits.len();
    s.pack.retain(|p| p.ch.code != staged.ch.code);
    if n > 0 {
        s.pack.push(staged);
    }
    Ok(n)
}

#[tauri::command]
fn pack_file_name(name: String) -> String {
    export::pack_file_name(&name)
}

#[tauri::command]
async fn build_pack(app: tauri::AppHandle, name: String, out: String) -> Result<ExportResult, String> {
    let (root, chars, records) = {
        let state = app.state::<AppState>();
        let s = lock(&state);
        if s.pack.is_empty() {
            return Err("E:build.nothing".into());
        }
        let order = |c: &str| game::CHARACTERS.iter().position(|(k, _)| *k == c).unwrap_or(usize::MAX);
        let mut staged: Vec<&Staged> = s.pack.iter().collect();
        staged.sort_by_key(|p| order(&p.ch.code));
        let chars: Vec<(game::Character, HashMap<String, Rgba8>)> =
            staged.iter().map(|p| (p.ch.clone(), p.edits.iter().map(|(n, e)| (n.clone(), e.image.clone())).collect())).collect();
        let records: Vec<export::CharacterRecord> =
            staged.iter().map(|p| export::CharacterRecord { code: p.ch.code.clone(), name: p.ch.name.clone(), edits: edit_records(&p.textures, &p.edits) }).collect();
        (s.root.clone().ok_or("E:game.none")?, chars, records)
    };
    let app2 = app.clone();
    blocking(move || {
        let out = PathBuf::from(out);
        let built = skin::build_pack(&root, &chars, |done, total, file| {
            let _ = app2.emit("mkss-progress", Progress { done, total, file: file.into() });
        })?;
        let textures = records.iter().map(|c| c.edits.len()).sum();
        let m = export::manifest(&name, records, &built);
        let bytes = export::write_zip(&out, &m, &built)?;
        Ok(ExportResult { path: out.display().to_string(), bytes, files: built.len(), textures })
    })
    .await
}

fn projects_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|_| "E:disc.storage")?.join("projects"))
}

#[tauri::command]
async fn projects_list(app: tauri::AppHandle) -> Result<Vec<project::Meta>, String> {
    let root = projects_dir(&app)?;
    blocking(move || Ok(project::list(&root))).await
}

#[tauri::command]
async fn project_create(app: tauri::AppHandle, name: String, game: String, game_path: String, game_id: String) -> Result<project::Meta, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::create(&root, &name, &project::GameRef { label: game, path: game_path, id: game_id })).await
}

#[tauri::command]
async fn project_set_game(app: tauri::AppHandle, id: String, game: String, game_path: String, game_id: String) -> Result<project::Meta, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::set_game(&root, &id, &project::GameRef { label: game, path: game_path, id: game_id })).await
}

#[tauri::command]
async fn project_rename(app: tauri::AppHandle, id: String, name: String) -> Result<project::Meta, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::rename(&root, &id, &name)).await
}

#[tauri::command]
async fn project_duplicate(app: tauri::AppHandle, id: String, name: String) -> Result<project::Meta, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::duplicate(&root, &id, &name)).await
}

#[tauri::command]
async fn project_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let root = projects_dir(&app)?;
    let auto = autosave_dir(&app)?.join(&id);
    blocking(move || {
        project::delete(&root, &id)?;
        let _ = std::fs::remove_dir_all(auto);
        Ok(())
    })
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Opened {
    meta: project::Meta,
    existing: bool,
}

#[tauri::command]
async fn project_import(app: tauri::AppHandle, path: String, name: String) -> Result<Opened, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::open_file(&root, &PathBuf::from(path), &name).map(|(meta, existing)| Opened { meta, existing })).await
}

#[derive(Default)]
struct OpenRequests(Mutex<Vec<String>>);

pub fn mkss_args<I: IntoIterator<Item = String>>(args: I, cwd: &std::path::Path) -> Vec<String> {
    args.into_iter()
        .filter(|a| a.to_ascii_lowercase().ends_with(".mkss"))
        .map(|a| {
            let p = PathBuf::from(&a);
            if p.is_absolute() { p } else { cwd.join(p) }
        })
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

fn queue_open(app: &tauri::AppHandle, files: Vec<String>) {
    if files.is_empty() {
        return;
    }
    if let Some(q) = app.try_state::<OpenRequests>() {
        q.0.lock().unwrap().extend(files);
    }
    let _ = app.emit("mkss-open", ());
}

#[tauri::command]
fn take_open_requests(state: tauri::State<'_, OpenRequests>) -> Vec<String> {
    std::mem::take(&mut *state.0.lock().unwrap())
}

#[tauri::command]
async fn project_export_file(app: tauri::AppHandle, id: String, path: String) -> Result<u64, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::export_file(&root, &id, &PathBuf::from(path))).await
}

#[tauri::command]
async fn project_char_read(app: tauri::AppHandle, id: String, code: String) -> Result<Option<project::Project>, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::char_read(&root, &id, &code)).await
}

#[tauri::command]
async fn project_char_write(app: tauri::AppHandle, id: String, code: String, name: String, project: Option<project::Project>, edits: Vec<String>) -> Result<project::Meta, String> {
    let root = projects_dir(&app)?;
    blocking(move || project::char_write(&root, &id, &code, &name, project.as_ref(), &edits)).await
}

fn autosave_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_local_data_dir().map_err(|_| "E:disc.storage")?.join("autosave"))
}

fn autosave_path(app: &tauri::AppHandle, id: &str, code: &str) -> Result<PathBuf, String> {
    if id.is_empty() || id.len() > 40 || !id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) || !project::safe_code(code) {
        return Err("E:project.unknown".into());
    }
    Ok(autosave_dir(app)?.join(id).join(format!("{code}.mkss")))
}

#[tauri::command]
async fn autosave_write(app: tauri::AppHandle, id: String, code: String, project: project::Project) -> Result<(), String> {
    let p = autosave_path(&app, &id, &code)?;
    blocking(move || project::write(&p, &project).map(|_| ())).await
}

#[tauri::command]
async fn autosave_read(app: tauri::AppHandle, id: String, code: String) -> Result<Option<project::Project>, String> {
    let p = autosave_path(&app, &id, &code)?;
    blocking(move || if p.is_file() { project::read(&p).map(Some) } else { Ok(None) }).await
}

#[tauri::command]
fn autosave_clear(app: tauri::AppHandle, id: String, code: String) -> Result<(), String> {
    let p = autosave_path(&app, &id, &code)?;
    if p.is_file() {
        std::fs::remove_file(p).map_err(|_| "E:export.write")?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
            queue_open(app, mkss_args(argv.into_iter().skip(1), std::path::Path::new(&cwd)));
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .manage(OpenRequests::default())
        .setup(|app| {
            let cwd = std::env::current_dir().unwrap_or_default();
            app.state::<OpenRequests>().0.lock().unwrap().extend(mkss_args(std::env::args().skip(1), &cwd));
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                window_size::install(&window)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_for_game,
            open_game,
            load_character,
            model_load,
            commit_texture,
            read_image,
            export_original,
            pack_begin,
            pack_stage,
            pack_file_name,
            build_pack,
            projects_list,
            project_create,
            project_set_game,
            project_rename,
            project_duplicate,
            project_delete,
            project_import,
            project_export_file,
            project_char_read,
            project_char_write,
            autosave_write,
            autosave_read,
            autosave_clear,
            take_open_requests
        ])
        .build(tauri::generate_context!())
        .expect("error while running MK Skin Studio")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                let files = urls.iter().filter_map(|u| u.to_file_path().ok()).map(|p| p.to_string_lossy().into_owned()).filter(|p| p.to_ascii_lowercase().ends_with(".mkss")).collect();
                queue_open(_app, files);
            }
        });
}
