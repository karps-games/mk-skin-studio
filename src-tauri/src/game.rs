use std::path::{Path, PathBuf};

pub const CHARACTERS: &[(&str, &str)] = &[
    ("bmr", "Baby Mario"),
    ("blg", "Baby Luigi"),
    ("bpc", "Baby Peach"),
    ("bds", "Baby Daisy"),
    ("ko", "Toad"),
    ("kk", "Toadette"),
    ("nk", "Koopa Troopa"),
    ("ka", "Dry Bones"),
    ("mr", "Mario"),
    ("lg", "Luigi"),
    ("pc", "Peach"),
    ("ds", "Daisy"),
    ("ys", "Yoshi"),
    ("ca", "Birdo"),
    ("dd", "Diddy Kong"),
    ("jr", "Bowser Jr."),
    ("wr", "Wario"),
    ("wl", "Waluigi"),
    ("dk", "Donkey Kong"),
    ("kp", "Bowser"),
    ("kt", "King Boo"),
    ("rs", "Rosalina"),
    ("fk", "Funky Kong"),
    ("bk", "Dry Bowser"),
];

pub const DRIVER_SZS: &str = "Scene/Model/Driver.szs";

pub fn display_name(code: &str) -> Option<&'static str> {
    CHARACTERS.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

pub fn resolve(root: &Path, rel: &str) -> PathBuf {
    let mut p = root.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty()) {
        let exact = p.join(part);
        if std::fs::symlink_metadata(&exact).is_ok() {
            p = exact;
            continue;
        }
        let found = std::fs::read_dir(&p).ok().and_then(|rd| {
            let mut names: Vec<_> = rd.flatten().map(|e| e.file_name()).filter(|n| n.to_str().is_some_and(|n| n.eq_ignore_ascii_case(part))).collect();
            names.sort();
            names.into_iter().next()
        });
        p = match found { Some(name) => p.join(name), None => exact };
    }
    p
}

pub fn resolve_root(path: &Path) -> Option<PathBuf> {
    let candidates = [
        path.to_path_buf(),
        resolve(path, "files"),
        resolve(path, "DATA/files"),
        resolve(path, "disc/files"),
    ];
    candidates.into_iter().find(|c| resolve(c, "Race/Kart").is_dir() && resolve(c, DRIVER_SZS).is_file())
}

pub fn character_of(file_name: &str) -> Option<&'static str> {
    let stem = file_name.strip_suffix(".szs")?;
    let (vehicle, rest) = stem.split_once('-')?;
    if vehicle.is_empty() || !vehicle.bytes().all(|c| c.is_ascii_lowercase() || c == b'_') {
        return None;
    }
    let code = rest.strip_suffix("_2").or_else(|| rest.strip_suffix("_4")).unwrap_or(rest);
    CHARACTERS.iter().find(|(c, _)| *c == code).map(|(c, _)| *c)
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Character {
    pub code: String,
    pub name: String,
    pub files: Vec<String>,
}

pub fn scan(root: &Path) -> Result<Vec<Character>, String> {
    let dir = resolve(root, "Race/Kart");
    let mut by_code: std::collections::BTreeMap<&str, Vec<String>> = Default::default();
    for e in std::fs::read_dir(&dir).map_err(|_| "E:game.read")? {
        let e = e.map_err(|_| "E:game.read")?;
        if !e.file_type().map_err(|_| "E:game.read")?.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(code) = character_of(&name) {
            by_code.entry(code).or_default().push(format!("Race/Kart/{name}"));
        }
    }
    let mut out = Vec::new();
    for (code, name) in CHARACTERS {
        if let Some(mut files) = by_code.remove(code) {
            files.sort();
            out.push(Character { code: code.to_string(), name: name.to_string(), files });
        }
    }
    if out.is_empty() {
        return Err("E:game.noCharacters".into());
    }
    Ok(out)
}

pub fn menu_member(arc: &crate::u8arc::Archive, code: &str) -> Option<String> {
    [format!("{code}.brres"), format!("{code}_menu.brres")].into_iter().find(|n| arc.find(n).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_map_to_characters_and_skip_miis() {
        assert_eq!(character_of("ma_kart-mr.szs"), Some("mr"));
        assert_eq!(character_of("ma_kart-mr_2.szs"), Some("mr"));
        assert_eq!(character_of("sa_bike-bmr_4.szs"), Some("bmr"));
        assert_eq!(character_of("ma_kart-ma_mii_f.szs"), None);
        assert_eq!(character_of("ma_kart-la_mii_m_2.szs"), None);
        assert_eq!(character_of("mr-allkart.szs"), None);
        assert_eq!(character_of("ma_kart-mr.brres"), None);
        assert_eq!(CHARACTERS.len(), 24);
        assert_eq!(display_name("bk"), Some("Dry Bowser"));
    }

    #[test]
    fn folders_resolve_case_insensitively() {
        let base = std::env::var_os("MKSS_TEST_SCRATCH").map(PathBuf::from).unwrap_or_else(std::env::temp_dir)
            .join(format!("mkss-case-{}", std::process::id()));
        let root = base.join("DISC").join("Files");
        std::fs::create_dir_all(root.join("race").join("KART")).unwrap();
        std::fs::create_dir_all(root.join("SCENE").join("model")).unwrap();
        std::fs::write(root.join("SCENE/model/Driver.szs"), b"x").unwrap();
        std::fs::write(root.join("race/KART/ma_kart-mr.szs"), b"x").unwrap();
        assert_eq!(resolve(&root, "nothing/here"), root.join("nothing").join("here"));
        assert!(resolve(&root, "Race/Kart").is_dir());
        assert!(resolve(&root, DRIVER_SZS).is_file());
        assert!(resolve(&root, "Race/Kart/ma_kart-mr.szs").is_file());
        let found = resolve_root(&base).expect("disc/files in any case");
        assert!(found.file_name().unwrap().to_string_lossy().eq_ignore_ascii_case("files"));
        assert_eq!(resolve_root(&root).as_deref(), Some(root.as_path()));
        let chars = scan(&root).unwrap();
        assert_eq!(chars[0].files, vec!["Race/Kart/ma_kart-mr.szs".to_string()]);
        assert!(crate::skin::read_archive(&root, &chars[0].files[0]).is_ok());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
