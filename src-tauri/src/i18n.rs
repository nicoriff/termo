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
    GeminiCliNotFound,
    CopilotCliNotFound,
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
    AgentSecretFailed(&'a str),
    AgentDraftFailed(&'a str),
    AgentGithubNeedsRepo,
    AgentGithubClaudeOnly,
    AgentGithubWriteFailed(&'a str),
    AgentConnectorIncomplete,
    AgentConnectorBadName(&'a str),
    AgentImportNotAgent,
    AgentGitNotRepo(&'a str),
    AgentHookFailed(&'a str),
    AgentImportTooNew,
    /// (file, OS error)
    AgentFileFailed(&'a str, &'a str),
    /// the connector's name
    AgentBadConnectorName(&'a str),
    /// the variable's name
    AgentBadVariable(&'a str),
    /// the model
    AgentBadModel(&'a str),
    /// (agent, why)
    AgentImportRejected(&'a str, &'a str),
    /// runs a day
    ProjectDailyLimit(u32),
    ProjectNotFound,
    ProjectNoEntry,
    ProjectDraftFailed(&'a str),
    ProjectImproveFailed(&'a str),
    ProjectImportNotProject,
    ProjectVersionMissing,
    /// the system's error
    KeepAwakeFailed(&'a str),
    /// Why the computer is kept awake, as the system lists it (Linux).
    KeepAwakeReason,
    /// An agent run asked for while Modo IA is off (see ai_mode.rs).
    AiModeOff,
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
        (GeminiCliNotFound, Es) => "No se encontró Gemini CLI".into(),
        (GeminiCliNotFound, En) => "Couldn't find Gemini CLI".into(),
        (CopilotCliNotFound, Es) => "No se encontró Copilot CLI".into(),
        (CopilotCliNotFound, En) => "Couldn't find Copilot CLI".into(),

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

        (AgentSecretFailed(error), Es) => format!("No se pudo guardar el secreto en el llavero del sistema: {error}"),
        (AgentSecretFailed(error), En) => format!("Couldn't store the secret in the system keychain: {error}"),

        (AgentDraftFailed(error), Es) => format!("No se pudo armar el agente con IA: {error}"),
        (AgentDraftFailed(error), En) => format!("Couldn't build the agent with AI: {error}"),

        (AgentGithubNeedsRepo, Es) => "Para llevarlo a GitHub, la carpeta de trabajo del agente tiene que ser un repositorio de git".into(),
        (AgentGithubNeedsRepo, En) => "To take it to GitHub, the agent's working folder has to be a git repository".into(),

        (AgentGithubClaudeOnly, Es) => "Por ahora solo los agentes con Claude Code pueden correr en GitHub Actions".into(),
        (AgentGithubClaudeOnly, En) => "For now only Claude Code agents can run on GitHub Actions".into(),

        (AgentGithubWriteFailed(error), Es) => format!("No se pudo escribir el workflow: {error}"),
        (AgentGithubWriteFailed(error), En) => format!("Couldn't write the workflow: {error}"),

        (AgentConnectorIncomplete, Es) => "Falta el comando que inicia el conector".into(),
        (AgentConnectorIncomplete, En) => "The command that starts the connector is missing".into(),

        (AgentConnectorBadName(name), Es) => format!("\"{name}\" no sirve como nombre: usá letras, números y _ (o - en un header)"),
        (AgentGitNotRepo(dir), Es) => format!("\"{dir}\" no es un repositorio de git"),
        (AgentGitNotRepo(dir), En) => format!("\"{dir}\" isn't a git repository"),

        (AgentHookFailed(error), Es) => format!("No se pudo instalar el hook de git: {error}"),
        (AgentHookFailed(error), En) => format!("Couldn't install the git hook: {error}"),

        (AgentImportNotAgent, Es) => "El archivo no es un agente exportado desde Termo".into(),
        (AgentImportNotAgent, En) => "The file isn't an agent exported from Termo".into(),

        (AgentImportTooNew, Es) => "El agente se exportó desde una versión más nueva de Termo: actualizá la app para importarlo".into(),
        (AgentImportTooNew, En) => "The agent was exported from a newer Termo: update the app to import it".into(),

        (AgentFileFailed(file, error), Es) => format!("No se pudo usar el archivo \"{file}\": {error}"),
        (AgentFileFailed(file, error), En) => format!("Couldn't use the file \"{file}\": {error}"),
        (AgentBadConnectorName(name), Es) => {
            format!("\"{name}\" no sirve como nombre de conector: usá letras, números, - y _")
        }
        (AgentBadConnectorName(name), En) => format!("\"{name}\" can't be a connector's name: use letters, digits, - and _"),
        (AgentBadVariable(name), Es) => format!(
            "\"{name}\" no puede ser una variable de un conector ni de un secreto: no es un nombre válido o cambia cómo arranca un programa"
        ),
        (AgentBadVariable(name), En) => format!(
            "\"{name}\" can't be a connector's or a secret's variable: it isn't a valid name, or it changes how a program starts"
        ),
        (AgentBadModel(model), Es) => format!("\"{model}\" no es un nombre de modelo"),
        (AgentBadModel(model), En) => format!("\"{model}\" isn't a model's name"),
        (AgentImportRejected(agent, why), Es) => format!("No se importó nada: el agente \"{agent}\" trae algo no permitido. {why}"),
        (AgentImportRejected(agent, why), En) => {
            format!("Nothing was imported: the agent \"{agent}\" holds something not allowed. {why}")
        }

        (ProjectDailyLimit(runs), Es) => format!("El proyecto ya hizo sus {runs} corridas de hoy"),
        (ProjectDailyLimit(runs), En) => format!("The project already made its {runs} runs for today"),

        (ProjectNotFound, Es) => "El proyecto ya no existe".into(),
        (ProjectNotFound, En) => "The project no longer exists".into(),

        (ProjectNoEntry, Es) => "Elegí el agente de entrada del proyecto en su configuración".into(),
        (ProjectNoEntry, En) => "Choose the project's entry agent in its settings".into(),

        (ProjectDraftFailed(error), Es) => format!("No se pudo armar el proyecto con IA: {error}"),
        (ProjectDraftFailed(error), En) => format!("Couldn't build the project with AI: {error}"),

        (ProjectImproveFailed(error), Es) => format!("No se pudieron pensar mejoras: {error}"),
        (ProjectImproveFailed(error), En) => format!("Couldn't come up with improvements: {error}"),

        (ProjectImportNotProject, Es) => "El archivo no es un proyecto exportado desde Termo".into(),
        (ProjectImportNotProject, En) => "The file isn't a project exported from Termo".into(),

        (ProjectVersionMissing, Es) => "Esa versión del proyecto ya no está".into(),
        (ProjectVersionMissing, En) => "That version of the project is gone".into(),

        (KeepAwakeFailed(error), Es) => format!("No se pudo mantener la computadora despierta: {error}"),
        (KeepAwakeFailed(error), En) => format!("Couldn't keep the computer awake: {error}"),

        (KeepAwakeReason, Es) => "Termo mantiene la computadora despierta".into(),
        (KeepAwakeReason, En) => "Termo is keeping the computer awake".into(),

        (AiModeOff, Es) => "Modo IA está apagado: activalo en Preferencias para usar los agentes".into(),
        (AiModeOff, En) => "Modo IA is off: turn it on in Preferences to use agents".into(),

        (AgentConnectorBadName(name), En) => format!("\"{name}\" can't be a name: use letters, digits and _ (or - in a header)"),
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
            Msg::GeminiCliNotFound,
            Msg::CopilotCliNotFound,
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
            Msg::AgentSecretFailed("locked"),
            Msg::AgentDraftFailed("timeout"),
            Msg::AgentGithubNeedsRepo,
            Msg::AgentGithubClaudeOnly,
            Msg::AgentGithubWriteFailed("access denied"),
            Msg::AgentConnectorIncomplete,
            Msg::AgentConnectorBadName("MY VAR"),
            Msg::AgentImportNotAgent,
            Msg::AgentGitNotRepo("C:\\x"),
            Msg::AgentHookFailed("access denied"),
            Msg::AgentImportTooNew,
            Msg::AgentFileFailed("a.json", "access denied"),
            Msg::AgentBadConnectorName("x,Bash"),
            Msg::AgentBadVariable("NODE_OPTIONS"),
            Msg::AgentBadModel("--x"),
            Msg::AgentImportRejected("Revisor", "bad name"),
            Msg::ProjectDailyLimit(20),
            Msg::ProjectNotFound,
            Msg::ProjectNoEntry,
            Msg::ProjectDraftFailed("timeout"),
            Msg::ProjectImproveFailed("timeout"),
            Msg::ProjectImportNotProject,
            Msg::ProjectVersionMissing,
            Msg::KeepAwakeFailed("access denied"),
            Msg::KeepAwakeReason,
            Msg::AiModeOff,
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
