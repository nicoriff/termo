//! Translations for the user-visible strings produced by the backend (errors
//! shown in the UI, window titles). The frontend resolves the effective
//! language (including "system") and reports it via `set_ui_language`.
//!
//! Every `Msg` must be translated for every `Lang`: `tr_in` matches on both,
//! so a missing translation fails to compile.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Lang {
    Es = 0,
    En = 1,
}

static CURRENT: AtomicU8 = AtomicU8::new(Lang::Es as u8);

pub fn set(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::En as u8 {
        Lang::En
    } else {
        Lang::Es
    }
}

pub enum Msg<'a> {
    PreferencesTitle,
    HomeDirNotFound,
    NotAJsonObject(&'a str),
    HooksNotAnObject,
    HooksEventNotAList(&'a str),
    OhMyPoshNoBinary,
    InvalidThemeName(&'a str),
    FontFileNotFound(&'a str),
    ClaudeCliNotFound,
    CodexCliNotFound,
    MarketplaceCommandFailed(&'a str),
    InvalidComponentName(&'a str),
    CodexHasNoPlugins,
    McpRegistryUnreachable(&'a str),
    McpCodexHeadersUnsupported,
}

pub fn tr(msg: Msg) -> String {
    tr_in(current(), msg)
}

fn tr_in(lang: Lang, msg: Msg) -> String {
    use Lang::{En, Es};
    use Msg::*;

    match (msg, lang) {
        (PreferencesTitle, Es) => "Preferencias".into(),
        (PreferencesTitle, En) => "Preferences".into(),

        (HomeDirNotFound, Es) => "No se encontró el directorio home".into(),
        (HomeDirNotFound, En) => "Couldn't find the home directory".into(),

        (NotAJsonObject(file), Es) => format!("{file} no contiene un objeto JSON"),
        (NotAJsonObject(file), En) => format!("{file} doesn't contain a JSON object"),

        (HooksNotAnObject, Es) => "\"hooks\" no es un objeto".into(),
        (HooksNotAnObject, En) => "\"hooks\" isn't an object".into(),

        (HooksEventNotAList(event), Es) => format!("\"hooks.{event}\" no es una lista"),
        (HooksEventNotAList(event), En) => format!("\"hooks.{event}\" isn't a list"),

        (OhMyPoshNoBinary, Es) => "Oh My Posh no tiene un binario para esta plataforma".into(),
        (OhMyPoshNoBinary, En) => "Oh My Posh has no binary for this platform".into(),

        (InvalidThemeName(theme), Es) => format!("Nombre de tema inválido: {theme}"),
        (InvalidThemeName(theme), En) => format!("Invalid theme name: {theme}"),

        (FontFileNotFound(family), Es) => format!("No se encontró el archivo de fuente para \"{family}\""),
        (FontFileNotFound(family), En) => format!("Couldn't find the font file for \"{family}\""),

        (ClaudeCliNotFound, Es) => "No se encontró el CLI de Claude Code".into(),
        (ClaudeCliNotFound, En) => "Couldn't find the Claude Code CLI".into(),

        (CodexCliNotFound, Es) => "No se encontró el CLI de Codex".into(),
        (CodexCliNotFound, En) => "Couldn't find the Codex CLI".into(),

        (MarketplaceCommandFailed(detail), Es) => format!("Falló el comando: {detail}"),
        (MarketplaceCommandFailed(detail), En) => format!("The command failed: {detail}"),

        (InvalidComponentName(name), Es) => format!("Nombre de componente inválido: \"{name}\""),
        (InvalidComponentName(name), En) => format!("Invalid component name: \"{name}\""),

        (CodexHasNoPlugins, Es) => "Codex no tiene sistema de plugins".into(),
        (CodexHasNoPlugins, En) => "Codex has no plugin system".into(),

        (McpRegistryUnreachable(detail), Es) => format!("No se pudo consultar el registro de MCP: {detail}"),
        (McpRegistryUnreachable(detail), En) => format!("Couldn't reach the MCP registry: {detail}"),

        (McpCodexHeadersUnsupported, Es) =>
            "Este servidor necesita encabezados personalizados, y el CLI de Codex todavía no tiene forma de recibirlos (solo admite un token de portador leído de una variable de entorno)".into(),
        (McpCodexHeadersUnsupported, En) =>
            "This server needs custom headers, and the Codex CLI has no way to take them yet (it only accepts a bearer token read from an environment variable)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_messages() -> Vec<Msg<'static>> {
        vec![
            Msg::PreferencesTitle,
            Msg::HomeDirNotFound,
            Msg::NotAJsonObject("settings.json"),
            Msg::HooksNotAnObject,
            Msg::HooksEventNotAList("Stop"),
            Msg::OhMyPoshNoBinary,
            Msg::InvalidThemeName("x"),
            Msg::FontFileNotFound("Fira Code"),
            Msg::ClaudeCliNotFound,
            Msg::CodexCliNotFound,
            Msg::MarketplaceCommandFailed("exit 1"),
            Msg::InvalidComponentName("../x"),
            Msg::CodexHasNoPlugins,
            Msg::McpRegistryUnreachable("timeout"),
            Msg::McpCodexHeadersUnsupported,
        ]
    }

    #[test]
    fn every_message_renders_in_both_languages() {
        for lang in [Lang::Es, Lang::En] {
            for msg in all_messages() {
                assert!(!tr_in(lang, msg).trim().is_empty());
            }
        }
    }

    #[test]
    fn languages_differ_and_params_are_included() {
        assert_ne!(tr_in(Lang::Es, Msg::PreferencesTitle), tr_in(Lang::En, Msg::PreferencesTitle));
        assert!(tr_in(Lang::En, Msg::InvalidThemeName("dracula")).contains("dracula"));
    }
}
