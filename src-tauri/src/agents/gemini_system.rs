//! Gemini CLI reads one "system" settings file, which on a managed computer
//! holds what an administrator enforces. Termo gives its runs and consoles
//! settings of their own through `GEMINI_CLI_SYSTEM_SETTINGS_PATH`, which
//! takes that file's place — so Termo's settings are written on top of the
//! system file's, never instead of it.

use std::path::PathBuf;

use serde_json::Value;

/// The system settings file Gemini CLI would read: the one the environment
/// names, or the platform's.
pub fn system_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GEMINI_CLI_SYSTEM_SETTINGS_PATH") {
        return Some(PathBuf::from(path));
    }
    if cfg!(windows) {
        Some(PathBuf::from(r"C:\ProgramData\gemini-cli\settings.json"))
    } else if cfg!(target_os = "macos") {
        Some(PathBuf::from("/Library/Application Support/GeminiCli/settings.json"))
    } else {
        Some(PathBuf::from("/etc/gemini-cli/settings.json"))
    }
}

/// `ours` on top of `base`: objects merged key by key, lists joined without
/// repeats, and anything else taken from `ours`. What `base` has that `ours`
/// doesn't mention stays as it is.
pub fn merge(base: Value, ours: Value) -> Value {
    match (base, ours) {
        (Value::Object(mut base), Value::Object(ours)) => {
            for (key, value) in ours {
                let merged = match base.remove(&key) {
                    Some(existing) => merge(existing, value),
                    None => value,
                };
                base.insert(key, merged);
            }
            Value::Object(base)
        }
        (Value::Array(mut base), Value::Array(ours)) => {
            for item in ours {
                if !base.contains(&item) {
                    base.push(item);
                }
            }
            Value::Array(base)
        }
        (_, ours) => ours,
    }
}

/// Termo's settings (`ours`, JSON) on top of the system file's, if there is
/// one that reads as JSON.
pub fn with_system(ours: &str) -> String {
    let Ok(ours_value) = serde_json::from_str::<Value>(ours) else { return ours.to_string() };
    let base = system_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object);
    match base {
        Some(base) => merge(base, ours_value).to_string(),
        None => ours.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn what_the_administrator_set_stays() {
        let system = json!({
            "security": { "auth": { "enforcedType": "oauth-personal" } },
            "mcp": { "allowed": ["corp"] },
            "mcpServers": { "corp": { "httpUrl": "https://corp" } },
            "tools": { "exclude": ["run_shell_command"] }
        });
        let ours = json!({
            "mcpServers": { "termo": { "httpUrl": "http://127.0.0.1:1/mcp" } },
            "tools": { "exclude": ["web_fetch"] }
        });
        let merged = merge(system, ours);
        assert_eq!(merged["security"]["auth"]["enforcedType"], "oauth-personal");
        assert_eq!(merged["mcp"]["allowed"], json!(["corp"]));
        assert_eq!(merged["mcpServers"]["corp"]["httpUrl"], "https://corp");
        assert_eq!(merged["mcpServers"]["termo"]["httpUrl"], "http://127.0.0.1:1/mcp");
        assert_eq!(merged["tools"]["exclude"], json!(["run_shell_command", "web_fetch"]));
    }
}
