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
