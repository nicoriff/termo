//! The name an MCP server is installed under. A catalog's own id — the
//! registry's reverse-DNS `io.github.owner/repo`, Smithery's `owner/name` —
//! can't be used as it is: Claude Code only takes letters, digits, hyphens
//! and underscores, and refuses anything else. So every server gets a short
//! name made from its id, the same one in every agent, and the same one
//! every time — which is also how what's installed is recognized later.

/// Namespaces that say where a server is hosted, not who made it: they add
/// nothing to the name.
const HOSTS: &[&str] = &["smithery"];
const MAX: usize = 60;

fn clean(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// The name `id` is installed under in every agent: its owner and its own
/// name, in the characters every CLI takes.
///
/// `io.github.acme/files` → `acme-files`, `ai.smithery/acme-files` →
/// `acme-files`, `vercel/grep` → `vercel-grep`, `github` → `github`.
pub fn install_name(id: &str) -> String {
    let id = id.trim().trim_start_matches('@');
    let (namespace, name) = match id.rsplit_once('/') {
        Some((namespace, name)) => (namespace, name),
        None => ("", id),
    };
    // Who made it: the last label of a reverse-DNS namespace.
    let owner = clean(namespace.rsplit('.').next().unwrap_or_default());
    let name = clean(name);
    let owned = !owner.is_empty()
        && !HOSTS.iter().any(|host| owner.eq_ignore_ascii_case(host))
        && !name.to_ascii_lowercase().starts_with(&owner.to_ascii_lowercase());
    let mut full = if owned && !name.is_empty() { format!("{owner}-{name}") } else if name.is_empty() { owner } else { name };
    if full.len() > MAX {
        full.truncate(MAX);
        full = full.trim_matches('-').to_string();
    }
    if full.is_empty() {
        "mcp-server".to_string()
    } else {
        full
    }
}

/// Whether `name` is one every agent's CLI takes as a server's name.
pub fn is_install_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_from_who_made_it_and_what_it_is() {
        assert_eq!(install_name("io.github.acme/files"), "acme-files");
        assert_eq!(install_name("com.acme/mcp"), "acme-mcp");
        assert_eq!(install_name("vercel/grep"), "vercel-grep");
        assert_eq!(install_name("@upstash/context7-mcp"), "upstash-context7-mcp");
        assert_eq!(install_name("github"), "github");
    }

    #[test]
    fn a_host_or_a_repeated_owner_adds_nothing() {
        assert_eq!(install_name("ai.smithery/Hint-Services-obsidian-github-mcp"), "Hint-Services-obsidian-github-mcp");
        assert_eq!(install_name("butterbase/butterbase"), "butterbase");
        assert_eq!(install_name("io.github.exa/exa-search"), "exa-search");
    }

    /// Whatever a catalog calls it, the name is one every CLI takes — the
    /// reason this exists is Claude Code refusing `ai.smithery/x`.
    #[test]
    fn every_name_is_one_the_clis_take() {
        for id in [
            "ai.smithery/x",
            "io.github.a/b.c",
            "weird name/with spaces & símbolos",
            "--flag/--other",
            "../../etc/passwd",
            "",
            "/",
            &"x".repeat(300),
        ] {
            let name = install_name(id);
            assert!(is_install_name(&name), "{id:?} -> {name:?}");
        }
        assert_eq!(install_name("--flag/--other"), "flag-other");
        assert_eq!(install_name(""), "mcp-server");
    }

    #[test]
    fn the_same_id_always_gets_the_same_name() {
        assert_eq!(install_name("vercel/grep"), install_name(" vercel/grep "));
    }
}
