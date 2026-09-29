//! Agents' secrets — the API tokens their integrations need — kept in the
//! system keychain (Credential Manager on Windows, Keychain on macOS), never
//! in the agent's JSON.
//!
//! An integration's config names each secret as `${NAME}`, and a run hands
//! the values over through the environment, never on a command line: Claude
//! Code expands the placeholders itself, and for Codex the variables and
//! headers are forwarded by name (see spec::build_invocation).

use std::collections::HashMap;

const SERVICE: &str = "termo-agents";

fn entry(agent_id: &str, name: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("{agent_id}/{name}"))
}

pub fn set(agent_id: &str, name: &str, value: &str) -> Result<(), String> {
    entry(agent_id, name)
        .and_then(|entry| entry.set_password(value))
        .map_err(|err| err.to_string())
}

/// The values of the agent's secrets that are stored; a missing one is left
/// out (the integration then fails to authenticate, and says so).
pub fn load(agent_id: &str, names: &[String]) -> HashMap<String, String> {
    names
        .iter()
        .filter_map(|name| {
            let value = entry(agent_id, name).and_then(|entry| entry.get_password()).ok()?;
            Some((name.clone(), value))
        })
        .collect()
}

pub fn delete_all(agent_id: &str, names: &[String]) {
    for name in names {
        if let Ok(entry) = entry(agent_id, name) {
            let _ = entry.delete_credential();
        }
    }
}

/// A secret's name as an environment variable: what a `${…}` placeholder
/// refers to. It goes into the CLI's own environment, so never one of the
/// variables that change how a program starts (a server called `git` would
/// otherwise make `GIT_…`).
pub fn env_name(server: &str, param: &str) -> String {
    let raw = format!("{server}_{param}");
    let mut name: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if super::spec::is_reserved_env(&name) {
        name.insert_str(0, "SECRET_");
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_names_are_valid_variables() {
        assert_eq!(env_name("github", "Authorization"), "GITHUB_AUTHORIZATION");
        assert_eq!(env_name("my-server", "api.key"), "MY_SERVER_API_KEY");
        assert_eq!(env_name("1x", "k"), "_1X_K");
    }

    #[test]
    fn env_names_never_change_how_a_program_starts() {
        assert_eq!(env_name("git", "token"), "SECRET_GIT_TOKEN");
        assert_eq!(env_name("node", "options"), "SECRET_NODE_OPTIONS");
        assert!(!super::super::spec::is_reserved_env(&env_name("python-tools", "key")));
    }
}
