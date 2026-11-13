use crate::skin::BuiltFile;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

pub const TOOL: &str = "MK Skin Studio";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRecord {
    pub name: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EditSummary {
    pub layers: u32,
    pub adjustments: u32,
    pub base_edited: bool,
    pub sources: Vec<SourceRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub eye_frame: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditRecord {
    pub texture: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub kind: String,
    #[serde(flatten)]
    pub summary: EditSummary,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
    pub textures: usize,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterRecord {
    pub code: String,
    pub name: String,
    pub edits: Vec<EditRecord>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub tool: String,
    pub tool_version: String,
    pub name: String,
    pub characters: Vec<CharacterRecord>,
    pub files: Vec<FileRecord>,
    pub notice: String,
}

pub fn sha256_hex(b: &[u8]) -> String {
    let d = Sha256::digest(b);
    d.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn clean_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') { c } else { ' ' })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('.').trim().chars().take(60).collect::<String>();
    if s.is_empty() { "My skin pack".into() } else { s }
}

pub fn pack_file_name(name: &str) -> String {
    format!("{}.zip", clean_name(name))
}

fn readme(m: &Manifest) -> String {
    let names: Vec<&str> = m.characters.iter().map(|c| c.name.as_str()).collect();
    let mut lines = vec![
        format!("{} - skin pack ({})", m.name, names.join(", ")),
        format!("Made with MK Skin Studio {} (Karps Games), a skin studio for Mario Kart Wii.", m.tool_version),
        String::new(),
        "INSTALL".into(),
        "- MK Launcher: Mods > Import, then choose this .zip.".into(),
        "- Riivolution / My Stuff loaders: copy the files to the same paths of your game".into(),
        "  (or into your My Stuff folder).".into(),
        String::new(),
        "NOTE: Scene/Model/Driver.szs contains the menu models of ALL characters. Every character".into(),
        "of this pack is already merged into it, but another mod that also replaces Driver.szs".into(),
        "will conflict with this one: only one of them wins.".into(),
        String::new(),
        "Files:".into(),
    ];
    lines.extend(m.files.iter().map(|f| format!("  {}", f.path)));
    lines.extend([
        String::new(),
        "These files were generated from the user's own copy of the game. Do not redistribute".into(),
        "game files you do not have the right to share.".into(),
        "Not affiliated with Nintendo.".into(),
        String::new(),
    ]);
    lines.join("\r\n")
}

pub fn manifest(name: &str, characters: Vec<CharacterRecord>, built: &[BuiltFile]) -> Manifest {
    Manifest {
        format: "mkss-skin/2".into(),
        tool: TOOL.into(),
        tool_version: VERSION.into(),
        name: clean_name(name),
        characters,
        files: built
            .iter()
            .map(|b| FileRecord { path: b.path.clone(), bytes: b.data.len() as u64, sha256: sha256_hex(&b.data), textures: b.textures })
            .collect(),
        notice: "Not affiliated with Nintendo.".into(),
    }
}

pub fn write_zip(out: &Path, m: &Manifest, built: &[BuiltFile]) -> Result<u64, String> {
    let parent = out.parent().ok_or("E:export.write")?;
    let tmp = parent.join(format!(".{}.mkss-tmp", out.file_name().and_then(|s| s.to_str()).unwrap_or("skin")));
    let result = (|| -> Result<(), String> {
        let file = std::fs::File::create(&tmp).map_err(|_| "E:export.write")?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for b in built {
            zip.start_file(b.path.as_str(), opts).map_err(|_| "E:export.write")?;
            zip.write_all(&b.data).map_err(|_| "E:export.write")?;
        }
        zip.start_file("mkss.json", opts).map_err(|_| "E:export.write")?;
        zip.write_all(&serde_json::to_vec_pretty(m).map_err(|_| "E:export.write")?).map_err(|_| "E:export.write")?;
        zip.start_file("README.txt", opts).map_err(|_| "E:export.write")?;
        zip.write_all(readme(m).as_bytes()).map_err(|_| "E:export.write")?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitised() {
        assert_eq!(clean_name("  Fire / Mario: v2?  "), "Fire Mario v2");
        assert_eq!(clean_name("../.."), "My skin pack");
        assert_eq!(pack_file_name("Blue: team"), "Blue team.zip");
    }
}
