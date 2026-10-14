//! Rules already in the repository that aren't Termo's: the permissions in
//! Claude Code's shared settings (Grok reads them too) that its record
//! doesn't list. The panel shows them and offers to adopt the ones it can
//! read as a harness rule: the rule joins the harness, and the entry goes
//! from the file, for Termo's own to take its place.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::claude;
use super::model::{Access, Decision, Harness};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEntry {
    /// Relative to the root.
    pub file: String,
    /// The list it's in, dotted: `permissions.deny`.
    pub key: String,
    pub value: String,
    /// The harness rule it reads as; `None` when it's none Termo writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<SuggestedRule>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SuggestedRule {
    Path { pattern: String, access: Access },
    Command { prefix: String, decision: Decision },
}

const LISTS: [(&str, Decision); 3] =
    [("permissions.deny", Decision::Deny), ("permissions.ask", Decision::Ask), ("permissions.allow", Decision::Allow)];

/// A permission rule as a harness rule, when it is one.
pub fn suggest(value: &str, decision: Decision) -> Option<SuggestedRule> {
    let (tool, rest) = value.split_once('(')?;
    let inner = rest.strip_suffix(')')?.trim();
    if inner.is_empty() {
        return None;
    }
    match tool {
        "Bash" | "PowerShell" => {
            let prefix = inner.strip_suffix(":*").or_else(|| inner.strip_suffix(" *")).unwrap_or(inner).trim();
            (!prefix.is_empty() && !prefix.contains('*'))
                .then(|| SuggestedRule::Command { prefix: prefix.to_string(), decision })
        }
        // Only a denial is a protected path.
        "Read" | "Edit" | "Write" if decision == Decision::Deny => {
            let pattern = inner.strip_prefix("**/").or_else(|| inner.strip_prefix("./")).or_else(|| inner.strip_prefix('/'));
            let pattern = pattern.unwrap_or(inner).trim_end_matches('/');
            // `//abs` and `~/` are outside the repository.
            if pattern.is_empty() || inner.starts_with("//") || inner.starts_with('~') {
                return None;
            }
            let access = if tool == "Read" { Access::NoRead } else { Access::NoEdit };
            Some(SuggestedRule::Path { pattern: pattern.to_string(), access })
        }
        _ => None,
    }
}

/// Every permission in Claude Code's shared settings that Termo didn't write.
pub fn find(root: &Path, spec: &Harness) -> Vec<ExternalEntry> {
    let Ok(text) = fs::read_to_string(root.join(claude::FILE)) else { return Vec::new() };
    let Ok(Value::Object(settings)) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
    let owned = spec.managed.get(claude::FILE);
    let mut found = Vec::new();
    for (key, decision) in LISTS {
        let (section, list) = key.split_once('.').unwrap_or((key, key));
        let Some(values) = settings.get(section).and_then(|value| value.get(list)).and_then(Value::as_array) else {
            continue;
        };
        let ours: Vec<&Value> = owned.and_then(|owned| owned.get(key)).map(|list| list.iter().collect()).unwrap_or_default();
        for value in values {
            let Some(text) = value.as_str() else { continue };
            if ours.contains(&value) {
                continue;
            }
            found.push(ExternalEntry {
                file: claude::FILE.to_string(),
                key: key.to_string(),
                value: text.to_string(),
                rule: suggest(text, decision),
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_read_as_rules() {
        assert_eq!(
            suggest("Bash(git push:*)", Decision::Ask),
            Some(SuggestedRule::Command { prefix: "git push".into(), decision: Decision::Ask })
        );
        assert_eq!(
            suggest("PowerShell(rm -rf *)", Decision::Deny),
            Some(SuggestedRule::Command { prefix: "rm -rf".into(), decision: Decision::Deny })
        );
        assert_eq!(
            suggest("Read(./secrets/**)", Decision::Deny),
            Some(SuggestedRule::Path { pattern: "secrets/**".into(), access: Access::NoRead })
        );
        assert_eq!(
            suggest("Edit(**/.env)", Decision::Deny),
            Some(SuggestedRule::Path { pattern: ".env".into(), access: Access::NoEdit })
        );
    }

    #[test]
    fn what_termo_doesnt_write_isnt_a_rule() {
        assert_eq!(suggest("WebFetch(domain:example.com)", Decision::Allow), None);
        assert_eq!(suggest("Read(./x)", Decision::Allow), None);
        assert_eq!(suggest("Read(//etc/passwd)", Decision::Deny), None);
        assert_eq!(suggest("Bash(npm run *:*)", Decision::Allow), None);
        assert_eq!(suggest("Bash", Decision::Deny), None);
    }
}
