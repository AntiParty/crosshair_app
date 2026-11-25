//! Files: the game's settings file, our presets and backups, and the guarded apply / restore steps.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::guard;
use crate::gvas::SaveFile;
use crate::model::Crosshair;

/// The `Version` the options file had when this app was verified against it.
pub const KNOWN_SAVE_VERSION: i32 = 8;
const SAVE_NAME: &str = "EmbarkOptionSaveGame.sav";

pub fn save_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_default();
    base.join("Discovery")
        .join("Saved")
        .join("SaveGames")
        .join(SAVE_NAME)
}

pub fn app_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("FinalsCrosshair")
}

pub fn presets_dir() -> PathBuf {
    app_dir().join("presets")
}

pub fn backups_dir() -> PathBuf {
    app_dir().join("backups")
}

/// "20261125-094501" in UTC, without pulling in a date crate.
pub fn stamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn safe_file_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().to_string();
    if s.is_empty() { "preset".into() } else { s }
}

// ── presets ─────────────────────────────────────────────────────────────────

pub fn list_presets() -> Vec<Crosshair> {
    let mut out: Vec<Crosshair> = fs::read_dir(presets_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|t| Crosshair::from_share_code(&t).ok())
        .collect();
    out.sort_by_key(|c| c.name.to_lowercase());
    out
}

pub fn save_preset(c: &Crosshair) -> Result<(), String> {
    fs::create_dir_all(presets_dir()).map_err(|e| e.to_string())?;
    let path = presets_dir().join(format!("{}.json", safe_file_name(&c.name)));
    fs::write(path, c.to_share_code()).map_err(|e| e.to_string())
}

pub fn delete_preset(name: &str) {
    let _ = fs::remove_file(presets_dir().join(format!("{}.json", safe_file_name(name))));
}

// ── reading the game's current crosshair ────────────────────────────────────

pub fn read_current(path: &Path) -> Result<Crosshair, String> {
    let bytes = fs::read(path).map_err(|_| {
        "Couldn't find the game's settings file. Start the game once first.".to_string()
    })?;
    let f = SaveFile::parse(&bytes).map_err(|e| format!("Couldn't read the settings file: {e}"))?;
    Ok(Crosshair::from_settings("Current in game", f.iter()))
}

// ── backups ─────────────────────────────────────────────────────────────────

pub struct Backup {
    pub path: PathBuf,
    pub label: String,
}

pub fn list_backups() -> Vec<Backup> {
    let mut v: Vec<Backup> = fs::read_dir(backups_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "sav").unwrap_or(false))
        .map(|e| {
            let label = e
                .file_name()
                .to_string_lossy()
                .trim_end_matches(".sav")
                .to_string();
            Backup {
                path: e.path(),
                label,
            }
        })
        .collect();
    v.sort_by(|a, b| b.label.cmp(&a.label));
    v
}

fn make_backup(dir: &Path, bytes: &[u8]) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Couldn't create the backup folder: {e}"))?;
    let original = dir.join("original.sav");
    if !original.exists() {
        fs::write(&original, bytes).map_err(|e| format!("Couldn't write the backup: {e}"))?;
    }
    let p = dir.join(format!("{}.sav", stamp(now())));
    fs::write(&p, bytes).map_err(|e| format!("Couldn't write the backup: {e}"))?;
    Ok(p)
}

// ── apply ───────────────────────────────────────────────────────────────────

pub struct Applied {
    pub changed: usize,
    pub backup: PathBuf,
}

/// After editing, every setting must equal its original except the ones we meant to change.
fn verify(orig: &SaveFile, edited: &SaveFile, wanted: &Crosshair) -> Result<(), String> {
    if orig.len() != edited.len() {
        return Err("The number of settings changed".into());
    }
    for (k, v) in orig.iter() {
        let now = edited.get(k).ok_or_else(|| format!("{k} went missing"))?;
        let expect = match wanted.values.get(k) {
            Some(w) => w.as_str(),
            None => v,
        };
        if now != expect && now != v {
            return Err(format!("{k} has an unexpected value"));
        }
    }
    Ok(())
}

/// Writes `wanted` into the game's settings file. Refuses unless the game is closed, the file's layout is the
/// one this app was verified against and an untouched round-trip reproduces it exactly. Backs up first,
/// verifies the result by re-reading it, then replaces the file.
pub fn apply(path: &Path, wanted: &Crosshair) -> Result<Applied, String> {
    apply_inner(path, wanted, &backups_dir(), true)
}

/// `require_closed = false` skips the game checks (used by tests, which run on copies in a temp folder).
fn apply_inner(
    path: &Path,
    wanted: &Crosshair,
    backups: &Path,
    require_closed: bool,
) -> Result<Applied, String> {
    let clear = |path: &Path| -> Result<(), String> {
        if require_closed {
            guard::check_clear(path).map_err(|b| b.message())
        } else {
            Ok(())
        }
    };
    clear(path)?;
    let bytes =
        fs::read(path).map_err(|_| "Couldn't read the game's settings file.".to_string())?;
    let orig =
        SaveFile::parse(&bytes).map_err(|e| format!("Couldn't read the settings file: {e}"))?;
    if !orig.class_name.ends_with("EmbarkOptionSaveGameV1") {
        return Err(format!(
            "This isn't the options save (it is '{}'), so nothing was written.",
            orig.class_name
        ));
    }
    if orig.version != Some(KNOWN_SAVE_VERSION) {
        return Err(format!(
            "The game's settings file is version {:?}, not {KNOWN_SAVE_VERSION}. A game update changed it, so nothing was written.",
            orig.version
        ));
    }
    if orig.to_bytes() != bytes {
        return Err("This file's layout isn't recognised exactly, so nothing was written.".into());
    }
    let backup = make_backup(backups, &bytes)?;

    let mut edited = SaveFile::parse(&bytes).map_err(|e| e.to_string())?;
    let mut changed = 0;
    for (k, v) in &wanted.values {
        if let Some(cur) = edited.get(k) {
            if cur != v {
                changed += 1;
            }
            edited.set(k, v);
        }
    }
    let out = edited.to_bytes();
    let reread = SaveFile::parse(&out).map_err(|e| {
        format!("The edited file didn't pass its check ({e}); nothing was written.")
    })?;
    verify(&orig, &reread, wanted).map_err(|e| {
        format!("The edited file didn't pass its check ({e}); nothing was written.")
    })?;

    clear(path)?; // the game could have started meanwhile
    let tmp = path.with_extension("sav.tmp");
    fs::write(&tmp, &out).map_err(|e| format!("Couldn't write: {e}"))?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Couldn't replace the settings file: {e}"));
    }
    Ok(Applied { changed, backup })
}

pub fn restore(path: &Path, backup: &Path) -> Result<(), String> {
    guard::check_clear(path).map_err(|b| b.message())?;
    let bytes = fs::read(backup).map_err(|e| format!("Couldn't read the backup: {e}"))?;
    SaveFile::parse(&bytes).map_err(|_| "That backup isn't a valid settings file.".to_string())?;
    let tmp = path.with_extension("sav.tmp");
    fs::write(&tmp, &bytes).map_err(|e| format!("Couldn't write: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Couldn't replace the settings file: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_are_correct_utc() {
        assert_eq!(stamp(0), "19700101-000000");
        assert_eq!(stamp(1_000_000_000), "20010909-014640");
        assert_eq!(stamp(1_764_064_245), "20251125-095045");
    }

    #[test]
    fn preset_file_names_are_safe() {
        assert_eq!(safe_file_name("../evil/name"), "___evil_name");
        assert_eq!(safe_file_name("  "), "preset");
        assert_eq!(safe_file_name("Ranked dot"), "Ranked dot");
    }

    fn temp_copy(tag: &str) -> Option<(PathBuf, PathBuf, Vec<u8>)> {
        let bytes = fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/opt.sav"
        ))
        .ok()?;
        let dir = std::env::temp_dir().join(format!("finals_{tag}_{}", now()));
        fs::create_dir_all(&dir).unwrap();
        let copy = dir.join(SAVE_NAME);
        fs::write(&copy, &bytes).unwrap();
        Some((dir, copy, bytes))
    }

    #[test]
    fn apply_to_a_copy_changes_only_what_it_should() {
        let Some((dir, copy, bytes)) = temp_copy("apply") else {
            return;
        };
        let mut want = Crosshair::defaults();
        want.set_float("GameplayOption.Crosshair.Center.Radius", 6.0);
        want.set_rgb("GameplayOption.Crosshair.Center.Color", [1, 2, 3]);
        want.values
            .retain(|k, _| k.starts_with("GameplayOption.Crosshair.Center."));

        let r = apply_inner(&copy, &want, &dir.join("bk"), false).expect("apply on a copy");
        assert!(r.changed >= 1);
        assert_eq!(
            fs::read(&r.backup).unwrap(),
            bytes,
            "the backup is the untouched original"
        );
        assert_eq!(
            fs::read(dir.join("bk").join("original.sav")).unwrap(),
            bytes
        );
        let after = fs::read(&copy).unwrap();
        let (a, b) = (
            SaveFile::parse(&bytes).unwrap(),
            SaveFile::parse(&after).unwrap(),
        );
        assert_eq!(b.get("GameplayOption.Crosshair.Center.Radius"), Some("6.0"));
        assert_eq!(
            b.get("GameplayOption.Crosshair.Center.Color.Red"),
            Some("1.0")
        );
        for (k, v) in a.iter() {
            if !k.starts_with("GameplayOption.Crosshair.Center.") {
                assert_eq!(b.get(k), Some(v), "{k} must be untouched");
            }
        }
        // restoring puts the original back
        restore_for_test(&copy, &r.backup);
        assert_eq!(fs::read(&copy).unwrap(), bytes);
        let _ = fs::remove_dir_all(&dir);
    }

    fn restore_for_test(path: &Path, backup: &Path) {
        fs::copy(backup, path).unwrap();
    }

    #[test]
    fn a_different_save_version_is_refused_and_nothing_is_written() {
        let Some((dir, copy, bytes)) = temp_copy("version") else {
            return;
        };
        // flip the Version property (an int 8 right after its tag) to 9
        let mut tampered = bytes.clone();
        let marker = b"Version\0";
        let at = tampered
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap();
        let int_pos = at + marker.len() + 4 + b"IntProperty\0".len() + 4 + 4 + 1;
        assert_eq!(tampered[int_pos], 8);
        tampered[int_pos] = 9;
        fs::write(&copy, &tampered).unwrap();
        let want = Crosshair::defaults();
        let e = apply_inner(&copy, &want, &dir.join("bk"), false)
            .err()
            .expect("must refuse");
        assert!(e.contains("version"), "{e}");
        assert_eq!(fs::read(&copy).unwrap(), tampered, "file untouched");
        assert!(!dir.join("bk").exists(), "no backup made when refusing");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn schema_matches_the_real_file() {
        let Ok(bytes) = fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/opt.sav"
        )) else {
            return;
        };
        let f = SaveFile::parse(&bytes).unwrap();
        let c = Crosshair::defaults();
        for k in c.values.keys() {
            assert!(
                f.get(k).is_some(),
                "the save has no {k}: schema and file disagree"
            );
        }
        let cur = Crosshair::from_settings("Current", f.iter());
        assert!(cur.float("GameplayOption.Crosshair.Center.Radius") >= 1.0);
    }
}
