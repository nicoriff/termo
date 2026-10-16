//! Checks a repository is likely to have, offered in the panel as "done"
//! checks: the scripts its package.json names check, lint, test and the
//! like (run with the package manager its lock file says), Cargo's test, and
//! a Makefile's test target.

use std::fs;
use std::path::Path;

use serde_json::Value;

/// package.json scripts worth running before saying done, in this order.
const SCRIPTS: [&str; 6] = ["check", "typecheck", "lint", "test", "test:unit", "build"];

fn package_manager(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if root.join("yarn.lock").exists() {
        "yarn"
    } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// The checks the repository's own files suggest, as commands.
pub fn suggest(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    if let Ok(text) = fs::read_to_string(root.join("package.json")) {
        if let Ok(package) = serde_json::from_str::<Value>(&text) {
            let manager = package_manager(root);
            if let Some(scripts) = package.get("scripts").and_then(Value::as_object) {
                for name in SCRIPTS {
                    if scripts.contains_key(name) {
                        found.push(format!("{manager} run {name}"));
                    }
                }
            }
        }
    }
    // A Tauri app keeps its crate one folder down.
    for (folder, prefix) in [("", ""), ("src-tauri", "cargo test --manifest-path src-tauri/Cargo.toml")] {
        let manifest = root.join(folder).join("Cargo.toml");
        if manifest.is_file() {
            found.push(if prefix.is_empty() { "cargo test".to_string() } else { prefix.to_string() });
            break;
        }
    }
    if root.join("pyproject.toml").is_file() || root.join("pytest.ini").is_file() {
        found.push("pytest".to_string());
    }
    if let Ok(makefile) = fs::read_to_string(root.join("Makefile")) {
        if makefile.lines().any(|line| line.starts_with("test:")) {
            found.push("make test".to_string());
        }
    }
    if root.join("go.mod").is_file() {
        found.push("go test ./...".to_string());
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripts_cargo_and_make() {
        let root = std::env::temp_dir().join(format!("termo-harness-detect-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("src-tauri")).unwrap();
        fs::write(root.join("package.json"), r#"{ "scripts": { "test": "vitest", "dev": "vite", "check": "tsc" } }"#).unwrap();
        fs::write(root.join("pnpm-lock.yaml"), "").unwrap();
        fs::write(root.join("src-tauri").join("Cargo.toml"), "[package]").unwrap();
        fs::write(root.join("Makefile"), "build:\n\tcc\ntest:\n\t./t\n").unwrap();
        assert_eq!(
            suggest(&root),
            ["pnpm run check", "pnpm run test", "cargo test --manifest-path src-tauri/Cargo.toml", "make test"]
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
