//! From the harness to the files the agents read: what each one would look
//! like (`plan`, the preview and the "out of sync" check) and writing it
//! (`apply`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::model::{self, Harness, Owned, SPEC_FILE};
use super::external::ExternalEntry;
use super::{checks, claude, codex, copilot, gemini, instructions, json_merge};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// Relative to the root, with forward slashes.
    pub path: String,
    /// `None`: the file isn't there yet.
    pub before: Option<String>,
    /// `None`: the file goes.
    pub after: Option<String>,
}

/// A settings file Termo adds entries to, beside the user's own.
struct JsonTarget {
    file: &'static str,
    applies: fn(&Harness) -> bool,
    desired: fn(&Harness, &Map<String, Value>) -> Owned,
}

const JSON_TARGETS: &[JsonTarget] = &[
    JsonTarget { file: claude::FILE, applies: claude::applies, desired: claude::desired },
    JsonTarget { file: codex::HOOKS_FILE, applies: codex::applies, desired: codex::hooks },
    JsonTarget { file: gemini::FILE, applies: gemini::applies, desired: gemini::desired },
];

/// A file that's Termo's alone, named so: written whole, or taken away.
const OWN_FILES: &[(&str, fn(&Harness) -> Option<String>)] = &[
    (codex::RULES_FILE, codex::rules),
    (copilot::FILE, copilot::hooks),
    (checks::SCRIPT_FILE, checks::script_for),
];

fn read(root: &Path, file: &str) -> Option<String> {
    fs::read_to_string(root.join(file)).ok()
}

/// Every file that would change for `spec` to hold, as it is and as it
/// would be. What Termo added before is known from the harness on disk;
/// `adopted` are the user's entries taken out with it, their rules now the
/// harness's (see external.rs).
pub fn plan(root: &Path, spec: &Harness, adopted: &[ExternalEntry]) -> anyhow::Result<Vec<FileChange>> {
    let mut previous = model::load(root)?.map(|spec| spec.managed).unwrap_or_default();
    for entry in adopted {
        previous
            .entry(entry.file.clone())
            .or_default()
            .entry(entry.key.clone())
            .or_default()
            .push(Value::String(entry.value.clone()));
    }
    let spec = spec.normalized();
    let mut managed = BTreeMap::new();
    let mut changes = Vec::new();
    let mut note = |path: &str, before: Option<String>, after: Option<String>| {
        if before != after {
            changes.push(FileChange { path: path.to_string(), before, after });
        }
    };

    for target in JSON_TARGETS {
        let before = read(root, target.file);
        let empty = Owned::new();
        let previous = previous.get(target.file).unwrap_or(&empty);
        let applies = (target.applies)(&spec);
        let (after, added) = json_merge::merge(target.file, before.as_deref(), previous, |current| {
            if applies {
                (target.desired)(&spec, current)
            } else {
                Owned::new()
            }
        })?;
        if !added.is_empty() {
            managed.insert(target.file.to_string(), added);
        }
        note(target.file, before, after);
    }

    for (file, render) in OWN_FILES {
        let before = read(root, file);
        note(file, before, render(&spec));
    }

    let before = read(root, instructions::AGENTS_MD);
    let after = instructions::agents_md(&spec, before.as_deref());
    note(instructions::AGENTS_MD, before, after);
    let before = read(root, instructions::CLAUDE_MD);
    let after = instructions::claude_md(&spec, before.as_deref());
    note(instructions::CLAUDE_MD, before, after);

    let mut saved = spec;
    saved.managed = managed;
    let before = read(root, SPEC_FILE);
    let after = (!saved.is_blank() || !saved.managed.is_empty()).then(|| model::to_text(&saved));
    // The same harness in other words (key order, spacing) needs no write.
    let same = match (&before, &after) {
        (Some(before), Some(_)) => serde_json::from_str::<Harness>(before).is_ok_and(|known| known == saved),
        _ => false,
    };
    if !same {
        note(SPEC_FILE, before, after);
    }
    Ok(changes)
}

/// Writes every change: a file through a temporary one beside it, so it's
/// never left half written; a file that goes takes the folders it leaves
/// empty with it.
pub fn apply(root: &Path, changes: &[FileChange]) -> anyhow::Result<()> {
    for change in changes {
        let file = root.join(&change.path);
        match &change.after {
            Some(text) => {
                if let Some(dir) = file.parent() {
                    fs::create_dir_all(dir)?;
                }
                let tmp = file.with_extension("termo-tmp");
                fs::write(&tmp, text)?;
                fs::rename(&tmp, &file)?;
            }
            None => {
                if file.exists() {
                    fs::remove_file(&file)?;
                }
                let mut dir = file.parent();
                while let Some(folder) = dir.filter(|folder| *folder != root && folder.starts_with(root)) {
                    if fs::remove_dir(folder).is_err() {
                        break;
                    }
                    dir = folder.parent();
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::model::{Access, Check, PathRule};

    fn temp_root() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("termo-harness-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn spec() -> Harness {
        Harness {
            paths: vec![PathRule { pattern: ".env".into(), access: Access::NoRead }],
            checks: vec![Check { command: "npm run check".into(), timeout_sec: None }],
            ..Harness::default()
        }
    }

    #[test]
    fn applying_and_then_clearing_leaves_the_repo_as_it_was() {
        let root = temp_root();
        let settings = "{\n  \"permissions\": {\n    \"deny\": [\"Read(./mine)\"]\n  }\n}\n";
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(root.join(claude::FILE), settings).unwrap();
        fs::write(root.join("CLAUDE.md"), "# Mine\n").unwrap();

        let changes = plan(&root, &spec(), &[]).unwrap();
        let paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                claude::FILE,
                codex::HOOKS_FILE,
                gemini::FILE,
                copilot::FILE,
                checks::SCRIPT_FILE,
                "AGENTS.md",
                "CLAUDE.md",
                SPEC_FILE
            ]
        );
        apply(&root, &changes).unwrap();
        // Applied: nothing left to do.
        assert!(plan(&root, &model::load(&root).unwrap().unwrap(), &[]).unwrap().is_empty());
        let saved = model::load(&root).unwrap().unwrap();
        assert!(!saved.managed[claude::FILE]["permissions.deny"].contains(&serde_json::json!("Read(./mine)")));

        apply(&root, &plan(&root, &Harness::default(), &[]).unwrap()).unwrap();
        // The same settings, reformatted.
        let back: Value = serde_json::from_str(&fs::read_to_string(root.join(claude::FILE)).unwrap()).unwrap();
        assert_eq!(back, serde_json::from_str::<Value>(settings).unwrap());
        assert_eq!(fs::read_to_string(root.join("CLAUDE.md")).unwrap(), "# Mine\n");
        assert!(!root.join("AGENTS.md").exists());
        for folder in [".termo", ".codex", ".gemini", ".github"] {
            assert!(!root.join(folder).exists(), "{folder} is left");
        }
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_adopted_entry_makes_way_for_termos() {
        use crate::harness::external;
        use crate::harness::model::{CommandRule, Decision};
        let root = temp_root();
        fs::create_dir_all(root.join(".claude")).unwrap();
        fs::write(root.join(claude::FILE), r#"{ "permissions": { "ask": ["Bash(git push:*)", "WebFetch"] } }"#).unwrap();
        let found = external::find(&root, &Harness::default());
        assert_eq!(found.len(), 2);
        let adopted: Vec<_> = found.into_iter().filter(|entry| entry.rule.is_some()).collect();
        let spec = Harness {
            agents: vec![model::Agent::Claude],
            commands: vec![CommandRule { prefix: "git push".into(), decision: Decision::Ask }],
            ..Harness::default()
        };
        apply(&root, &plan(&root, &spec, &adopted).unwrap()).unwrap();
        let settings: Value = serde_json::from_str(&fs::read_to_string(root.join(claude::FILE)).unwrap()).unwrap();
        let ask = settings["permissions"]["ask"].as_array().unwrap();
        assert!(!ask.contains(&serde_json::json!("Bash(git push:*)")));
        assert!(ask.contains(&serde_json::json!("WebFetch")));
        assert!(ask.contains(&serde_json::json!("Bash(git push *)")));
        // Termo's now, so not external any more; the one it can't read stays.
        let saved = model::load(&root).unwrap().unwrap();
        let left: Vec<_> = external::find(&root, &saved).into_iter().map(|entry| entry.value).collect();
        assert_eq!(left, ["WebFetch"]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_blank_harness_in_a_bare_repo_writes_nothing() {
        let root = temp_root();
        assert!(plan(&root, &Harness::default(), &[]).unwrap().is_empty());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_hand_edit_to_a_generated_file_shows_as_out_of_sync() {
        let root = temp_root();
        apply(&root, &plan(&root, &spec(), &[]).unwrap()).unwrap();
        fs::write(root.join(claude::FILE), "{}\n").unwrap();
        let changes = plan(&root, &model::load(&root).unwrap().unwrap(), &[]).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, claude::FILE);
        fs::remove_dir_all(&root).unwrap();
    }
}
