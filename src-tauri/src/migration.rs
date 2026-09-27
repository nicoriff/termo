//! One-time move of the user's data from the app's previous name. Termo was
//! Termini, identified as `com.termini.app`; the identifier names the folders
//! Tauri keeps config and data in, so without this the renamed app would start
//! from scratch — settings, saved layouts, recent sessions, Oh My Posh, and the
//! user's own Claude Code status line that Termini had chained onto (see
//! pty/status_line.rs), which would be lost for good.
//!
//! Copies rather than moves: the old folder stays until the user uninstalls
//! Termini, so nothing is lost if anything here goes wrong.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

const OLD_IDENTIFIER: &str = "com.termini.app";
/// Must match `identifier` in tauri.conf.json.
const IDENTIFIER: &str = "com.ln.termo";

/// Runs before the app is built, so nothing has read or created the new
/// folders yet. Failures are ignored: the app then just starts fresh, as it
/// would have without this.
pub fn migrate_from_termini() {
    // Config and data share a folder on Windows and macOS but not on Linux
    // (~/.config vs ~/.local/share), and Tauri uses both.
    let mut bases: Vec<PathBuf> = [dirs::config_dir(), dirs::data_dir()].into_iter().flatten().collect();
    bases.dedup();
    for base in bases {
        let _ = migrate_dir(&base.join(OLD_IDENTIFIER), &base.join(IDENTIFIER));
    }
}

/// Copies `old` to `new` unless there's nothing to copy or `new` already
/// exists — which also makes it run only once. Returns whether it copied.
fn migrate_dir(old: &Path, new: &Path) -> io::Result<bool> {
    if !old.is_dir() || new.exists() {
        return Ok(false);
    }
    // Copied under a temporary name and renamed at the end, so a copy cut
    // short isn't mistaken next time for a finished one.
    let staging = new.with_extension("migrating");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    copy_dir(old, &staging)?;
    rewrite_paths(&staging.join("config.json"), old, new)?;
    fs::rename(&staging, new)?;
    Ok(true)
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The config holds some absolute paths into the old folder (the installed
/// font, for one); they're pointed at the copy so nothing keeps depending on
/// a folder that goes away with Termini.
fn rewrite_paths(config: &Path, old: &Path, new: &Path) -> io::Result<()> {
    let Ok(text) = fs::read_to_string(config) else {
        return Ok(());
    };
    let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
        return Ok(());
    };
    let (old, new) = (old.to_string_lossy(), new.to_string_lossy());
    if replace_prefix(&mut value, &old, &new) {
        fs::write(config, serde_json::to_string_pretty(&value).unwrap_or(text))?;
    }
    Ok(())
}

fn replace_prefix(value: &mut Value, old: &str, new: &str) -> bool {
    match value {
        Value::String(text) if text.starts_with(old) => {
            *text = format!("{new}{}", &text[old.len()..]);
            true
        }
        Value::Array(items) => items.iter_mut().fold(false, |changed, item| replace_prefix(item, old, new) || changed),
        Value::Object(map) => map.values_mut().fold(false, |changed, item| replace_prefix(item, old, new) || changed),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        std::env::temp_dir().join(format!("termo-migration-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn copies_the_old_folder_and_points_config_paths_at_the_copy() {
        let base = scratch();
        let old = base.join(OLD_IDENTIFIER);
        let new = base.join(IDENTIFIER);
        fs::create_dir_all(old.join("fonts")).unwrap();
        fs::write(old.join("fonts").join("f.woff2"), "font").unwrap();
        fs::write(old.join("layouts.json"), "[]").unwrap();
        let font = old.join("fonts").join("f.woff2");
        let config = serde_json::json!({ "installed_font_path": font.to_string_lossy(), "theme": "x" });
        fs::write(old.join("config.json"), config.to_string()).unwrap();

        let copied = migrate_dir(&old, &new).unwrap();
        let migrated: Value = serde_json::from_str(&fs::read_to_string(new.join("config.json")).unwrap()).unwrap();
        let font_moved = new.join("fonts").join("f.woff2").is_file();
        let layouts = new.join("layouts.json").is_file();
        let old_kept = old.join("config.json").is_file();
        let _ = fs::remove_dir_all(&base);

        assert!(copied && font_moved && layouts && old_kept);
        assert_eq!(
            migrated["installed_font_path"].as_str().unwrap(),
            new.join("fonts").join("f.woff2").to_string_lossy()
        );
        assert_eq!(migrated["theme"], "x");
    }

    #[test]
    fn runs_only_once_and_never_over_existing_data() {
        let base = scratch();
        let old = base.join(OLD_IDENTIFIER);
        let new = base.join(IDENTIFIER);
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("config.json"), "{}").unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("config.json"), "{\"mine\":true}").unwrap();

        let copied = migrate_dir(&old, &new).unwrap();
        let kept = fs::read_to_string(new.join("config.json")).unwrap();
        let _ = fs::remove_dir_all(&base);

        assert!(!copied);
        assert_eq!(kept, "{\"mine\":true}");
    }

    #[test]
    fn does_nothing_without_an_old_folder() {
        let base = scratch();
        let copied = migrate_dir(&base.join(OLD_IDENTIFIER), &base.join(IDENTIFIER)).unwrap();
        let created = base.join(IDENTIFIER).exists();
        let _ = fs::remove_dir_all(&base);
        assert!(!copied && !created);
    }

    #[test]
    fn matches_the_identifier_in_tauri_conf() {
        let conf: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], IDENTIFIER);
    }
}
