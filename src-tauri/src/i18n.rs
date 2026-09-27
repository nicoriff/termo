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
    UnsafeMcpLaunchValue(&'a str),
    UnsafeMcpRemoteUrl(&'a str),
    /// (folder, OS error)
    FolderUnreadable(&'a str, &'a str),
    FolderNameInvalid(&'a str),
    FolderExists(&'a str),
    /// (folder name, OS error)
    FolderCreateFailed(&'a str, &'a str),
    /// (file name, OS error)
    FileReadFailed(&'a str, &'a str),
    /// (file name, limit in MB)
    FileTooLarge(&'a str, u64),
    FileNotText(&'a str),
    /// (file name, OS error)
    FileWriteFailed(&'a str, &'a str),
    AgentTaskMissing,
    AgentAlreadyRunning,
    AgentWorkDirMissing(&'a str),
    AgentRunFailed(&'a str),
    AgentSaveFailed(&'a str),
    AgentStopped,
    /// minutes
    AgentTimedOut(u32),
    /// exit code
    AgentExitedWith(&'a str),
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

        (UnsafeMcpLaunchValue(value), Es) =>
            format!("El registro de MCP declaró un valor de lanzamiento con forma de opción de línea de comandos (\"{value}\"): no se instala, por seguridad"),
        (UnsafeMcpLaunchValue(value), En) =>
            format!("The MCP registry declared a launch value shaped like a command-line flag (\"{value}\"): not installing it, for safety"),

        (UnsafeMcpRemoteUrl(url), Es) =>
            format!("La URL de este servidor MCP no es http(s) (\"{url}\"): no se instala, por seguridad"),
        (UnsafeMcpRemoteUrl(url), En) =>
            format!("This MCP server's URL isn't http(s) (\"{url}\"): not installing it, for safety"),

        (FolderUnreadable(folder, error), Es) => format!("No se puede abrir {folder}: {error}"),
        (FolderUnreadable(folder, error), En) => format!("Can't open {folder}: {error}"),

        (FolderNameInvalid(name), Es) => {
            format!("\"{name}\" no es un nombre de carpeta válido (sin / \\ : * ? \" < > | ni nombres reservados)")
        }
        (FolderNameInvalid(name), En) => {
            format!("\"{name}\" isn't a valid folder name (no / \\ : * ? \" < > | or reserved names)")
        }

        (FolderExists(name), Es) => format!("Ya existe una carpeta \"{name}\" acá"),
        (FolderExists(name), En) => format!("A folder named \"{name}\" already exists here"),

        (FolderCreateFailed(name, error), Es) => format!("No se pudo crear \"{name}\": {error}"),
        (FolderCreateFailed(name, error), En) => format!("Couldn't create \"{name}\": {error}"),

        (FileReadFailed(name, error), Es) => format!("No se pudo abrir \"{name}\": {error}"),
        (FileReadFailed(name, error), En) => format!("Couldn't open \"{name}\": {error}"),

        (FileTooLarge(name, limit), Es) => format!("\"{name}\" pesa más de {limit} MB: es demasiado grande para abrirlo acá"),
        (FileTooLarge(name, limit), En) => format!("\"{name}\" is over {limit} MB: too large to open here"),

        (FileNotText(name), Es) => format!("\"{name}\" no es un archivo de texto"),
        (FileNotText(name), En) => format!("\"{name}\" isn't a text file"),

        (FileWriteFailed(name, error), Es) => format!("No se pudo guardar \"{name}\": {error}"),
        (FileWriteFailed(name, error), En) => format!("Couldn't save \"{name}\": {error}"),

        (AgentTaskMissing, Es) => "El agente no tiene una tarea para hacer".into(),
        (AgentTaskMissing, En) => "The agent has no task to do".into(),

        (AgentAlreadyRunning, Es) => "El agente ya está corriendo".into(),
        (AgentAlreadyRunning, En) => "The agent is already running".into(),

        (AgentWorkDirMissing(dir), Es) => format!("No existe la carpeta de trabajo \"{dir}\""),
        (AgentWorkDirMissing(dir), En) => format!("The working folder \"{dir}\" doesn't exist"),

        (AgentRunFailed(error), Es) => format!("No se pudo iniciar el agente: {error}"),
        (AgentRunFailed(error), En) => format!("Couldn't start the agent: {error}"),

        (AgentSaveFailed(error), Es) => format!("No se pudo guardar el agente: {error}"),
        (AgentSaveFailed(error), En) => format!("Couldn't save the agent: {error}"),

        (AgentStopped, Es) => "Detenido a mano".into(),
        (AgentStopped, En) => "Stopped by hand".into(),

        (AgentTimedOut(minutes), Es) => format!("Se cortó a los {minutes} min (el límite de tiempo)"),
        (AgentTimedOut(minutes), En) => format!("Cut off after {minutes} min (the time limit)"),

        (AgentExitedWith(code), Es) => format!("Terminó con el código {code}"),
        (AgentExitedWith(code), En) => format!("Exited with code {code}"),
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
            Msg::UnsafeMcpLaunchValue("-c"),
            Msg::UnsafeMcpRemoteUrl("file:///etc/passwd"),
            Msg::FolderUnreadable("C:\\x", "access denied"),
            Msg::FolderNameInvalid("a/b"),
            Msg::FolderExists("project"),
            Msg::FolderCreateFailed("project", "access denied"),
            Msg::FileReadFailed("a.txt", "access denied"),
            Msg::FileTooLarge("big.log", 5),
            Msg::FileNotText("image.png"),
            Msg::FileWriteFailed("a.txt", "access denied"),
            Msg::AgentTaskMissing,
            Msg::AgentAlreadyRunning,
            Msg::AgentWorkDirMissing("C:\\x"),
            Msg::AgentRunFailed("not found"),
            Msg::AgentSaveFailed("disk full"),
            Msg::AgentStopped,
            Msg::AgentTimedOut(15),
            Msg::AgentExitedWith("1"),
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
