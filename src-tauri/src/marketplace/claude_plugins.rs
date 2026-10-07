//! Claude Code's plugin marketplaces as a browsable catalog.
//!
//! Two sources, joined, because neither is enough on its own:
//!
//! - `claude plugin list --json --available` is authoritative about what
//!   exists and what's installed, and it's the only thing that knows the
//!   `name@marketplace` id the installer takes — but it carries no category,
//!   author or homepage.
//! - Each marketplace's cloned `.claude-plugin/marketplace.json` has all of
//!   that, and is the file the marketplace's owner actually maintains.
//!
//! The CLI list is the spine and the manifest fills it in, so a plugin the
//! CLI reports but the manifest doesn't (a marketplace mid-update, a source
//! the CLI resolved some other way) still shows up rather than vanishing.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::agent_cli;
use super::model::{targets_for, AgentTarget, CatalogItem, ComponentClass, ItemDetail};

/// Where the CLI records the marketplaces the user has added.
fn known_marketplaces_file() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)?;
    Some(home.join(".claude").join("plugins").join("known_marketplaces.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub name: String,
    /// "github" / "url" / "local", as the CLI records it.
    pub kind: String,
    /// The repo or URL, for showing where a catalog comes from.
    pub location: Option<String>,
}

/// Manifest entries keyed by `<plugin name>@<marketplace name>`, plus the
/// list of marketplaces they came from.
pub struct DiskManifests {
    pub entries: HashMap<String, Value>,
    pub sources: Vec<SourceInfo>,
}

pub fn read_disk_manifests() -> DiskManifests {
    let mut entries = HashMap::new();
    let mut sources = Vec::new();

    let known = known_marketplaces_file()
        .and_then(|file| fs::read_to_string(file).ok())
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let Some(Value::Object(known)) = known else {
        return DiskManifests { entries, sources };
    };

    for (name, record) in known {
        let source = record.get("source");
        sources.push(SourceInfo {
            name: name.clone(),
            kind: source
                .and_then(|s| s.get("source"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            location: source
                .and_then(|s| s.get("repo").or_else(|| s.get("url")).or_else(|| s.get("path")))
                .and_then(Value::as_str)
                .map(str::to_string),
        });

        let Some(location) = record.get("installLocation").and_then(Value::as_str) else {
            continue;
        };
        let manifest = PathBuf::from(location).join(".claude-plugin").join("marketplace.json");
        let Ok(text) = fs::read_to_string(manifest) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let Some(plugins) = parsed.get("plugins").and_then(Value::as_array) else {
            continue;
        };
        for plugin in plugins {
            if let Some(plugin_name) = plugin.get("name").and_then(Value::as_str) {
                entries.insert(format!("{plugin_name}@{name}"), plugin.clone());
            }
        }
    }

    DiskManifests { entries, sources }
}

/// The GitHub owner an entry is published by, from any of the URL shapes
/// these catalogs use: a clone URL (`https://github.com/box/skills.git`), a
/// tree link (`.../adobe/skills/tree/main/...`) or an `owner/repo` shorthand.
pub(super) fn github_owner(url: &str) -> Option<String> {
    let rest = url
        .trim()
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let path = match rest.strip_prefix("github.com/") {
        Some(path) => path,
        // The bare `owner/repo` form the CLI records for a github source.
        None if !rest.contains('/') || rest.contains('.') => return None,
        None => rest,
    };
    let owner = path.split('/').next()?.trim();
    let valid = !owner.is_empty()
        && owner.len() <= 39
        && owner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    valid.then(|| owner.to_string())
}

/// No catalog here publishes an icon, so the publisher's GitHub avatar is
/// used — the one image that's actually tied to the entry and not guessed.
/// Anything not on GitHub gets none, and the UI draws initials instead of
/// pulling an image from some unrelated third party.
fn icon_for(entry: &Value, manifest: Option<&Value>) -> Option<String> {
    let candidates = [
        entry.get("source").and_then(|s| s.get("url")).and_then(Value::as_str),
        entry.get("source").and_then(|s| s.get("repo")).and_then(Value::as_str),
        manifest.and_then(|m| m.get("source")).and_then(|s| s.get("url")).and_then(Value::as_str),
        manifest.and_then(|m| m.get("homepage")).and_then(Value::as_str),
    ];
    candidates
        .into_iter()
        .flatten()
        .find_map(github_owner)
        .map(|owner| format!("https://github.com/{owner}.png?size=80"))
}

/// Read as `rating` or `stars` if an entry ever carries one, clamped to the
/// 0-5 the UI draws. Today nothing does.
fn rating_of(entry: Option<&Value>) -> Option<f64> {
    let entry = entry?;
    ["rating", "stars"]
        .iter()
        .find_map(|key| entry.get(*key).and_then(Value::as_f64))
        .filter(|value| (0.0..=5.0).contains(value))
}

fn string_field(entry: Option<&Value>, key: &str) -> Option<String> {
    entry?.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `author` is either a string or an object with a `name`.
fn author_of(entry: Option<&Value>) -> Option<String> {
    let author = entry?.get("author")?;
    author
        .as_str()
        .map(str::to_string)
        .or_else(|| author.get("name").and_then(Value::as_str).map(str::to_string))
}

fn string_list(entry: Option<&Value>, key: &str) -> Vec<String> {
    entry
        .and_then(|entry| entry.get(key))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// `installed` entries are matched by `<name>@<marketplace>` id — what the
/// CLI actually calls `id` here (`available` entries call the same shape
/// `pluginId`; the two arrays don't agree on a field name). `pluginId` and a
/// bare string are also accepted, for older CLI versions that used one of
/// those instead.
fn installed_ids(cli: &Value) -> Vec<String> {
    cli.get("installed")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.as_str().map(str::to_string).or_else(|| {
                        item.get("id")
                            .or_else(|| item.get("pluginId"))
                            .or_else(|| item.get("name"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Joins the CLI's catalog with the manifests read off disk. Pure, so the
/// join is testable without a `claude` binary or a real marketplace clone.
pub fn merge_catalog(cli: &Value, manifests: &HashMap<String, Value>) -> Vec<CatalogItem> {
    let installed = installed_ids(cli);
    let mut available = cli.get("available").and_then(Value::as_array).cloned().unwrap_or_default();

    // `available` leaves out whatever is already installed, so those only
    // exist in `installed` — which carries no name or marketplace of its
    // own, just the `<name>@<marketplace>` id. Rebuilt into the `available`
    // shape here; the manifest on disk fills in the description and the rest.
    let listed: Vec<String> = available
        .iter()
        .filter_map(|entry| entry.get("pluginId").and_then(Value::as_str).map(str::to_string))
        .collect();
    for id in &installed {
        if listed.contains(id) {
            continue;
        }
        let Some((name, marketplace)) = id.split_once('@') else { continue };
        let version = cli
            .get("installed")
            .and_then(Value::as_array)
            .and_then(|items| items.iter().find(|item| item.get("id").and_then(Value::as_str) == Some(id)))
            .and_then(|item| item.get("version"))
            .cloned()
            .unwrap_or(Value::Null);
        available.push(serde_json::json!({
            "pluginId": id,
            "name": name,
            "marketplaceName": marketplace,
            "version": version,
        }));
    }

    available
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name").and_then(Value::as_str)?;
            let marketplace = entry
                .get("marketplaceName")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let plugin_id = entry
                .get("pluginId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("{name}@{marketplace}"));

            let key = format!("{name}@{marketplace}");
            let manifest = manifests.get(&key);
            let installed_in = if installed.contains(&plugin_id) || installed.contains(&name.to_string()) {
                vec![AgentTarget::Claude]
            } else {
                Vec::new()
            };

            let mut tags = string_list(manifest, "tags");
            tags.extend(string_list(manifest, "keywords"));

            Some(CatalogItem {
                id: format!("plugin:{plugin_id}"),
                class: ComponentClass::Plugin,
                name: name.to_string(),
                display_name: string_field(manifest, "displayName").unwrap_or_else(|| name.to_string()),
                description: entry
                    .get("description")
                    .and_then(Value::as_str)
                    .or_else(|| manifest.and_then(|m| m.get("description")).and_then(Value::as_str))
                    .unwrap_or_default()
                    .to_string(),
                source_id: format!("claude-marketplace:{marketplace}"),
                source_label: marketplace.to_string(),
                category: string_field(manifest, "category"),
                author: author_of(manifest),
                homepage: string_field(manifest, "homepage"),
                version: entry
                    .get("version")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| string_field(manifest, "version")),
                tags,
                install_count: entry.get("installCount").and_then(Value::as_u64),
                icon_url: icon_for(entry, manifest),
                rating: rating_of(manifest).or_else(|| rating_of(Some(entry))),
                declares_skills: string_list(manifest, "skills"),
                targets: targets_for(ComponentClass::Plugin),
                installed_in,
                detail: ItemDetail::Plugin {
                    plugin_id,
                    marketplace: marketplace.to_string(),
                    source: entry
                        .get("source")
                        .cloned()
                        .or_else(|| manifest.and_then(|m| m.get("source")).cloned())
                        .unwrap_or(Value::Null),
                    strict: manifest.and_then(|m| m.get("strict")).and_then(Value::as_bool),
                },
                extra: Default::default(),
            })
        })
        .collect()
}

/// The catalog straight off disk, for when the CLI isn't installed: the user
/// can still browse what their marketplaces offer, just not install it.
pub fn catalog_from_disk(manifests: &DiskManifests) -> Vec<CatalogItem> {
    let available: Vec<Value> = manifests
        .entries
        .iter()
        .map(|(key, entry)| {
            let (name, marketplace) = key.split_once('@').unwrap_or((key.as_str(), "unknown"));
            serde_json::json!({
                "name": name,
                "marketplaceName": marketplace,
                "pluginId": key,
                "description": entry.get("description").and_then(Value::as_str).unwrap_or_default(),
            })
        })
        .collect();
    merge_catalog(&serde_json::json!({ "available": available }), &manifests.entries)
}

pub fn fetch_catalog(claude: &PathBuf) -> Result<Value, String> {
    agent_cli::run_json(claude, &["plugin", "list", "--json", "--available"])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cli_fixture() -> Value {
        json!({
            // The real CLI's installed entries carry `id`, never `pluginId`
            // or `name` — only `available` entries use those. Mismatching
            // this field name is exactly what made every plugin show as
            // never installed (see installed_ids's doc comment).
            "installed": [{ "id": "box@claude-plugins-official", "version": "abc123", "scope": "user", "enabled": true }],
            "available": [
                {
                    "pluginId": "box@claude-plugins-official",
                    "name": "box",
                    "description": "desde el CLI",
                    "marketplaceName": "claude-plugins-official",
                    "source": { "source": "url", "url": "https://github.com/box/skills.git" },
                    "installCount": 42
                },
                {
                    "pluginId": "solo-cli@claude-plugins-official",
                    "name": "solo-cli",
                    "description": "sin manifiesto en disco",
                    "marketplaceName": "claude-plugins-official"
                }
            ]
        })
    }

    fn manifest_fixture() -> HashMap<String, Value> {
        HashMap::from([(
            "box@claude-plugins-official".to_string(),
            json!({
                "name": "box",
                "description": "desde el manifiesto",
                "category": "productivity",
                "author": { "name": "Box" },
                "homepage": "https://box.com",
                "skills": ["./skills/box"],
                "displayName": "Box"
            }),
        )])
    }

    #[test]
    fn enriches_cli_entries_with_the_marketplace_manifest() {
        let items = merge_catalog(&cli_fixture(), &manifest_fixture());
        let box_item = items.iter().find(|item| item.name == "box").unwrap();

        assert_eq!(box_item.category.as_deref(), Some("productivity"));
        assert_eq!(box_item.author.as_deref(), Some("Box"));
        assert_eq!(box_item.homepage.as_deref(), Some("https://box.com"));
        assert_eq!(box_item.display_name, "Box");
        assert_eq!(box_item.declares_skills, vec!["./skills/box".to_string()]);
        assert_eq!(box_item.install_count, Some(42));
        // The CLI's description wins: it's the one the installer shows.
        assert_eq!(box_item.description, "desde el CLI");
        assert_eq!(box_item.id, "plugin:box@claude-plugins-official");
    }

    #[test]
    fn keeps_entries_the_manifest_does_not_have() {
        let items = merge_catalog(&cli_fixture(), &manifest_fixture());
        let solo = items.iter().find(|item| item.name == "solo-cli").unwrap();

        assert!(solo.category.is_none());
        assert!(solo.author.is_none());
        assert_eq!(solo.description, "sin manifiesto en disco");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn takes_the_icon_from_the_publisher_on_github() {
        let items = merge_catalog(&cli_fixture(), &manifest_fixture());
        let box_item = items.iter().find(|item| item.name == "box").unwrap();

        assert_eq!(box_item.icon_url.as_deref(), Some("https://github.com/box.png?size=80"));
    }

    #[test]
    fn reads_every_url_shape_these_catalogs_use() {
        assert_eq!(github_owner("https://github.com/box/skills.git").as_deref(), Some("box"));
        assert_eq!(
            github_owner("https://github.com/adobe/skills/tree/main/plugins/x").as_deref(),
            Some("adobe")
        );
        assert_eq!(github_owner("anthropics/claude-plugins-official").as_deref(), Some("anthropics"));
        // Anything not on GitHub gets no icon rather than one fetched from
        // an unrelated third party.
        assert_eq!(github_owner("https://42crunch.com"), None);
        assert_eq!(github_owner("https://gitlab.com/someone/repo"), None);
        assert_eq!(github_owner("https://github.com/"), None);
        assert_eq!(github_owner("https://github.com/bad owner/repo"), None);
    }

    #[test]
    fn there_is_no_rating_to_show_unless_a_source_publishes_one() {
        let items = merge_catalog(&cli_fixture(), &manifest_fixture());
        assert!(items.iter().all(|item| item.rating.is_none()));

        let rated = HashMap::from([(
            "box@claude-plugins-official".to_string(),
            json!({ "name": "box", "rating": 4.5 }),
        )]);
        let items = merge_catalog(&cli_fixture(), &rated);
        assert_eq!(items.iter().find(|i| i.name == "box").unwrap().rating, Some(4.5));

        // Out of the 0-5 range the UI draws, so it's ignored.
        let bogus = HashMap::from([(
            "box@claude-plugins-official".to_string(),
            json!({ "name": "box", "rating": 97.0 }),
        )]);
        assert!(merge_catalog(&cli_fixture(), &bogus)
            .iter()
            .find(|i| i.name == "box")
            .unwrap()
            .rating
            .is_none());
    }

    #[test]
    fn marks_only_what_is_installed() {
        let items = merge_catalog(&cli_fixture(), &manifest_fixture());

        let box_item = items.iter().find(|item| item.name == "box").unwrap();
        let solo = items.iter().find(|item| item.name == "solo-cli").unwrap();
        assert_eq!(box_item.installed_in, vec![AgentTarget::Claude]);
        assert!(solo.installed_in.is_empty());
        // No plugin is ever installable into Codex.
        assert_eq!(box_item.targets, vec![AgentTarget::Claude]);
    }

    #[test]
    fn an_installed_plugin_missing_from_available_is_still_listed() {
        // The real CLI drops installed plugins from `available` entirely, so
        // without this they'd vanish from the panel the moment they're
        // installed, and never show as installed at all.
        let cli = json!({
            "installed": [{ "id": "box@claude-plugins-official", "version": "abc123" }],
            "available": [{
                "pluginId": "solo-cli@claude-plugins-official",
                "name": "solo-cli",
                "marketplaceName": "claude-plugins-official"
            }]
        });
        let items = merge_catalog(&cli, &manifest_fixture());

        assert_eq!(items.len(), 2);
        let box_item = items.iter().find(|item| item.name == "box").unwrap();
        assert_eq!(box_item.installed_in, vec![AgentTarget::Claude]);
        assert_eq!(box_item.id, "plugin:box@claude-plugins-official");
        assert_eq!(box_item.version.as_deref(), Some("abc123"));
        // Nothing in `installed` describes it; the manifest does.
        assert_eq!(box_item.description, "desde el manifiesto");
        assert_eq!(box_item.category.as_deref(), Some("productivity"));
    }

    #[test]
    fn reads_the_installed_ids_shape_the_real_cli_uses() {
        // `claude plugin list --json --available` calls this field `id`,
        // never `pluginId` or `name` (those only appear on `available`
        // entries) — this regression is exactly why nothing ever showed as
        // installed.
        assert_eq!(
            installed_ids(&json!({ "installed": [{ "id": "box@official", "version": "1", "enabled": true }] })),
            vec!["box@official".to_string()]
        );
    }

    #[test]
    fn also_accepts_the_older_installed_id_shapes() {
        assert_eq!(
            installed_ids(&json!({ "installed": [{ "pluginId": "box@official", "name": "box" }] })),
            vec!["box@official".to_string()]
        );
        assert_eq!(installed_ids(&json!({ "installed": ["box@official"] })), vec!["box@official".to_string()]);
    }

    #[test]
    fn a_catalog_without_the_cli_still_lists_what_is_on_disk() {
        let manifests = DiskManifests {
            entries: manifest_fixture(),
            sources: Vec::new(),
        };
        let items = catalog_from_disk(&manifests);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "box");
        assert_eq!(items[0].category.as_deref(), Some("productivity"));
        assert!(items[0].installed_in.is_empty());
    }
}
