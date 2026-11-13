use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCandidate {
    pub path: String,
    pub id: String,
    pub region: String,
    pub format: String,
    pub size_bytes: u64,
}

#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub games: Vec<GameCandidate>,
    pub truncated: bool,
}

#[derive(Clone, Copy)]
pub struct ScanLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_dirs: usize,
    pub max_duration: Duration,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self { max_depth: 5, max_entries: 50_000, max_dirs: 5_000, max_duration: Duration::from_secs(5) }
    }
}

fn excluded_dir(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
    name.starts_with('.') || [
        "windows", "program files", "program files (x86)", "programdata", "$recycle.bin",
        "system volume information", "appdata", "winsxs", "recovery", "node_modules",
        "target", "build", "dist", "cache", "caches", "__pycache__", "vendor",
        "lost+found", "snap",
    ].contains(&name.as_str()) || platform_excluded(&name)
}

#[cfg(target_os = "macos")]
fn platform_excluded(name: &str) -> bool {
    name == "library" || name == "applications" || name.ends_with(".app") || name.ends_with(".photoslibrary")
}
#[cfg(not(target_os = "macos"))]
fn platform_excluded(_name: &str) -> bool { false }

#[cfg(unix)]
fn virtual_tree(path: &Path) -> bool {
    ["/proc", "/sys", "/dev", "/run/user", "/var/run", "/private/var/run"].iter().any(|p| path.starts_with(p))
}

#[cfg(any(target_os = "linux", test))]
fn remote_fs(fs: &str) -> bool {
    matches!(fs, "nfs" | "nfs4" | "cifs" | "smb3" | "smbfs" | "ncpfs" | "afs" | "ceph" | "glusterfs" | "9p" | "davfs" | "virtiofs")
        || ["fuse.sshfs", "fuse.rclone", "fuse.gvfs", "fuse.s3", "fuse.davfs", "fuse.onedriver", "fuse.google-drive"].iter().any(|p| fs.starts_with(p))
}

#[cfg(target_os = "linux")]
fn network_mount(path: &Path) -> bool {
    use std::sync::OnceLock;
    static MOUNTS: OnceLock<Vec<(PathBuf, String)>> = OnceLock::new();
    let mounts = MOUNTS.get_or_init(|| {
        let text = std::fs::read_to_string("/proc/self/mounts").unwrap_or_default();
        text.lines().filter_map(|line| {
            let mut it = line.split_whitespace();
            let (_dev, dir, fs) = (it.next()?, it.next()?, it.next()?);
            let dir = dir.replace("\\040", " ").replace("\\011", "\t").replace("\\134", "\\");
            Some((PathBuf::from(dir), fs.to_ascii_lowercase()))
        }).collect()
    });
    mounts.iter().filter(|(dir, _)| path.starts_with(dir)).max_by_key(|(dir, _)| dir.as_os_str().len())
        .is_some_and(|(_, fs)| remote_fs(fs))
}

#[cfg(target_os = "macos")]
fn network_mount(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return true };
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return false;
    }
    (st.f_flags as u64) & (libc::MNT_LOCAL as u64) == 0
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn network_mount(_path: &Path) -> bool { false }

fn is_link(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() { return true; }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 { return true; }
    }
    false
}

fn local_path(path: &Path) -> bool {
    if !path.is_absolute() { return false; }
    #[cfg(windows)] {
        use std::path::{Component, Prefix};
        let Some(Component::Prefix(prefix)) = path.components().next() else { return false };
        let drive = match prefix.kind() { Prefix::Disk(c) | Prefix::VerbatimDisk(c) => c, _ => return false };
        let root = [drive as u16, b':' as u16, b'\\' as u16, 0];
        #[link(name = "kernel32")]
        unsafe extern "system" { fn GetDriveTypeW(root: *const u16) -> u32; }
        return matches!(unsafe { GetDriveTypeW(root.as_ptr()) }, 2 | 3);
    }
    #[cfg(unix)] { !virtual_tree(path) && !network_mount(path) }
}

fn safe_path(path: &Path) -> bool {
    if !local_path(path) { return false; }
    if path.components().any(|c| c == std::path::Component::ParentDir) { return false; }
    let ancestors: Vec<_> = path.ancestors().collect();
    for ancestor in ancestors.into_iter().rev() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(meta) if !is_link(&meta) => {},
            _ => return false,
        }
    }
    true
}

fn identify(path: &Path) -> Option<GameCandidate> {
    use std::io::{Read, Seek, SeekFrom};
    let name = path.file_name()?.to_str()?.to_ascii_lowercase();
    if name.contains(".nkit.") { return None; }
    let ext = path.extension()?.to_str()?;
    let wbfs = ext.eq_ignore_ascii_case("wbfs");
    let compressed = ext.eq_ignore_ascii_case("rvz") || ext.eq_ignore_ascii_case("wia");
    if compressed {
        if !safe_path(path) || !std::fs::symlink_metadata(path).ok()?.is_file() { return None; }
        let mut magic = [0u8; 4];
        std::fs::File::open(path).ok()?.read_exact(&mut magic).ok()?;
        if &magic != b"RVZ\x01" && &magic != b"WIA\x01" { return None; }
        let id = crate::disc::game_id(path).ok()?;
        let region = match id.as_str() { "RMCE01" => "USA", "RMCP01" => "Europe", "RMCJ01" => "Japan", "RMCK01" => "Korea", _ => return None };
        return Some(GameCandidate {
            path: std::fs::canonicalize(path).ok()?.to_str()?.into(), id, region: region.into(),
            format: if &magic == b"RVZ\x01" { "RVZ" } else { "WIA" }.into(), size_bytes: std::fs::metadata(path).ok()?.len(),
        });
    }
    if !wbfs && !ext.eq_ignore_ascii_case("iso") { return None; }
    if !safe_path(path) || !std::fs::symlink_metadata(path).ok()?.is_file() { return None; }
    let mut file = std::fs::File::open(path).ok()?;
    let mut head = [0u8; 0x20];
    file.read_exact(&mut head).ok()?;
    if wbfs {
        if &head[..4] != b"WBFS" || head[8] != 9 || !(15..=31).contains(&head[9]) || head[12] != 1 {
            return None;
        }
        file.seek(SeekFrom::Start(0x200)).ok()?;
        file.read_exact(&mut head).ok()?;
    } else {
        file.seek(SeekFrom::Start(0x200)).ok()?;
        let mut nkit = [0u8; 4];
        if file.read_exact(&mut nkit).is_ok() && &nkit == b"NKIT" { return None; }
    }
    if head[0x18..0x1c] != [0x5d, 0x1c, 0x9e, 0xa3] { return None; }
    let region = match &head[..6] {
        b"RMCE01" => "USA", b"RMCP01" => "Europe", b"RMCJ01" => "Japan", b"RMCK01" => "Korea", _ => return None,
    };
    Some(GameCandidate {
        path: std::fs::canonicalize(path).ok()?.to_str()?.into(),
        id: std::str::from_utf8(&head[..6]).ok()?.into(), region: region.into(),
        format: if wbfs { "WBFS" } else { "ISO" }.into(), size_bytes: file.metadata().ok()?.len(),
    })
}

#[cfg(windows)]
pub const HOME_FOLDERS: &[&str] = &["Desktop", "Downloads", "Documents", "Videos", "Movies", "OneDrive"];
#[cfg(target_os = "macos")]
pub const HOME_FOLDERS: &[&str] = &["Downloads", "Documents", "Games", "Desktop", "Movies"];
#[cfg(all(unix, not(target_os = "macos")))]
pub const HOME_FOLDERS: &[&str] = &["Downloads", "Documents", "Games", "Desktop", "Videos"];

#[cfg(windows)]
pub const DOLPHIN_CONFIGS: &[&str] = &[
    "AppData/Roaming/Dolphin Emulator/Config/Dolphin.ini",
    "Documents/Dolphin Emulator/Config/Dolphin.ini",
];
#[cfg(target_os = "macos")]
pub const DOLPHIN_CONFIGS: &[&str] = &["Library/Application Support/Dolphin/Config/Dolphin.ini"];
#[cfg(all(unix, not(target_os = "macos")))]
pub const DOLPHIN_CONFIGS: &[&str] = &[
    ".config/dolphin-emu/Dolphin.ini",
    ".local/share/dolphin-emu/Config/Dolphin.ini",
    ".var/app/org.DolphinEmu.dolphin-emu/config/dolphin-emu/Dolphin.ini",
    ".dolphin-emu/Config/Dolphin.ini",
];

const MAX_ISO_PATHS: usize = 128;
const MAX_VOLUMES: usize = 32;

pub fn default_roots_at(home: &Path) -> (Vec<PathBuf>, bool) {
    use std::io::Read;
    const MAX_CONFIG: usize = 65_536;
    let mut roots: Vec<_> = HOME_FOLDERS
        .iter().map(|name| home.join(name)).filter(|p| local_path(p)).collect();
    let mut truncated = false;
    let mut path_count = 0;
    for relative in DOLPHIN_CONFIGS {
        let ini = home.join(relative);
        if !safe_path(&ini) { continue; }
        let Ok(file) = std::fs::File::open(&ini) else { truncated = true; continue };
        let mut bytes = Vec::new();
        if file.take((MAX_CONFIG + 1) as u64).read_to_end(&mut bytes).is_err() || bytes.len() > MAX_CONFIG {
            truncated = true;
            continue;
        }
        let Ok(text) = std::str::from_utf8(&bytes) else { truncated = true; continue };
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Some(suffix) = key.trim().strip_prefix("ISOPath") else { continue };
            if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) { continue; }
            if path_count >= MAX_ISO_PATHS { truncated = true; break; }
            path_count += 1;
            let value = value.trim();
            let value = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value);
            let path = PathBuf::from(value);
            if local_path(&path) && !roots.contains(&path) { roots.push(path); }
        }
    }
    #[cfg(unix)]
    if local_path(home) && !roots.iter().any(|r| r.as_path() == home) { roots.push(home.to_path_buf()); }
    (roots, truncated)
}

#[cfg(unix)]
fn volumes_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries.flatten().take(256)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()))
        .map(|e| e.path()).collect();
    out.sort();
    out.truncate(MAX_VOLUMES);
    out
}

#[cfg(target_os = "macos")]
fn volume_dirs() -> Vec<PathBuf> { vec![PathBuf::from("/Volumes")] }
#[cfg(all(unix, not(target_os = "macos")))]
fn volume_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(user) = std::env::var_os("USER").or_else(|| std::env::var_os("LOGNAME")) {
        dirs.push(Path::new("/media").join(&user));
        dirs.push(Path::new("/run/media").join(&user));
    }
    dirs.push(PathBuf::from("/mnt"));
    dirs
}

pub const fn max_default_roots() -> usize {
    if cfg!(windows) { HOME_FOLDERS.len() + MAX_ISO_PATHS + 3 + 26 } else { HOME_FOLDERS.len() + MAX_ISO_PATHS + 1 + 3 * MAX_VOLUMES }
}

pub fn default_roots() -> (Vec<PathBuf>, bool) {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    #[cfg(unix)]
    let home = std::env::var_os("HOME").map(PathBuf::from).filter(|h| h.is_absolute())
        .map(|h| std::fs::canonicalize(&h).unwrap_or(h));
    let (mut roots, truncated) = home.as_deref().map(default_roots_at).unwrap_or_default();
    #[cfg(windows)] {
        for key in ["OneDrive", "OneDriveConsumer", "OneDriveCommercial"] {
            if let Some(value) = std::env::var_os(key) {
                let p = PathBuf::from(value);
                if local_path(&p) && !roots.contains(&p) { roots.push(p); }
            }
        }
        for c in b'A'..=b'Z' {
            let drive = PathBuf::from(format!("{}:\\", c as char));
            if local_path(&drive) && !roots.contains(&drive) { roots.push(drive); }
        }
    }
    #[cfg(unix)] {
        let home_last = roots.last().is_some_and(|r| home.as_deref() == Some(r.as_path()));
        let at = if home_last { roots.len() - 1 } else { roots.len() };
        let mut volumes = Vec::new();
        for dir in volume_dirs() {
            for volume in volumes_in(&dir) {
                if local_path(&volume) && !roots.contains(&volume) && !volumes.contains(&volume) { volumes.push(volume); }
            }
        }
        roots.splice(at..at, volumes);
    }
    (roots, truncated)
}

pub fn scan(last_path: Option<&Path>) -> DiscoveryResult {
    let started = std::time::Instant::now();
    let (roots, config_truncated) = default_roots();
    let mut limits = ScanLimits::default();
    limits.max_duration = limits.max_duration.saturating_sub(started.elapsed());
    let mut result = scan_at(&roots, last_path, limits);
    result.truncated |= config_truncated;
    result
}

pub fn scan_at(roots: &[PathBuf], last_path: Option<&Path>, limits: ScanLimits) -> DiscoveryResult {
    use std::collections::{HashMap, HashSet};
    let started = std::time::Instant::now();
    let expired = || started.elapsed() >= limits.max_duration;
    let mut result = DiscoveryResult::default();
    let mut seen = HashSet::new();
    let mut entries_read = 0;
    let mut dirs_read = 0;
    if expired() || limits.max_entries == 0 || limits.max_dirs == 0 {
        result.truncated = !roots.is_empty() || last_path.is_some();
        return result;
    }
    let hint = last_path.and_then(|p| { entries_read += 1; identify(p) });
    let priority = hint.as_ref().map(|g| g.path.clone());
    if let Some(g) = hint { seen.insert(g.path.clone()); result.games.push(g); }
    let mut visited = HashMap::<PathBuf, usize>::new();
    let root_count = roots.len().min(limits.max_dirs);
    result.truncated = root_count < roots.len();
    let mut stack: Vec<_> = roots[..root_count].iter().rev().map(|r| (r.clone(), 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        if expired() || dirs_read >= limits.max_dirs || entries_read >= limits.max_entries {
            result.truncated = true;
            break;
        }
        if excluded_dir(&dir) || !safe_path(&dir) { continue; }
        let Ok(real) = std::fs::canonicalize(&dir) else { continue };
        if visited.get(&real).is_some_and(|&d| d <= depth) { continue; }
        visited.insert(real, depth);
        dirs_read += 1;
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => { result.truncated = true; continue; }
        };
        let mut collected = Vec::new();
        let mut stopped = false;
        for entry in entries {
            if expired() || entries_read >= limits.max_entries {
                result.truncated = true;
                stopped = true;
                break;
            }
            entries_read += 1;
            match entry {
                Ok(entry) => collected.push(entry),
                Err(_) => result.truncated = true,
            }
        }
        collected.sort_by_key(|e| e.path());
        let mut children = Vec::new();
        for entry in collected {
            if expired() { result.truncated = true; stopped = true; break; }
            let p = entry.path();
            let Ok(ft) = entry.file_type() else { result.truncated = true; continue };
            if ft.is_symlink() { continue; }
            if ft.is_dir() {
                if excluded_dir(&p) { continue; }
                let Ok(meta) = std::fs::symlink_metadata(&p) else { result.truncated = true; continue };
                if is_link(&meta) { continue; }
                if depth < limits.max_depth { children.push((p, depth + 1)); }
                else { result.truncated = true; }
            } else if ft.is_file() {
                if let Some(g) = identify(&p) {
                    if seen.insert(g.path.clone()) { result.games.push(g); }
                }
            }
        }
        stack.extend(children.into_iter().rev());
        if stopped { break; }
    }
    result.games.sort_by(|a, b| {
        let rank = |g: &GameCandidate| if Some(&g.path) == priority.as_ref() { 0 } else { 1 };
        rank(a).cmp(&rank(b)).then_with(|| a.path.cmp(&b.path))
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::var_os("MKSS_TEST_SCRATCH").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
                .join(format!("mkss-discovery-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir_all(&root).unwrap();
            #[cfg(unix)] let root = fs::canonicalize(&root).unwrap();
            Self(root)
        }
        fn image(&self, name: &str, id: &[u8; 6], wbfs: bool) -> PathBuf {
            let p = self.0.join(name);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            let mut bytes = vec![0u8; 0x400];
            let at = if wbfs {
                bytes[..4].copy_from_slice(b"WBFS");
                bytes[8] = 9;
                bytes[9] = 21;
                bytes[12] = 1;
                0x200
            } else { 0 };
            bytes[at..at + 6].copy_from_slice(id);
            bytes[at + 0x18..at + 0x1c].copy_from_slice(&[0x5d, 0x1c, 0x9e, 0xa3]);
            fs::write(&p, bytes).unwrap();
            p
        }
        fn scan(&self) -> DiscoveryResult { scan_at(&[self.0.clone()], None, ScanLimits::default()) }
    }
    impl Drop for Fixture {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn command_is_registered_and_runs_on_blocking_worker() {
        let source = include_str!("lib.rs");
        let handler = source.split("tauri::generate_handler![").nth(1).unwrap().split(']').next().unwrap();
        assert!(handler.split(|c: char| !c.is_alphanumeric() && c != '_').any(|t| t == "scan_for_game"), "missing IPC registration");
        let command = source.split("async fn scan_for_game(").nth(1).expect("missing asynchronous command").split("#[tauri::command]").next().unwrap();
        assert!(command.contains("last_path: Option<String>"));
        assert!(command.contains("Result<discovery::DiscoveryResult, String>"));
        assert!(command.contains("spawn_blocking"));
        assert!(command.contains("discovery::scan("));
    }

    #[test]
    fn runtime_default_roots_are_local_and_bounded() {
        let (roots, _) = default_roots();
        assert!(!roots.is_empty(), "runtime roots must include local user folders or drives");
        assert!(roots.len() <= max_default_roots(), "home folders + 128 Dolphin + drives/volumes");
        assert!(roots.iter().all(|p| local_path(p)));
        #[cfg(windows)] for path in [r"\\server\share", r"\\?\UNC\server\share", r"\\.\PhysicalDrive0", "C:relative", "relative"] {
            assert!(!local_path(Path::new(path)), "reject before any filesystem access: {path}");
        }
    }

    #[test]
    #[ignore = "opt-in: set MKSS_DISCOVERY_GAME and run discovery::tests::real_image_read_only --ignored"]
    fn real_image_read_only() {
        let path = PathBuf::from(std::env::var_os("MKSS_DISCOVERY_GAME").expect("set MKSS_DISCOVERY_GAME to your own ISO/WBFS"));
        let before = fs::metadata(&path).unwrap();
        let result = scan_at(&[], Some(&path), ScanLimits::default());
        assert_eq!(result.games.len(), 1, "real image must have a supported header");
        let after = fs::metadata(&path).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        assert_eq!(before.permissions().readonly(), after.permissions().readonly());
        assert!(matches!(crate::disc::classify(&path).unwrap(), crate::disc::ImageKind::Iso | crate::disc::ImageKind::Wbfs));
        println!("read-only discovery: {} {} {}", result.games[0].id, result.games[0].format, result.games[0].size_bytes);
    }

    #[test]
    #[ignore = "opt-in: set MKSS_DISCOVERY_GAME; performs the same bounded local scan as startup"]
    fn real_default_scan_without_hint() {
        let path = PathBuf::from(std::env::var_os("MKSS_DISCOVERY_GAME").expect("set MKSS_DISCOVERY_GAME to your own ISO/WBFS"));
        let before = fs::metadata(&path).unwrap();
        let expected = fs::canonicalize(&path).unwrap();
        let started = std::time::Instant::now();
        let result = scan(None);
        let found = result.games.iter().any(|g| Path::new(&g.path) == expected);
        println!("default scan: found_owned_game={found}, candidates={}, truncated={}, elapsed_ms={}", result.games.len(), result.truncated, started.elapsed().as_millis());
        assert!(found, "owned fixture must be found by normal startup roots without a remembered-path hint");
        let after = fs::metadata(&path).unwrap();
        assert_eq!(before.len(), after.len());
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        assert_eq!(before.permissions().readonly(), after.permissions().readonly());
    }

    #[test]
    fn default_roots_read_only_bounded_dolphin_paths() {
        for config in DOLPHIN_CONFIGS {
            let f = Fixture::new();
            let ini = f.0.join(config);
            fs::create_dir_all(ini.parent().unwrap()).unwrap();
            let games = f.0.join("my-games");
            let bogus = f.0.join("bogus");
            fs::create_dir(&games).unwrap();
            fs::write(&ini, format!("[General]\nISOPaths = 5\nISOPath0 = {}\nISOPath1 = \\\\server\\share\nISOPathCount = 4\nISOPathBogus = {}\nISOPath2 = relative/path\n", games.display(), bogus.display())).unwrap();
            let before = fs::metadata(&ini).unwrap();
            let (roots, truncated) = default_roots_at(&f.0);
            for name in HOME_FOLDERS {
                assert!(roots.contains(&f.0.join(name)), "missing {name}");
            }
            assert!(roots.contains(&games), "{config}");
            assert!(!roots.contains(&bogus));
            assert!(!roots.iter().any(|r| r.ends_with("relative/path") || r.to_string_lossy().contains("server")));
            assert_eq!(roots.last() == Some(&f.0), cfg!(unix));
            assert_eq!(roots.contains(&f.0), cfg!(unix));
            assert!(roots.iter().all(|p| local_path(p)));
            assert!(!truncated);
            assert_eq!(before.len(), fs::metadata(&ini).unwrap().len());
            assert_eq!(before.modified().unwrap(), fs::metadata(&ini).unwrap().modified().unwrap());
            fs::write(&ini, vec![b'x'; 65_537]).unwrap();
            let (roots, truncated) = default_roots_at(&f.0);
            assert!(!roots.contains(&games));
            assert!(truncated, "oversize config is an incomplete search");
        }
    }

    #[test]
    fn platform_search_locations() {
        #[cfg(windows)] {
            assert!(DOLPHIN_CONFIGS.contains(&"AppData/Roaming/Dolphin Emulator/Config/Dolphin.ini"));
            assert!(HOME_FOLDERS.contains(&"OneDrive"));
        }
        #[cfg(target_os = "macos")] {
            assert_eq!(DOLPHIN_CONFIGS, &["Library/Application Support/Dolphin/Config/Dolphin.ini"]);
            assert_eq!(volume_dirs(), vec![PathBuf::from("/Volumes")]);
            assert!(excluded_dir(Path::new("/Users/x/Library")) && excluded_dir(Path::new("/Applications/Foo.app")));
        }
        #[cfg(target_os = "linux")] {
            for c in [".config/dolphin-emu/Dolphin.ini", ".local/share/dolphin-emu/Config/Dolphin.ini", ".var/app/org.DolphinEmu.dolphin-emu/config/dolphin-emu/Dolphin.ini"] {
                assert!(DOLPHIN_CONFIGS.contains(&c), "{c}");
            }
            assert!(HOME_FOLDERS.contains(&"Games"));
            assert!(volume_dirs().contains(&PathBuf::from("/mnt")));
        }
    }

    #[test]
    fn remote_file_systems_are_not_local() {
        for fs in ["nfs", "nfs4", "cifs", "smb3", "fuse.sshfs", "fuse.rclone", "9p"] { assert!(remote_fs(fs), "{fs}"); }
        for fs in ["ext4", "btrfs", "xfs", "vfat", "exfat", "ntfs3", "fuseblk", "apfs", "tmpfs"] { assert!(!remote_fs(fs), "{fs}"); }
    }

    #[cfg(unix)]
    #[test]
    fn unix_rejects_virtual_trees_and_lists_volumes_without_links() {
        for p in ["/proc", "/proc/self", "/sys/class", "/dev", "/run/user/1000"] {
            assert!(!local_path(Path::new(p)), "{p}");
        }
        assert!(!local_path(Path::new("relative/dir")));
        let f = Fixture::new();
        f.image("Vol1/game.wbfs", b"RMCP01", true);
        f.image("Vol2/sub/game.iso", b"RMCE01", false);
        std::os::unix::fs::symlink(f.0.join("Vol1"), f.0.join("VolLink")).unwrap();
        let vols = volumes_in(&f.0);
        assert_eq!(vols, vec![f.0.join("Vol1"), f.0.join("Vol2")], "links are not volumes");
        let result = scan_at(&vols, None, ScanLimits::default());
        assert_eq!(result.games.len(), 2);
        assert!(!result.truncated);
    }

    #[test]
    fn excludes_system_build_and_cache_directories() {
        let f = Fixture::new();
        f.image("ok/game.iso", b"RMCE01", false);
        for name in ["Windows", "Program Files", "AppData", "node_modules", ".git", "target", "build", "dist", "cache", ".hidden", "$RECYCLE.BIN", "System Volume Information"] {
            f.image(&format!("{name}/game.iso"), b"RMCE01", false);
        }
        let result = f.scan();
        assert_eq!(result.games.len(), 1);
        assert!(!result.truncated);
        assert!(scan_at(&[f.0.join("target")], None, ScanLimits::default()).games.is_empty());
    }

    #[test]
    fn never_follows_directory_links_including_roots_and_hint_ancestors() {
        let f = Fixture::new();
        let outside = Fixture::new();
        let game = outside.image("game.iso", b"RMCE01", false);
        let link = f.0.join("linked");
        #[cfg(windows)] {
            let output = std::process::Command::new("cmd.exe").args(["/d", "/c", "mklink", "/J"])
                .arg(link.to_string_lossy().replace('/', "\\")).arg(outside.0.to_string_lossy().replace('/', "\\")).output().unwrap();
            assert!(output.status.success(), "junction creation failed: {:?}", output);
        }
        #[cfg(unix)] std::os::unix::fs::symlink(&outside.0, &link).unwrap();
        assert!(f.scan().games.is_empty(), "must not follow a child junction");
        assert!(scan_at(&[link.clone()], None, ScanLimits::default()).games.is_empty(), "must not follow a root junction");
        assert!(scan_at(&[link.join("sub")], Some(&link.join("game.iso")), ScanLimits::default()).games.is_empty(), "must not follow a hint ancestor junction");
        #[cfg(windows)] {
            match std::os::windows::fs::symlink_file(&game, f.0.join("alias.iso")) {
                Ok(()) => assert!(f.scan().games.is_empty(), "must not open a file symlink"),
                Err(e) if e.raw_os_error() == Some(1314) => println!("file symlink creation unavailable without Windows privilege; junction cases exercised"),
                Err(e) => panic!("file symlink creation: {e}"),
            }
        }
        #[cfg(unix)] {
            std::os::unix::fs::symlink(&game, f.0.join("alias.iso")).unwrap();
            assert!(f.scan().games.is_empty());
        }
        #[cfg(windows)] fs::remove_dir(&link).unwrap();
        #[cfg(unix)] fs::remove_file(&link).unwrap();
    }

    #[test]
    fn bounds_depth_entries_directories_and_deadline() {
        let f = Fixture::new();
        f.image("a.iso", b"RMCE01", false);
        f.image("one/b.iso", b"RMCE01", false);
        f.image("one/two/c.iso", b"RMCE01", false);
        let base = ScanLimits::default();
        let result = scan_at(&[f.0.clone()], None, ScanLimits { max_depth: 0, ..base });
        assert_eq!(result.games.len(), 1);
        assert!(result.truncated);
        let result = scan_at(&[f.0.clone()], None, ScanLimits { max_depth: 1, ..base });
        assert_eq!(result.games.len(), 2);
        assert!(result.truncated);
        let result = scan_at(&[f.0.clone(), f.0.join("one")], None, ScanLimits { max_depth: 1, ..base });
        assert_eq!(result.games.len(), 3, "a shallower explicit root must revisit overlap");
        for limits in [
            ScanLimits { max_entries: 0, ..base },
            ScanLimits { max_dirs: 0, ..base },
            ScanLimits { max_duration: Duration::ZERO, ..base },
        ] {
            let result = scan_at(&[f.0.clone()], None, limits);
            assert!(result.games.is_empty());
            assert!(result.truncated);
        }
        let result = scan_at(&[f.0.clone()], None, ScanLimits { max_entries: 1, ..base });
        assert!(result.games.len() <= 1);
        assert!(result.truncated);
        let result = scan_at(&[f.0.clone()], None, ScanLimits { max_dirs: 1, ..base });
        assert_eq!(result.games.len(), 1);
        assert!(result.truncated);
        assert!(!f.scan().truncated);
    }

    #[test]
    fn recursive_scan_deduplicates_overlap_and_prioritizes_valid_hint() {
        let f = Fixture::new();
        let a = f.image("a.iso", b"RMCE01", false);
        let b = f.image("deep/b.wbfs", b"RMCP01", true);
        f.image("deep/nested/c.iso", b"RMCJ01", false);
        let roots = [f.0.clone(), f.0.join("deep"), f.0.clone()];
        let result = scan_at(&roots, Some(&b), ScanLimits::default());
        assert_eq!(result.games.len(), 3);
        assert_eq!(PathBuf::from(&result.games[0].path), fs::canonicalize(&b).unwrap());
        assert_eq!(PathBuf::from(&result.games[1].path), fs::canonicalize(&a).unwrap());
        assert!(!result.truncated);
        let again = scan_at(&roots, Some(&b), ScanLimits::default());
        assert_eq!(serde_json::to_value(result).unwrap(), serde_json::to_value(again).unwrap());
        let bad = f.image("bad.iso", b"SMNE01", false);
        assert_eq!(scan_at(&[], Some(&bad), ScanLimits::default()).games.len(), 0);
        assert_eq!(scan_at(&[], Some(&b), ScanLimits::default()).games.len(), 1);
        assert!(scan_at(&[], Some(&f.0), ScanLimits::default()).games.is_empty(), "hint must be an image, never a traversal root");
    }

    #[test]
    fn accepts_only_extractor_compatible_valid_headers() {
        let f = Fixture::new();
        for (id, region) in [(b"RMCE01", "USA"), (b"RMCP01", "Europe"), (b"RMCJ01", "Japan"), (b"RMCK01", "Korea")] {
            for wbfs in [false, true] {
                let path = f.image(&format!("{region}.{}", if wbfs { "wbfs" } else { "iso" }), id, wbfs);
                let g = identify(&path).expect("valid disc header");
                assert_eq!(g.region, region);
                assert_eq!(g.format, if wbfs { "WBFS" } else { "ISO" });
                assert!(matches!(crate::disc::classify(&path).unwrap(), crate::disc::ImageKind::Iso | crate::disc::ImageKind::Wbfs));
            }
        }
        for ext in ["rvz", "wia", "nkit.iso", "nkit.wbfs", "gcm", "ciso"] {
            assert!(identify(&f.image(&format!("unsupported.{ext}"), b"RMCE01", false)).is_none(), "{ext}");
        }
        for id in [b"RMCE00", b"RMCA01", b"RMCE02", b"SMNE01"] {
            assert!(identify(&f.image("wrong.iso", id, false)).is_none());
        }
        for wbfs in [false, true] {
            let p = f.image(if wbfs { "bad.wbfs" } else { "bad.iso" }, b"RMCE01", wbfs);
            let mut bytes = fs::read(&p).unwrap();
            let at = if wbfs { 0x200 } else { 0 };
            bytes[at + 0x18] = 0;
            fs::write(&p, &bytes).unwrap();
            assert!(identify(&p).is_none());
        }
        let nkit = f.image("renamed.iso", b"RMCE01", false);
        let mut bytes = fs::read(&nkit).unwrap();
        bytes[0x200..0x204].copy_from_slice(b"NKIT");
        fs::write(&nkit, bytes).unwrap();
        assert!(identify(&nkit).is_none());
        for len in [0, 4, 9, 0x1b, 0x210, 0x21b] {
            let p = f.image("short.wbfs", b"RMCE01", true);
            let file = fs::OpenOptions::new().write(true).open(&p).unwrap();
            file.set_len(len).unwrap();
            assert!(identify(&p).is_none(), "truncated {len}");
        }
        for exponent in [0, 8, 10, 31, 63, 64, 255] {
            let p = f.image("sector.wbfs", b"RMCE01", true);
            let mut bytes = fs::read(&p).unwrap();
            bytes[8] = exponent;
            fs::write(&p, bytes).unwrap();
            assert!(identify(&p).is_none(), "unsupported sector exponent {exponent}");
        }
        for (offset, value) in [(9, 0), (9, 255), (12, 0), (12, 2)] {
            let p = f.image("malformed.wbfs", b"RMCE01", true);
            let mut bytes = fs::read(&p).unwrap();
            bytes[offset] = value;
            fs::write(&p, bytes).unwrap();
            assert!(identify(&p).is_none(), "malformed WBFS {offset}={value}");
        }
    }

    #[test]
    fn discovers_iso_with_camel_case_contract() {
        let f = Fixture::new();
        let path = f.image("game.ISO", b"RMCE01", false);
        let result = f.scan();
        assert_eq!(result.games.len(), 1);
        let g = &result.games[0];
        assert_eq!((&*g.id, &*g.region, &*g.format, g.size_bytes), ("RMCE01", "USA", "ISO", 1024));
        assert_eq!(PathBuf::from(&g.path), fs::canonicalize(path).unwrap());
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["games"][0]["sizeBytes"], 1024);
        assert_eq!(json["truncated"], false);
    }
}
