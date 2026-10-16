//! Termo's entries in someone else's JSON settings file: added to the lists
//! they belong in, and taken out again by the record of what was added (see
//! model::Owned) — the file's other entries are never touched.

use serde_json::{Map, Value};

use super::model::Owned;
use crate::i18n::{tr, Msg};

pub fn parse(file: &str, text: Option<&str>) -> anyhow::Result<Map<String, Value>> {
    let Some(text) = text.filter(|text| !text.trim().is_empty()) else { return Ok(Map::new()) };
    match serde_json::from_str::<Value>(text)? {
        Value::Object(map) => Ok(map),
        _ => anyhow::bail!(tr(Msg::NotAJsonObject(file))),
    }
}

/// Takes out what Termo added last time; a list left empty goes, and so does
/// an object it leaves empty.
pub fn strip(root: &mut Map<String, Value>, owned: &Owned) {
    for (path, values) in owned {
        let keys: Vec<&str> = path.split('.').collect();
        strip_at(root, &keys, values);
    }
}

fn strip_at(map: &mut Map<String, Value>, keys: &[&str], values: &[Value]) {
    let [key, rest @ ..] = keys else { return };
    if rest.is_empty() {
        if let Some(Value::Array(list)) = map.get_mut(*key) {
            for value in values {
                if let Some(at) = list.iter().position(|item| item == value) {
                    list.remove(at);
                }
            }
            if list.is_empty() {
                map.remove(*key);
            }
        }
        return;
    }
    if let Some(Value::Object(child)) = map.get_mut(*key) {
        strip_at(child, rest, values);
        if child.is_empty() {
            map.remove(*key);
        }
    }
}

/// Adds what isn't there yet and returns what it added: an entry the file
/// already had stays the user's. A path that holds something other than a
/// list is left alone.
pub fn add(root: &mut Map<String, Value>, desired: &Owned) -> Owned {
    let mut added = Owned::new();
    for (path, values) in desired {
        let keys: Vec<&str> = path.split('.').collect();
        let Some(list) = list_at(root, &keys) else { continue };
        for value in values {
            if !list.contains(value) {
                list.push(value.clone());
                added.entry(path.clone()).or_default().push(value.clone());
            }
        }
    }
    added
}

fn list_at<'a>(map: &'a mut Map<String, Value>, keys: &[&str]) -> Option<&'a mut Vec<Value>> {
    let [key, rest @ ..] = keys else { return None };
    if rest.is_empty() {
        return match map.entry(key.to_string()).or_insert_with(|| Value::Array(Vec::new())) {
            Value::Array(list) => Some(list),
            _ => None,
        };
    }
    match map.entry(key.to_string()).or_insert_with(|| Value::Object(Map::new())) {
        Value::Object(child) => list_at(child, rest),
        _ => None,
    }
}

/// The file's new text, or `None` when nothing would be left in it. An
/// unchanged file keeps its text as it was, formatting and all.
pub fn render(before: Option<&str>, original: &Map<String, Value>, root: &Map<String, Value>) -> Option<String> {
    if root == original {
        return before.map(str::to_string);
    }
    if root.is_empty() {
        return None;
    }
    let text = serde_json::to_string_pretty(root).unwrap_or_default() + "\n";
    Some(if before.is_some_and(|before| before.contains("\r\n")) { text.replace('\n', "\r\n") } else { text })
}

/// Merges `desired` into the file's text in place of what `previous` says
/// Termo added before. `desired` is worked out from the file as it is
/// without Termo's entries.
pub fn merge(
    file: &str,
    before: Option<&str>,
    previous: &Owned,
    desired: impl FnOnce(&Map<String, Value>) -> Owned,
) -> anyhow::Result<(Option<String>, Owned)> {
    let original = parse(file, before)?;
    let mut root = original.clone();
    strip(&mut root, previous);
    let wanted = desired(&root);
    let added = add(&mut root, &wanted);
    Ok((render(before, &original, &root), added))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn owned(entries: &[(&str, Value)]) -> Owned {
        let mut owned = Owned::new();
        for (path, value) in entries {
            owned.entry(path.to_string()).or_default().push(value.clone());
        }
        owned
    }

    #[test]
    fn the_users_entries_stay_and_termos_come_and_go() {
        let before = r#"{
  "model": "opus",
  "permissions": { "deny": ["Read(./secret)"] }
}
"#;
        let desired = owned(&[("permissions.deny", json!("Edit(/gen/**)")), ("permissions.deny", json!("Read(./secret)"))]);
        let (after, added) = merge("settings.json", Some(before), &Owned::new(), |_| desired.clone()).unwrap();
        let after = after.unwrap();
        // The user's entry was already there: it stays theirs.
        assert_eq!(added, owned(&[("permissions.deny", json!("Edit(/gen/**)"))]));
        let root: Value = serde_json::from_str(&after).unwrap();
        assert_eq!(root["permissions"]["deny"], json!(["Read(./secret)", "Edit(/gen/**)"]));
        assert_eq!(root.as_object().unwrap().keys().next().unwrap(), "model");

        let (removed, none) = merge("settings.json", Some(&after), &added, |_| Owned::new()).unwrap();
        assert!(none.is_empty());
        let removed: Value = serde_json::from_str(&removed.unwrap()).unwrap();
        assert_eq!(removed, serde_json::from_str::<Value>(before).unwrap());
    }

    #[test]
    fn a_file_of_termos_alone_goes_away() {
        let desired = owned(&[("hooks.Stop", json!({ "hooks": [] }))]);
        let (created, added) = merge("s.json", None, &Owned::new(), |_| desired.clone()).unwrap();
        assert!(created.is_some());
        let (gone, _) = merge("s.json", created.as_deref(), &added, |_| Owned::new()).unwrap();
        assert_eq!(gone, None);
    }

    #[test]
    fn an_untouched_file_keeps_its_formatting() {
        let before = "{\"a\":1}";
        let (after, _) = merge("s.json", Some(before), &Owned::new(), |_| Owned::new()).unwrap();
        assert_eq!(after.as_deref(), Some(before));
    }

    #[test]
    fn something_other_than_a_list_is_left_alone() {
        let before = r#"{ "permissions": { "deny": "x" } }"#;
        let desired = owned(&[("permissions.deny", json!("y"))]);
        let (after, added) = merge("s.json", Some(before), &Owned::new(), |_| desired.clone()).unwrap();
        assert!(added.is_empty());
        assert_eq!(after.as_deref(), Some(before));
    }

    #[test]
    fn crlf_files_stay_crlf() {
        let desired = owned(&[("x", json!(1))]);
        let (after, _) = merge("s.json", Some("{\r\n  \"a\": 1\r\n}\r\n"), &Owned::new(), |_| desired.clone()).unwrap();
        let after = after.unwrap();
        assert!(after.contains("\r\n") && !after.replace("\r\n", "").contains('\n'));
    }

    #[test]
    fn not_an_object_is_an_error() {
        assert!(merge("s.json", Some("[1]"), &Owned::new(), |_| Owned::new()).is_err());
    }
}
