use crate::{brres, export, game, skin, texture, u8arc, yaz0};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn game_dir() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var_os("MKSS_TEST_GAME_DIR")?);
    let root = game::resolve_root(&p);
    if root.is_none() {
        eprintln!("MKSS_TEST_GAME_DIR is not an extracted game; skipping");
    }
    root
}

fn scratch(name: &str) -> PathBuf {
    let base = std::env::var_os("MKSS_TEST_SCRATCH").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let d = base.join("mkss-tests").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn copy_game(src: &Path, files: &[String], name: &str) -> PathBuf {
    let dst = scratch(name);
    for rel in files {
        let to = dst.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(src.join(rel), &to).unwrap();
    }
    dst
}

fn blind_tex0(b: &[u8]) -> Vec<usize> {
    (0..b.len().saturating_sub(4)).filter(|&i| &b[i..i + 4] == b"TEX0").collect()
}

fn driver(raw: &[u8]) -> (u8arc::FileEntry, Vec<brres::Tex>) {
    let arc = u8arc::parse(raw).unwrap();
    let f = arc.find(skin::DRIVER_MEMBER).unwrap().clone();
    let texs = brres::textures(&raw[f.offset..f.offset + f.size]).unwrap();
    (f, texs)
}

const MARIO_KART: &str = "Race/Kart/ma_kart-mr.szs";

#[test]
fn real_yaz0_and_u8_round_trip_ma_kart_mr() {
    let Some(src) = game_dir() else { return };
    let dir = copy_game(&src, &[MARIO_KART.into()], "u8");
    let packed = std::fs::read(dir.join(MARIO_KART)).unwrap();
    let raw = yaz0::decode(&packed).unwrap();
    assert_eq!(yaz0::decode(&yaz0::encode(&raw).unwrap()).unwrap(), raw);
    let arc = u8arc::parse(&raw).unwrap();
    let paths: Vec<&str> = arc.files.iter().map(|f| f.path.as_str()).collect();
    println!("U8 files: {paths:?}");
    assert!(paths.contains(&"./driver_model.brres") && paths.contains(&"./kart_model.brres") && paths.contains(&"./IKParam.ikp"));
    let rebuilt = u8arc::rewrite(&raw, &arc);
    if let Some(i) = (0..raw.len().max(rebuilt.len())).find(|&i| raw.get(i) != rebuilt.get(i)) {
        panic!("U8 rewrite differs at {i:#x}: len {} vs {}; header orig {:02x?} new {:02x?}", raw.len(), rebuilt.len(), &raw[..16], &rebuilt[..16]);
    }
    println!("U8 rewrite byte-identical ({} bytes, {} nodes)", raw.len(), arc.nodes.len());
    let mut rels = vec![game::DRIVER_SZS.to_string()];
    rels.extend(mario(&src).files);
    for rel in &rels {
        let raw = skin::read_archive(&src, rel).unwrap();
        assert!(u8arc::rewrite(&raw, &u8arc::parse(&raw).unwrap()) == raw, "{rel}");
    }
    println!("U8 rewrite byte-identical for {} more archives", rels.len());
}

#[test]
fn real_tex0_locator_ma_kart_mr() {
    let Some(src) = game_dir() else { return };
    let raw = skin::read_archive(&src, MARIO_KART).unwrap();
    let (f, texs) = driver(&raw);
    let b = &raw[f.offset..f.offset + f.size];
    let mut names: Vec<&str> = texs.iter().map(|t| t.name.as_str()).collect();
    names.sort();
    let mut expected = vec!["lm_0", "lm_1", "mario_all", "mario_tx_64"];
    let eyes: Vec<String> = (0..8).map(|i| format!("mario_eye.{i}")).collect();
    expected.extend(eyes.iter().map(String::as_str));
    expected.sort();
    assert_eq!(names, expected);
    let mut found: Vec<usize> = texs.iter().map(|t| t.section).collect();
    found.sort();
    assert_eq!(found, blind_tex0(b));
    let all = texs.iter().find(|t| t.name == "mario_all").unwrap();
    assert_eq!((all.width, all.height, all.format, all.images, all.len), (256, 128, 14, 1, 16384));
    assert_eq!(all.data - all.section, 0x40);
    for t in &texs {
        println!("{} {}x{} {} images={} data=+{:#x} len={}", t.name, t.width, t.height, brres::format_name(t.format), t.images, t.data - t.section, t.len);
    }
}

#[test]
fn real_unedited_cmpr_reencode_changes_only_that_texture() {
    let Some(src) = game_dir() else { return };
    let raw = skin::read_archive(&src, MARIO_KART).unwrap();
    let (f, texs) = driver(&raw);
    let t = texs.iter().find(|t| t.name == "mario_all").unwrap().clone();
    let orig = texture::decode(&raw[f.offset + t.data..f.offset + t.data + t.len], t.width, t.height, t.format).unwrap();
    assert_eq!((orig.width, orig.height), (256, 128));
    let mut patched = raw.clone();
    let edits = HashMap::from([("mario_all".to_string(), orig.clone())]);
    let n = skin::patch_archive(&mut patched, skin::DRIVER_MEMBER, &edits, &mut HashMap::new()).unwrap();
    assert_eq!(n, 1);
    assert_eq!(patched.len(), raw.len());
    let (start, end) = (f.offset + t.data, f.offset + t.data + t.len);
    let diffs: Vec<usize> = (0..raw.len()).filter(|&i| raw[i] != patched[i]).collect();
    assert!(diffs.iter().all(|&i| i >= start && i < end), "bytes changed outside mario_all data");
    println!("re-encoded mario_all: {} of {} data bytes differ, all inside [{start:#x},{end:#x})", diffs.len(), t.len);
    let again = texture::decode(&patched[start..end], 256, 128, 14).unwrap();
    let mad = texture::mean_abs_diff(&orig, &again);
    println!("decode->encode->decode mean abs diff: {mad:.3}");
    assert!(mad < 4.0);
    let (_, texs2) = driver(&patched);
    assert_eq!(texs2, texs);
}

fn mario(src: &Path) -> game::Character {
    game::scan(src).unwrap().into_iter().find(|c| c.code == "mr").unwrap()
}

#[test]
fn real_mario_recolor_all_files_and_menu() {
    let Some(src) = game_dir() else { return };
    let ch = mario(&src);
    assert_eq!(ch.files.len(), 48);
    let mut files = ch.files.clone();
    files.push(game::DRIVER_SZS.into());
    let root = copy_game(&src, &files, "mario-game");
    let found = skin::discover(&root, &ch).unwrap();
    assert!(found.textures.iter().all(|t| !t.name.starts_with("lm_")));
    assert_eq!(found.menu_member.as_deref(), Some("mr.brres"));
    let all_info = found.textures.iter().find(|t| t.name == "mario_all").unwrap();
    assert_eq!((all_info.files, all_info.menu), (48, true));
    println!("Mario textures: {:?}", found.textures.iter().map(|t| format!("{} {}x{} {} files={} menu={}", t.name, t.width, t.height, t.format, t.files, t.menu)).collect::<Vec<_>>());

    let orig = found.originals["mario_all"].clone();
    let rc = texture::Recolor { hue: 180.0, saturation: 30.0, brightness: 0.0 };
    let new = texture::recolor(&orig, rc);
    let edits = HashMap::from([("mario_all".to_string(), new.clone())]);
    let t0 = std::time::Instant::now();
    let built = skin::build(&root, &ch, &edits, |_, _, _| {}).unwrap();
    println!("build: {} files in {:?}", built.len(), t0.elapsed());
    assert_eq!(built.len(), 49, "48 race files + Driver.szs");

    for b in &built {
        let before = skin::read_archive(&root, &b.path).unwrap();
        let after = yaz0::decode(&b.data).unwrap();
        assert_eq!(before.len(), after.len());
        let member = if b.path == game::DRIVER_SZS { "mr.brres" } else { skin::DRIVER_MEMBER };
        let arc = u8arc::parse(&after).unwrap();
        let f = arc.find(member).unwrap().clone();
        let texs = brres::textures(&after[f.offset..f.offset + f.size]).unwrap();
        let t = texs.iter().find(|t| t.name == "mario_all").unwrap();
        let (s, e) = (f.offset + t.data, f.offset + t.data + t.len);
        assert!((0..after.len()).filter(|&i| before[i] != after[i]).all(|i| i >= s && i < e), "{}", b.path);
        let got = texture::decode(&after[s..e], t.width, t.height, t.format).unwrap();
        let want = texture::resize(&new, t.width, t.height);
        let was = texture::decode(&before[s..e], t.width, t.height, t.format).unwrap();
        let (d_new, d_old) = (texture::mean_abs_diff(&got, &want), texture::mean_abs_diff(&got, &was));
        assert!(d_new < 6.0 && d_old > 15.0, "{}: new {d_new:.2} old {d_old:.2}", b.path);
        if b.path == game::DRIVER_SZS {
            let arc_before = u8arc::parse(&before).unwrap();
            let mut others = 0;
            for g in &arc_before.files {
                if g.path.trim_start_matches("./") != "mr.brres" {
                    assert_eq!(&before[g.offset..g.offset + g.size], &after[g.offset..g.offset + g.size], "{}", g.path);
                    others += 1;
                }
            }
            println!("Driver.szs: {others} other members unchanged; mr.brres mario_all new={d_new:.2} old={d_old:.2}");
        }
    }

    let out_dir = scratch("export");
    let records = vec![export::EditRecord {
        texture: "mario_all".into(),
        width: orig.width,
        height: orig.height,
        format: "CMPR".into(),
        kind: "layers".into(),
        summary: export::EditSummary { layers: 1, adjustments: 1, base_edited: false, sources: vec![], ..Default::default() },
    }];
    let m = export::manifest("Hue 180", vec![export::CharacterRecord { code: "mr".into(), name: "Mario".into(), edits: records }], &built);
    let out = out_dir.join(export::pack_file_name("Mario Hue 180"));
    let size = export::write_zip(&out, &m, &built).unwrap();
    let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
    let names: Vec<String> = (0..z.len()).map(|i| z.by_index(i).unwrap().name().unwrap().to_string()).collect();
    assert_eq!(names.len(), 51);
    assert!(names.iter().filter(|n| n.starts_with("Race/Kart/") && n.ends_with(".szs")).count() == 48);
    assert!(names.contains(&"Scene/Model/Driver.szs".to_string()) && names.contains(&"mkss.json".to_string()) && names.contains(&"README.txt".to_string()));
    let manifest: export::Manifest = serde_json::from_reader(z.by_name("mkss.json").unwrap()).unwrap();
    assert_eq!(manifest.files.len(), 49);
    println!("EXPORT_ZIP {} bytes={size} entries={}", out.display(), names.len());
    let drv = built.iter().find(|b| b.path == game::DRIVER_SZS).unwrap();
    let after = yaz0::decode(&drv.data).unwrap();
    let arc = u8arc::parse(&after).unwrap();
    let f = arc.find("mr.brres").unwrap();
    let t = brres::textures(&after[f.offset..f.offset + f.size]).unwrap().into_iter().find(|t| t.name == "mario_all").unwrap();
    let img = texture::decode(&after[f.offset + t.data..f.offset + t.data + t.len], t.width, t.height, t.format).unwrap();
    std::fs::write(out_dir.join("mario_all-after.png"), texture::png(&img).unwrap()).unwrap();
    std::fs::write(out_dir.join("mario_all-before.png"), texture::png(&orig).unwrap()).unwrap();
}

#[test]
fn real_every_character_discovers_textures() {
    let Some(src) = game_dir() else { return };
    let chars = game::scan(&src).unwrap();
    assert_eq!(chars.len(), 24);
    for ch in &chars {
        let found = skin::discover(&src, ch).unwrap();
        let names: Vec<String> = found.textures.iter().map(|t| format!("{}{}", t.name, if t.menu { "*" } else { "" })).collect();
        println!("{} ({}) files={} menu={:?}: {}", ch.name, ch.code, ch.files.len(), found.menu_member, names.join(", "));
        assert!(found.menu_member.is_some(), "{}", ch.code);
    }
}

#[test]
fn real_disc_extracts_only_needed_files() {
    let Some(image) = std::env::var_os("MKSS_TEST_IMAGE").map(PathBuf::from) else { return };
    let app = scratch("disc-app");
    let t0 = std::time::Instant::now();
    let root = crate::disc::extract(&image, &app).unwrap();
    let kart = std::fs::read_dir(root.join("Race/Kart")).unwrap().count();
    println!("disc extract: {} in {:?}, Race/Kart files={kart}", root.display(), t0.elapsed());
    assert_eq!(kart, 1528);
    assert!(root.join(game::DRIVER_SZS).is_file());
    let t1 = std::time::Instant::now();
    assert_eq!(crate::disc::extract(&image, &app).unwrap(), root);
    assert!(t1.elapsed().as_secs() < 2);
}

#[test]
fn real_models_geometry_and_parts() {
    let Some(src) = game_dir() else { return };
    let chars = game::scan(&src).unwrap();
    let mut report = Vec::new();
    for code in ["mr", "bmr", "kp", "rs", "pc", "ds", "kk", "bk", "fk", "ka", "kt"] {
        let ch = chars.iter().find(|c| c.code == code).unwrap();
        let found = skin::discover(&src, ch).unwrap();
        let sizes: Vec<(String, u32, u32)> = found.textures.iter().map(|t| (t.name.clone(), t.width, t.height)).collect();
        let t0 = std::time::Instant::now();
        let sources = crate::model::load_sources(&src, ch).unwrap();
        let computed = crate::parts::compute(&sources, &sizes);
        let ms = t0.elapsed().as_millis();
        let mut tris = 0;
        for (mi, s) in sources.iter().enumerate() {
            assert!(!s.model.submeshes.is_empty(), "{code}: model without submeshes");
            let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
            for (si, sm) in s.model.submeshes.iter().enumerate() {
                let nv = sm.positions.len() / 3;
                assert!(sm.triangles() > 0);
                assert_eq!(sm.uvs.len(), nv * 2);
                assert_eq!(sm.normals.len(), nv * 3);
                assert!(sm.indices.iter().all(|&i| (i as usize) < nv), "{code}: index out of range");
                assert!(sm.uvs.iter().all(|u| u.is_finite() && u.abs() < 64.0), "{code}: bad uv");
                for p in sm.positions.chunks_exact(3) {
                    for a in 0..3 {
                        assert!(p[a].is_finite());
                        lo[a] = lo[a].min(p[a]);
                        hi[a] = hi[a].max(p[a]);
                    }
                }
                tris += sm.triangles();
                if sm.texture(&s.model).is_some() {
                    let assigned = computed.tri_parts[mi][si].iter().filter(|&&p| p != u16::MAX).count();
                    assert!(assigned * 100 >= sm.triangles() * 95, "{code} {}: {assigned}/{} triangles in parts", sm.material, sm.triangles());
                }
            }
            let span: Vec<f32> = (0..3).map(|a| hi[a] - lo[a]).collect();
            assert!(span.iter().all(|&d| d > 1.0 && d < 2000.0), "{code}: bbox {lo:?}..{hi:?}");
        }
        let mut unmapped = Vec::new();
        for t in &computed.textures {
            if t.model.is_none() {
                unmapped.push(t.texture.clone());
                continue;
            }
            assert!(!t.parts.is_empty(), "{code} {}: no parts", t.texture);
            for p in &t.parts {
                assert!(p.pixels > 0 && p.runs.chunks(2).map(|r| r[1]).sum::<u32>() == p.pixels);
                assert!(p.runs.chunks(2).all(|r| r[0] + r[1] <= t.width * t.height));
            }
        }
        let labels: Vec<String> = computed.textures.iter().filter(|t| t.alias_of.is_none() && !t.parts.is_empty()).map(|t| format!("{}: {}", t.texture, t.parts.iter().map(|p| if p.n > 0 { format!("{}{}", p.key, p.n) } else { p.key.clone() }).collect::<Vec<_>>().join(","))).collect();
        report.push(format!(
            "{code}: {} models ({}), {tris} tris, {} textures, {} unmapped {:?}, {ms} ms\n    {}",
            sources.len(),
            sources.iter().map(|s| format!("{}:{}", s.kind, s.model.name)).collect::<Vec<_>>().join(" "),
            computed.textures.len(),
            unmapped.len(),
            unmapped,
            labels.join("\n    ")
        ));
    }
    let raw = skin::read_archive(&src, game::DRIVER_SZS).unwrap();
    let arc = u8arc::parse(&raw).unwrap();
    let m = game::menu_member(&arc, "rs").unwrap();
    let f = arc.find(&m).unwrap();
    let menu = crate::mdl0::parse_all(&raw[f.offset..f.offset + f.size]).unwrap();
    let menu_tris: usize = menu.iter().flat_map(|m| m.submeshes.iter()).map(|s| s.triangles()).sum();
    assert!(menu_tris > 500);
    report.push(format!("rs menu ({m}): {} models, {menu_tris} tris", menu.len()));
    eprintln!("{}", report.join("\n"));
}

#[test]
fn real_kart_bike_outfits_from_data() {
    let Some(src) = game_dir() else { return };
    let mut with_bike = Vec::new();
    for ch in game::scan(&src).unwrap() {
        let sources = crate::model::load_sources(&src, &ch).unwrap();
        let has = |v: &str, d: &str| sources.iter().any(|s| crate::model::vehicle_of(s.kind, &s.file) == Some(v) && crate::model::detail_of(&s.model.name) == d);
        assert!(has("kart", "full") && has("kart", "low"), "{}: kart full + low models", ch.code);
        let bike = crate::model::has_bike_outfit(&sources);
        if bike {
            assert!(has("bike", "low"), "{}: bike outfit without its own low-detail model", ch.code);
            let texs = |v: &str| -> std::collections::BTreeSet<String> {
                sources
                    .iter()
                    .filter(|s| crate::model::vehicle_of(s.kind, &s.file) == Some(v))
                    .flat_map(|s| s.model.submeshes.iter().filter_map(|m| m.texture(&s.model).map(|t| t.texture.clone())).collect::<Vec<_>>())
                    .collect()
            };
            let (k, b) = (texs("kart"), texs("bike"));
            assert!(b.iter().any(|t| !k.contains(t)), "{}: bike textures {b:?} vs kart {k:?}", ch.code);
            eprintln!("{}: kart {k:?} | bike {b:?}", ch.code);
            with_bike.push(ch.code.clone());
        }
        eprintln!(
            "{}: {}",
            ch.code,
            sources.iter().map(|s| format!("{}/{}/{}", crate::model::vehicle_of(s.kind, &s.file).unwrap_or("menu"), crate::model::detail_of(&s.model.name), s.model.name)).collect::<Vec<_>>().join(" ")
        );
    }
    with_bike.sort();
    eprintln!("characters with a separate bike outfit: {with_bike:?}");
    assert_eq!(with_bike, vec!["ds".to_string(), "pc".into(), "rs".into()], "docs/TEXTURE-MAP.md: Peach, Daisy, Rosalina");
}

#[test]
fn real_all_characters_models_parse() {
    let Some(src) = game_dir() else { return };
    let mut n = 0;
    for ch in game::scan(&src).unwrap() {
        let sources = crate::model::load_sources(&src, &ch).unwrap_or_else(|e| panic!("{}: {e}", ch.code));
        assert!(!sources.is_empty());
        n += sources.len();
    }
    eprintln!("24 characters: {n} distinct models parsed");
}

#[test]
fn real_eye_frames_alias_and_blink_for_all_characters() {
    let Some(src) = game_dir() else { return };
    for ch in game::scan(&src).unwrap() {
        let found = skin::discover(&src, &ch).unwrap();
        let sizes: Vec<(String, u32, u32)> = found.textures.iter().map(|t| (t.name.clone(), t.width, t.height)).collect();
        let sources = crate::model::load_sources(&src, &ch).unwrap();
        let map = crate::texmap::analyze(&src, &ch, &sizes).ok();
        let b = crate::model::bundle_with(&ch.code, &sources, &sizes, map.as_ref());
        for e in &b.eyes {
            let alias = |n: &str| b.textures.iter().find(|t| t.texture == n).and_then(|t| t.alias_of.clone());
            let distinct: std::collections::BTreeSet<Vec<u8>> = e.blink.iter().map(|n| found.originals[n].pixels.clone()).collect();
            eprintln!(
                "{} {}: frames {:?} default {} blink {:?} aliases {:?} distinct-images {}",
                ch.code,
                ch.name,
                e.frames,
                e.default_frame,
                e.blink,
                e.frames.iter().map(|f| alias(f)).collect::<Vec<_>>(),
                distinct.len()
            );
            for f in &e.frames {
                assert!(f == &e.default_frame || alias(f).as_deref() == Some(e.default_frame.as_str()), "{}: {f} not shown through {}", ch.code, e.default_frame);
            }
            assert!(distinct.len() >= 2, "{}: blink {:?} shows a single image", ch.code, e.blink);
        }
    }
}

#[test]
fn real_pack_merges_driver_szs_for_three_characters() {
    let Some(src) = game_dir() else { return };
    let all = game::scan(&src).unwrap();
    let mut chars = Vec::new();
    let mut members = HashMap::new();
    let mut hues = HashMap::new();
    let driver_before = skin::read_archive(&src, game::DRIVER_SZS).unwrap();
    let arc_before = u8arc::parse(&driver_before).unwrap();
    for (code, hue) in [("mr", 120.0f32), ("pc", 200.0), ("fk", 300.0)] {
        let ch = all.iter().find(|c| c.code == code).unwrap().clone();
        let found = skin::discover(&src, &ch).unwrap();
        let tex = found.textures.iter().find(|t| !t.eye && t.menu).unwrap().name.clone();
        let img = texture::recolor(&found.originals[&tex], texture::Recolor { hue, saturation: 40.0, brightness: 0.0 });
        members.insert(code, (game::menu_member(&arc_before, code).unwrap(), tex.clone()));
        hues.insert(code, img.clone());
        chars.push((ch, HashMap::from([(tex, img)])));
    }
    println!("pack members: {:?}", members.iter().map(|(c, (m, t))| format!("{c}:{m}/{t}")).collect::<Vec<_>>());
    let t0 = std::time::Instant::now();
    let built = skin::build_pack(&src, &chars, |_, _, _| {}).unwrap();
    println!("build_pack: {} files in {:?}", built.len(), t0.elapsed());
    let mut expected = Vec::new();
    for (ch, edits) in &chars {
        for f in &ch.files {
            let raw = skin::read_archive(&src, f).unwrap();
            if driver(&raw).1.iter().any(|t| edits.contains_key(&t.name)) {
                expected.push(f.clone());
            }
        }
        println!("{}: {} of {} race files carry the edit", ch.code, expected.iter().filter(|f| ch.files.contains(f)).count(), ch.files.len());
    }
    let race: Vec<&skin::BuiltFile> = built.iter().filter(|b| b.path.starts_with("Race/Kart/")).collect();
    let mut got: Vec<String> = race.iter().map(|b| b.path.clone()).collect();
    got.sort();
    expected.sort();
    assert_eq!(got, expected, "race files of the three characters that contain the edit");
    for (ch, _) in &chars {
        assert!(race.iter().any(|b| ch.files.contains(&b.path)), "{}: no race file", ch.code);
    }
    let drivers: Vec<&skin::BuiltFile> = built.iter().filter(|b| b.path == game::DRIVER_SZS).collect();
    assert_eq!(drivers.len(), 1, "a single merged Driver.szs");
    let after = yaz0::decode(&drivers[0].data).unwrap();
    assert_eq!(after.len(), driver_before.len());
    let arc_after = u8arc::parse(&after).unwrap();
    let mut unchanged = 0;
    for g in &arc_before.files {
        let name = g.path.trim_start_matches("./");
        let a = &after[g.offset..g.offset + g.size];
        let b = &driver_before[g.offset..g.offset + g.size];
        match members.iter().find(|(_, (m, _))| m == name) {
            None => {
                if let Some(i) = (0..a.len()).find(|&i| a[i] != b[i]) {
                    panic!("Driver.szs member {name} changed at +{i:#x}");
                }
                unchanged += 1;
            }
            Some((code, (_, tex))) => {
                let f = arc_after.find(name).unwrap();
                let texs = brres::textures(&after[f.offset..f.offset + f.size]).unwrap();
                let t = texs.iter().find(|t| &t.name == tex).unwrap();
                let (s0, e0) = (t.data, t.data + t.len);
                if let Some(i) = (0..a.len()).find(|&i| a[i] != b[i] && (i < s0 || i >= e0)) {
                    panic!("{name}: byte +{i:#x} outside {tex} changed");
                }
                let got = texture::decode(&a[s0..e0], t.width, t.height, t.format).unwrap();
                let was = texture::decode(&b[s0..e0], t.width, t.height, t.format).unwrap();
                let want = texture::resize(&hues[code], t.width, t.height);
                let (d_new, d_old) = (texture::mean_abs_diff(&got, &want), texture::mean_abs_diff(&got, &was));
                println!("Driver.szs {name} ({code}) {tex}: err to edit {d_new:.2}, to original {d_old:.2}");
                assert!(d_new < 8.0 && d_old > d_new * 2.0, "{name}: edit not carried (new {d_new:.2} old {d_old:.2})");
            }
        }
    }
    println!("Driver.szs: 3 edited menu members, {unchanged} other members byte-identical");
    assert_eq!(unchanged, arc_before.files.len() - 3);
    let records = chars
        .iter()
        .map(|(c, e)| export::CharacterRecord {
            code: c.code.clone(),
            name: c.name.clone(),
            edits: e.keys().map(|n| export::EditRecord { texture: n.clone(), width: 0, height: 0, format: String::new(), kind: "layers".into(), summary: Default::default() }).collect(),
        })
        .collect();
    let m = export::manifest("Team colours", records, &built);
    let out = scratch("pack").join(export::pack_file_name("Team colours"));
    let size = export::write_zip(&out, &m, &built).unwrap();
    let mut z = zip::ZipArchive::new(std::fs::File::open(&out).unwrap()).unwrap();
    assert_eq!(z.len(), built.len() + 2);
    let manifest: export::Manifest = serde_json::from_reader(z.by_name("mkss.json").unwrap()).unwrap();
    assert_eq!(manifest.format, "mkss-skin/2");
    assert_eq!(manifest.characters.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(), ["mr", "pc", "fk"]);
    assert_eq!(manifest.files.iter().filter(|f| f.path == game::DRIVER_SZS).count(), 1);
    println!("PACK_ZIP {} bytes={size} entries={}", out.display(), z.len());
}

#[test]
fn open_file_reopens_instead_of_duplicating() {
    use crate::project;
    let d = scratch("open-file-dedupe");
    let lib = d.join("library");
    let a = project::create(&lib, "Pack A", &project::GameRef::default()).unwrap();
    let out = d.join("Pack A.mkss");
    project::export_file(&lib, &a.id, &out).unwrap();
    let (m0, e0) = project::open_file(&lib, &lib.join(&a.id).join(project::DATA), "x").unwrap();
    assert!(e0 && m0.id == a.id);
    let (m1, e1) = project::open_file(&lib, &out, "Pack A").unwrap();
    assert!(!e1 && m1.id != a.id && !m1.imported_sha.is_empty());
    let (m2, e2) = project::open_file(&lib, &out, "Pack A").unwrap();
    assert!(e2 && m2.id == m1.id);
    assert_eq!(project::list(&lib).len(), 2);
    let dup = project::duplicate(&lib, &m1.id, "Copy").unwrap();
    assert!(dup.imported_sha.is_empty());
    let (m3, _) = project::open_file(&lib, &out, "Pack A").unwrap();
    assert_eq!(m3.id, m1.id);
    let args = crate::mkss_args(vec!["--flag".into(), "Pack A.mkss".into(), "missing.mkss".into(), "x.zip".into()], &d);
    assert_eq!(args, vec![d.join("Pack A.mkss").to_string_lossy().into_owned()]);
}
