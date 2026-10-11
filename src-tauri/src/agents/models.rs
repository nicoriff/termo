//! The models an agent can run on: the ones each CLI itself offers, read
//! from the catalogs they keep on disk and refresh from their own servers —
//! Claude Code's `cache/model-catalog/*.json` in its config folder, Codex's
//! `models_cache.json` in its home. Nothing is fetched here: whatever the
//! CLI last saw for this account is the list.

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::spec::Engine;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelOption {
    /// What goes to `--model` / `-m`.
    pub id: String,
    pub name: String,
    pub description: String,
    /// One of the models the CLI puts first; the rest are older or niche.
    pub main: bool,
    /// A remark the CLI shows next to it ("Requires usage credits").
    pub note: String,
}

/// Claude Code's config folder: `$CLAUDE_CONFIG_DIR`, or `~/.claude`.
pub(crate) fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".claude")))
}

/// Codex's home: `$CODEX_HOME`, or `~/.codex`.
pub(crate) fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
}

fn text(value: &serde_json::Value, key: &str) -> String {
    value.get(key).and_then(|text| text.as_str()).unwrap_or_default().trim().to_string()
}

/// Claude Code's catalog file: `{catalog: {config: {models: [...]}}}`, main
/// section first as it lists them.
pub fn parse_claude(text_json: &str) -> Vec<ModelOption> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text_json) else { return Vec::new() };
    let models = value
        .pointer("/catalog/config/models")
        .and_then(|models| models.as_array())
        .cloned()
        .unwrap_or_default();
    let mut options: Vec<ModelOption> = models
        .iter()
        .filter(|model| !text(model, "id").is_empty())
        .map(|model| ModelOption {
            id: text(model, "id"),
            name: Some(text(model, "name")).filter(|name| !name.is_empty()).unwrap_or_else(|| text(model, "id")),
            description: text(model, "description"),
            main: text(model, "section") == "main",
            note: model.pointer("/badge/message").and_then(|note| note.as_str()).unwrap_or_default().to_string(),
        })
        .collect();
    options.sort_by_key(|option| !option.main);
    options
}

/// Codex's cache: `{models: [...]}`, only the ones it lists (others are
/// hidden, internal ones), by its own priority.
pub fn parse_codex(text_json: &str) -> Vec<ModelOption> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text_json) else { return Vec::new() };
    let mut models: Vec<serde_json::Value> = value
        .get("models")
        .and_then(|models| models.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|model| text(model, "visibility") == "list" && !text(model, "slug").is_empty())
        .collect();
    models.sort_by_key(|model| model.get("priority").and_then(|priority| priority.as_i64()).unwrap_or(i64::MAX));
    models
        .iter()
        .map(|model| ModelOption {
            id: text(model, "slug"),
            name: Some(text(model, "display_name")).filter(|name| !name.is_empty()).unwrap_or_else(|| text(model, "slug")),
            description: text(model, "description"),
            main: true,
            note: String::new(),
        })
        .collect()
}

/// The newest `.json` in a folder.
fn newest_json(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .max_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok())
        .map(|entry| entry.path())
}

/// The models the engine's CLI offers this account; empty when it hasn't
/// saved its list yet (it does on its first run).
pub fn available(engine: Engine) -> Vec<ModelOption> {
    let file = match engine {
        Engine::Claude => claude_dir().and_then(|dir| newest_json(&dir.join("cache").join("model-catalog"))),
        Engine::Codex => codex_home().map(|home| home.join("models_cache.json")),
        Engine::Gemini => return gemini_models(),
        Engine::Copilot => return copilot_models(),
        Engine::Grok => return grok_models(),
    };
    let Some(text) = file.and_then(|file| std::fs::read_to_string(file).ok()) else { return Vec::new() };
    match engine {
        Engine::Claude => parse_claude(&text),
        Engine::Codex => parse_codex(&text),
        Engine::Gemini | Engine::Copilot | Engine::Grok => Vec::new(),
    }
}

/// Gemini CLI saves no list of models: its own aliases, which follow its
/// current models from one version to the next ("pro" is today's Pro).
fn gemini_models() -> Vec<ModelOption> {
    [("auto", true), ("pro", true), ("flash", true), ("flash-lite", false)]
        .into_iter()
        .map(|(id, main)| ModelOption {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            main,
            note: String::new(),
        })
        .collect()
}

/// Copilot CLI saves no list either, and which models it has depends on the
/// account's plan: its own automatic pick is the one that always works.
fn copilot_models() -> Vec<ModelOption> {
    vec![ModelOption {
        id: "auto".into(),
        name: "auto".into(),
        description: String::new(),
        main: true,
        note: String::new(),
    }]
}

/// Grok saves no list either, but tells: `grok models` prints what the
/// account has, its default marked. Empty when it isn't installed or logged
/// in.
fn grok_models() -> Vec<ModelOption> {
    use crate::marketplace::{agent_cli, model::AgentTarget};
    agent_cli::locate(AgentTarget::Grok, None)
        .and_then(|bin| agent_cli::run(&bin, &["models"]).ok())
        .map(|output| parse_grok(&output))
        .unwrap_or_default()
}

/// `grok models`' list: one model a line, after a `*` (the default) or a `-`.
fn parse_grok(output: &str) -> Vec<ModelOption> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let main = line.starts_with("* ");
            let id = line.strip_prefix("* ").or_else(|| line.strip_prefix("- "))?.split_whitespace().next()?;
            Some(ModelOption { id: id.into(), name: id.into(), description: String::new(), main, note: String::new() })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groks_own_listing_gives_its_models_and_which_is_the_default() {
        let output = "You are logged in with grok.com.\n\nDefault model: grok-4.7\n\nAvailable models:\n  * grok-4.7 (default)\n  - grok-4.5\n";
        let models = parse_grok(output);
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["grok-4.7", "grok-4.5"]);
        assert!(models[0].main && !models[1].main);
        assert!(parse_grok("You are not authenticated.").is_empty());
    }

    #[test]
    fn claudes_catalog_puts_the_main_models_first() {
        let text = serde_json::json!({
            "version": 2,
            "catalog": { "config": { "models": [
                { "id": "claude-opus-4-8", "name": "Opus 4.8", "section": "overflow" },
                { "id": "claude-opus-5-5", "name": "Opus 5.5", "description": "For complex work", "section": "main" },
                { "id": "claude-fable-5-1", "name": "Fable 5.1", "section": "main",
                  "badge": { "message": "Requires usage credits" } },
                { "name": "no id" }
            ] } }
        })
        .to_string();
        let models = parse_claude(&text);
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["claude-opus-5-5", "claude-fable-5-1", "claude-opus-4-8"]);
        assert!(models[0].main && !models[2].main);
        assert_eq!(models[0].description, "For complex work");
        assert_eq!(models[1].note, "Requires usage credits");
    }

    #[test]
    fn codex_lists_only_what_it_shows_by_priority() {
        let text = serde_json::json!({ "models": [
            { "slug": "gpt-5.5", "display_name": "GPT-5.5", "visibility": "list", "priority": 12 },
            { "slug": "internal", "display_name": "Internal", "visibility": "hide", "priority": 1 },
            { "slug": "gpt-5.6-terra", "display_name": "GPT-5.6-Terra", "description": "Fast", "visibility": "list", "priority": 7 }
        ] })
        .to_string();
        let models = parse_codex(&text);
        let ids: Vec<&str> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["gpt-5.6-terra", "gpt-5.5"]);
        assert_eq!(models[0].description, "Fast");
    }

    #[test]
    fn a_broken_or_missing_file_is_just_no_list() {
        assert!(parse_claude("not json").is_empty());
        assert!(parse_codex("{}").is_empty());
    }
}
