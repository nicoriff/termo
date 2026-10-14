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
    GrokCliNotFound,
    NpmNotFound,
    AgentInstallBusy,
    AgentInstallCancelled,
    AgentInstallBadFolder(&'a str),
    AgentInstallFailed(&'a str),
    AgentInstalledNotFound(&'a str),
    MarketplaceCommandFailed(&'a str),
    InvalidComponentName(&'a str),
    CodexHasNoPlugins,
    McpRegistryUnreachable(&'a str),
    McpCodexHeadersUnsupported,
    /// the name, the agent
    McpNameTaken(&'a str, &'a str),
    SourceKeyMissing,
    SourceKeyInvalid,
    SourceNeedsNoKey,
    /// the error
    SourceKeyNotSaved(&'a str),
    SmitheryKeyRejected,
    SmitheryNotFound,
    SmitheryRateLimited,
    /// the error
    SmitheryUnreachable(&'a str),
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
    /// The agent CLI is installed under a path cmd.exe can't be trusted to read as one.
    AgentLaunchUnsafePath(&'a str),
    /// A favorite for an unknown agent or with an id that can't be reopened.
    AgentFavoriteInvalid,
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
    AskFolderRunsPrograms,
    /// the planner's error
    OrchestrationPlanFailed(&'a str),
    /// the error
    OrchestrationReportFailed(&'a str),
    /// Told to every orchestration task: it may ask the user.
    OrchestrationCanAsk,
    /// Heads the answers the user gave before planning.
    OrchestrationClarifications,
    OrchestrationQuestionClosed,
    /// who answered, the model, what it said
    OrchestrationServiceBlocked(&'a str, &'a str, &'a str),
    /// the folder
    OrchestrationNoFolder(&'a str),
    OrchestrationFolderRequired,
    OrchestrationNotFound,
    /// Approving, resuming or resolving something its state doesn't allow.
    OrchestrationWrongState,
    OrchestrationEmptyPlan,
    OrchestrationSaveFailed(&'a str),
    /// Notes an orchestration is left with (see orchestration/executor.rs).
    OrchestrationStopped,
    OrchestrationInterrupted,
    /// what it spent, the budget
    OrchestrationOverBudget(&'a str, &'a str),
    OrchestrationBlocked,
    /// the supervisor's error
    OrchestrationNoReview(&'a str),
    /// The task an orchestration's agent stands for between tasks.
    OrchestrationAgentTask,
    /// Labels around what an agent of an orchestration is given: the line
    /// before "done when", the heading of the results it builds on, and the
    /// one over its own failed attempt.
    OrchestrationDoneWhen,
    OrchestrationBuildsOn,
    OrchestrationLastAttempt,
    GitNotFound,
    GitNotARepo,
    /// (file, parse error)
    HarnessInvalid(&'a str, &'a str),
    /// (OS error)
    HarnessWriteFailed(&'a str),
    /// Another action is still running on the repository.
    GitBusy,
    /// what went wrong
    GitFailed(&'a str),
    GitTimedOut,
    /// what git said
    GitNeedsCredentials(&'a str),
    GitCancelled,
    GitBranchNameInvalid(&'a str),
    GitNoUpstream,
    GitNoRemote,
    GitDetached,
    GitCommitNeedsMessage,
    /// The service's CLI: gh or glab.
    PrCliMissing(&'a str),
    PrNotSignedIn(&'a str),
    PrUnreadable(&'a str),
    /// what the CLI said
    PrFailed(&'a str),
    PrUnsupported,
    PrNeedsTitle,
    GitReviewBusy,
    /// why
    GitReviewFailed(&'a str),
    GitReviewNothing,
    GitReviewCancelled,
    GitReviewTimedOut,
    GitReviewUnavailable,
    GitReviewNoBase,
    GitReviewGone,
    GitReviewFixNeedsEdit,
    GitReviewAgentGone,
    GitReviewNoPr,
    GitReviewCommentTitle,
    GitReviewSerious,
    GitReviewWarning,
    GitReviewSuggestion,
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
        (GrokCliNotFound, Es) => "No se encontró Grok".into(),
        (GrokCliNotFound, En) => "Couldn't find Grok".into(),
        (NpmNotFound, Es) => "No se encontró npm. Instalá Node.js y volvé a intentar.".into(),
        (NpmNotFound, En) => "Couldn't find npm. Install Node.js and try again.".into(),
        (AgentInstallBusy, Es) => "Ya hay una instalación en curso.".into(),
        (AgentInstallBusy, En) => "Another install is already running.".into(),
        (AgentInstallCancelled, Es) => "Instalación cancelada.".into(),
        (AgentInstallCancelled, En) => "Install cancelled.".into(),
        (AgentInstallBadFolder(dir), Es) => format!("No se puede instalar en «{dir}»: tiene que ser una carpeta con ruta completa en la que se pueda escribir."),
        (AgentInstallBadFolder(dir), En) => format!("Can't install into “{dir}”: it has to be a folder with a full path that can be written to."),
        (AgentInstallFailed(detail), Es) => format!("npm no pudo instalarlo:\n{detail}"),
        (AgentInstallFailed(detail), En) => format!("npm couldn't install it:\n{detail}"),
        (AgentInstalledNotFound(tool), Es) => format!("npm terminó bien, pero no aparece el comando «{tool}» en la carpeta donde se instaló."),
        (AgentInstalledNotFound(tool), En) => format!("npm finished, but the “{tool}” command isn't in the folder it installed into."),

        (MarketplaceCommandFailed(detail), Es) => format!("Falló el comando: {detail}"),
        (MarketplaceCommandFailed(detail), En) => format!("The command failed: {detail}"),

        (InvalidComponentName(name), Es) => format!("Nombre de componente inválido: \"{name}\""),
        (InvalidComponentName(name), En) => format!("Invalid component name: \"{name}\""),

        (CodexHasNoPlugins, Es) => "Codex no tiene sistema de plugins".into(),
        (CodexHasNoPlugins, En) => "Codex has no plugin system".into(),

        (McpRegistryUnreachable(detail), Es) => format!("No se pudo consultar el registro de MCP: {detail}"),
        (McpRegistryUnreachable(detail), En) => format!("Couldn't reach the MCP registry: {detail}"),

        (McpNameTaken(name, agent), Es) => format!(
            "Ya hay otro servidor llamado \"{name}\" en {agent}. Quitalo o renombralo ahí antes de instalar este."
        ),
        (McpNameTaken(name, agent), En) => format!(
            "There's already another server called \"{name}\" in {agent}. Remove or rename it there before installing this one."
        ),
        (SourceKeyMissing, Es) => "Esta fuente ahora pide una API key. Cargala en Preferencias → Marketplace.".into(),
        (SourceKeyMissing, En) => "This source now asks for an API key. Add it in Preferences → Marketplace.".into(),
        (SourceKeyInvalid, Es) => "Eso no parece una API key: tiene que ser un solo texto, sin espacios.".into(),
        (SourceKeyInvalid, En) => "That doesn't look like an API key: it has to be one piece of text, with no spaces.".into(),
        (SourceNeedsNoKey, Es) => "Esta fuente no usa API key.".into(),
        (SourceNeedsNoKey, En) => "This source takes no API key.".into(),
        (SourceKeyNotSaved(error), Es) => format!("No se pudo guardar la key en el llavero del sistema: {error}"),
        (SourceKeyNotSaved(error), En) => format!("Couldn't save the key in the system keychain: {error}"),
        (SmitheryKeyRejected, Es) => "Smithery rechazó la API key. Revisala en Preferencias → Marketplace.".into(),
        (SmitheryKeyRejected, En) => "Smithery turned the API key down. Check it in Preferences → Marketplace.".into(),
        (SmitheryNotFound, Es) => "Smithery no tiene ese servidor.".into(),
        (SmitheryNotFound, En) => "Smithery doesn't have that server.".into(),
        (SmitheryRateLimited, Es) => "Smithery está limitando los pedidos de tu key. Probá de nuevo en un rato.".into(),
        (SmitheryRateLimited, En) => "Smithery is limiting your key's requests. Try again in a while.".into(),
        (SmitheryUnreachable(error), Es) => format!("No se pudo consultar Smithery: {error}"),
        (SmitheryUnreachable(error), En) => format!("Couldn't reach Smithery: {error}"),
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
        (AgentFavoriteInvalid, Es) => "Esta sesión no se puede guardar en favoritos: no es de un agente conocido o su identificador no se puede reabrir.".into(),
        (AgentFavoriteInvalid, En) => "This session can't be saved as a favorite: it isn't from a known agent or its id can't be reopened.".into(),
        (AgentLaunchUnsafePath(path), Es) => format!(
            "No se abrió el agente: está instalado en una ruta con caracteres que la consola leería como comandos ({path}). Abrilo escribiendo su nombre en una consola."
        ),
        (AgentLaunchUnsafePath(path), En) => format!(
            "The agent didn't open: it's installed under a path with characters the console would read as commands ({path}). Open it by typing its name in a console."
        ),
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

        (OrchestrationPlanFailed(error), Es) => format!("No se pudo armar el plan: {error}"),
        (OrchestrationPlanFailed(error), En) => format!("Couldn't build the plan: {error}"),
        (OrchestrationReportFailed(error), Es) => format!("No se pudo escribir el informe final: {error}"),
        (OrchestrationReportFailed(error), En) => format!("Couldn't write the final report: {error}"),
        (OrchestrationCanAsk, Es) => "Si te encontrás con una decisión que le corresponde al usuario y cambia el resultado (un enfoque u otro, el alcance, qué hacer con lo que ya existe), preguntale con la herramienta ask_user de Termo en vez de suponer. Para lo demás, decidí vos y seguí.".into(),
        (OrchestrationCanAsk, En) => "If you hit a decision that is the user's to make and changes the result (one approach or another, the scope, what to do with what already exists), ask them with Termo's ask_user tool instead of assuming. For everything else, decide yourself and go on.".into(),
        (OrchestrationClarifications, Es) => "Aclaraciones del usuario:".into(),
        (OrchestrationClarifications, En) => "The user's clarifications:".into(),
        (OrchestrationServiceBlocked(service, model, message), Es) => format!(
            "En pausa: {service} rechazó el pedido con el modelo {model}, por la cuenta o el servicio y no por la tarea. Cambiá el modelo del agente (Editar plan) o esperá a que se resuelva, y reanudá. Respuesta: {message}"
        ),
        (OrchestrationServiceBlocked(service, model, message), En) => format!(
            "Paused: {service} turned the request down with model {model}, because of the account or the service rather than the task. Change the agent's model (Edit plan) or wait for it to clear, then resume. Response: {message}"
        ),
        (OrchestrationQuestionClosed, Es) => "Esa pregunta ya no espera respuesta: el agente terminó o siguió sin ella.".into(),
        (OrchestrationQuestionClosed, En) => "That question isn't waiting any more: the agent finished or went on without it.".into(),
        (OrchestrationNoFolder(path), Es) => format!("La carpeta no existe: {path}"),
        (OrchestrationNoFolder(path), En) => format!("The folder doesn't exist: {path}"),
        (OrchestrationFolderRequired, Es) => "Elegí la carpeta del proyecto: los agentes trabajan ahí".into(),
        (OrchestrationFolderRequired, En) => "Choose the project's folder: the agents work there".into(),
        (OrchestrationNotFound, Es) => "La orquestación ya no existe".into(),
        (OrchestrationNotFound, En) => "The orchestration no longer exists".into(),
        (OrchestrationWrongState, Es) => "La orquestación ya no está en ese estado".into(),
        (OrchestrationWrongState, En) => "The orchestration is no longer in that state".into(),
        (OrchestrationEmptyPlan, Es) => "El plan no tiene tareas para ejecutar".into(),
        (OrchestrationEmptyPlan, En) => "The plan has no tasks to run".into(),
        (OrchestrationSaveFailed(error), Es) => format!("No se pudo guardar la orquestación: {error}"),
        (OrchestrationSaveFailed(error), En) => format!("Couldn't save the orchestration: {error}"),
        (OrchestrationStopped, Es) => "Detenida. Podés reanudarla cuando quieras.".into(),
        (OrchestrationStopped, En) => "Stopped. You can resume it whenever you like.".into(),
        (OrchestrationInterrupted, Es) => "Termo se cerró mientras corría. Podés reanudarla.".into(),
        (OrchestrationInterrupted, En) => "Termo was closed while it ran. You can resume it.".into(),
        (OrchestrationOverBudget(spent, budget), Es) => {
            format!("En pausa: lleva gastados {spent} y el presupuesto es {budget}. Subí el presupuesto para seguir.")
        }
        (OrchestrationOverBudget(spent, budget), En) => {
            format!("Paused: it has spent {spent} and the budget is {budget}. Raise the budget to go on.")
        }
        (OrchestrationBlocked, Es) => "Hay tareas que fallaron y nada más para ejecutar.".into(),
        (OrchestrationBlocked, En) => "Some tasks failed and there's nothing else to run.".into(),
        (OrchestrationNoReview(error), Es) => format!("El orquestador no pudo revisar el plan: {error}"),
        (OrchestrationNoReview(error), En) => format!("The orchestrator couldn't review the plan: {error}"),
        (OrchestrationAgentTask, Es) => "Hacé la tarea que te asigne el orquestador.".into(),
        (OrchestrationAgentTask, En) => "Do the task the orchestrator assigns you.".into(),
        (OrchestrationDoneWhen, Es) => "Está listo cuando:".into(),
        (OrchestrationDoneWhen, En) => "Done when:".into(),
        (OrchestrationBuildsOn, Es) => "Resultados de las tareas anteriores de las que depende esta:".into(),
        (OrchestrationBuildsOn, En) => "Results of the earlier tasks this one builds on:".into(),
        (OrchestrationLastAttempt, Es) => "Tu intento anterior de esta tarea falló así:".into(),
        (OrchestrationLastAttempt, En) => "Your previous attempt at this task failed like this:".into(),
        (AiModeOff, Es) => "Modo IA está apagado: activalo en Preferencias para usar los agentes".into(),
        (AiModeOff, En) => "AI Mode is off: turn it on in Preferences to use agents".into(),
        (AskFolderRunsPrograms, Es) => {
            "La configuración del repositorio de esta carpeta nombra programas que git ejecutaría al mirarla (filtros o drivers de diff), así que Termo no deja que un agente la mire por su cuenta".into()
        }
        (AskFolderRunsPrograms, En) => {
            "The config of this folder's repository names programs git would run to look at it (filters or diff drivers), so Termo doesn't let an agent look at it on its own".into()
        }

        (AgentConnectorBadName(name), En) => format!("\"{name}\" can't be a name: use letters, digits and _ (or - in a header)"),

        (GitNotFound, Es) => "No se encontró git en esta computadora".into(),
        (GitNotFound, En) => "git wasn't found on this computer".into(),
        (GitNotARepo, Es) => "Esa carpeta no es un repositorio de git".into(),
        (GitNotARepo, En) => "That folder isn't a git repository".into(),

        (HarnessInvalid(file, error), Es) => format!("No se pudo leer {file}: {error}"),
        (HarnessInvalid(file, error), En) => format!("Couldn't read {file}: {error}"),
        (HarnessWriteFailed(error), Es) => format!("No se pudieron escribir los archivos del harness: {error}"),
        (HarnessWriteFailed(error), En) => format!("Couldn't write the harness files: {error}"),
        (GitBusy, Es) => "Ya hay una acción de git en curso en este repositorio".into(),
        (GitBusy, En) => "A git action is already running on this repository".into(),
        (GitFailed(error), Es) => format!("Git no pudo hacerlo: {error}"),
        (GitFailed(error), En) => format!("Git couldn't do it: {error}"),
        (GitTimedOut, Es) => "Git tardó demasiado y se canceló. Revisá la conexión y probá de nuevo.".into(),
        (GitTimedOut, En) => "Git took too long and was cancelled. Check the connection and try again.".into(),
        (GitNeedsCredentials(said), Es) => {
            format!("Git necesita que te identifiques y acá no tiene dónde preguntar. Corré el comando una vez en la consola para guardar las credenciales.\n{said}")
        }
        (GitNeedsCredentials(said), En) => {
            format!("Git needs you to sign in and has nowhere to ask here. Run the command once in the console to save the credentials.\n{said}")
        }
        (GitCancelled, Es) => "Cancelado".into(),
        (GitCancelled, En) => "Cancelled".into(),
        (GitBranchNameInvalid(name), Es) => format!("\"{name}\" no sirve como nombre de rama"),
        (GitBranchNameInvalid(name), En) => format!("\"{name}\" can't be a branch's name"),
        (GitNoUpstream, Es) => "Esta rama todavía no está en el remoto: publicala primero".into(),
        (GitNoUpstream, En) => "This branch isn't on the remote yet: publish it first".into(),
        (GitNoRemote, Es) => "El repositorio no tiene ningún remoto configurado".into(),
        (GitNoRemote, En) => "The repository has no remote set up".into(),
        (GitDetached, Es) => "No estás en ninguna rama (HEAD suelto): cambiá a una rama primero".into(),
        (GitDetached, En) => "You aren't on a branch (detached HEAD): switch to one first".into(),
        (GitCommitNeedsMessage, Es) => "Escribí un mensaje para el commit".into(),
        (GitCommitNeedsMessage, En) => "Write a message for the commit".into(),
        (PrCliMissing(cli), Es) => format!("Hace falta tener instalado «{cli}»"),
        (PrCliMissing(cli), En) => format!("\"{cli}\" needs to be installed"),
        (PrNotSignedIn(cli), Es) => format!("Iniciá sesión con «{cli} auth login» en una consola"),
        (PrNotSignedIn(cli), En) => format!("Sign in with \"{cli} auth login\" in a console"),
        (PrUnreadable(cli), Es) => format!("No se entendió lo que respondió «{cli}»"),
        (PrUnreadable(cli), En) => format!("Couldn't make sense of what \"{cli}\" answered"),
        (PrFailed(error), Es) => format!("No se pudo: {error}"),
        (PrFailed(error), En) => format!("It couldn't be done: {error}"),
        (PrUnsupported, Es) => "Acá no hay cómo consultar los pull requests de este servicio".into(),
        (PrUnsupported, En) => "There's no way here to ask this service for its pull requests".into(),
        (PrNeedsTitle, Es) => "Ponele un título".into(),
        (PrNeedsTitle, En) => "Give it a title".into(),
        (GitReviewBusy, Es) => "Ya hay una revisión en curso en este repositorio".into(),
        (GitReviewBusy, En) => "There's already a review under way in this repository".into(),
        (GitReviewFailed(error), Es) => format!("La revisión no se pudo hacer: {error}"),
        (GitReviewFailed(error), En) => format!("The review couldn't be done: {error}"),
        (GitReviewNothing, Es) => "No hay cambios para revisar".into(),
        (GitReviewNothing, En) => "There are no changes to review".into(),
        (GitReviewCancelled, Es) => "Revisión cancelada".into(),
        (GitReviewCancelled, En) => "Review cancelled".into(),
        (GitReviewTimedOut, Es) => "La revisión tardó más de lo permitido. Se puede subir el tiempo en Preferencias → Git".into(),
        (GitReviewTimedOut, En) => "The review took longer than allowed. The time can be raised in Preferences → Git".into(),
        (GitReviewUnavailable, Es) => {
            "La configuración de este repositorio nombra programas que git ejecutaría al mirar los cambios (filtros o drivers de diff), así que Termo no lo hace revisar".into()
        }
        (GitReviewUnavailable, En) => {
            "This repository's config names programs git would run to look at changes (filters or diff drivers), so Termo doesn't have it reviewed".into()
        }
        (GitReviewNoBase, Es) => "No se encontró una rama base contra la cual revisar".into(),
        (GitReviewNoBase, En) => "No base branch was found to review against".into(),
        (GitReviewGone, Es) => "Esa revisión ya no está".into(),
        (GitReviewGone, En) => "That review is no longer there".into(),
        (GitReviewFixNeedsEdit, Es) => "Ese agente solo puede mirar: para arreglar hace falta uno que pueda cambiar archivos".into(),
        (GitReviewFixNeedsEdit, En) => "That agent can only look: fixing takes one that can change files".into(),
        (GitReviewAgentGone, Es) => "El agente elegido para arreglar ya no está, o dejó de ser un arreglador. Elegí otro en Preferencias → Git".into(),
        (GitReviewAgentGone, En) => "The agent picked to fix is gone, or isn't a fixer any more. Pick another in Preferences → Git".into(),
        (GitReviewNoPr, Es) => "Esta rama no tiene un pull request abierto".into(),
        (GitReviewNoPr, En) => "This branch has no open pull request".into(),
        (GitReviewCommentTitle, Es) => "Revisión de código (Termo)".into(),
        (GitReviewCommentTitle, En) => "Code review (Termo)".into(),
        (GitReviewSerious, Es) => "Grave".into(),
        (GitReviewSerious, En) => "Serious".into(),
        (GitReviewWarning, Es) => "Aviso".into(),
        (GitReviewWarning, En) => "Warning".into(),
        (GitReviewSuggestion, Es) => "Sugerencia".into(),
        (GitReviewSuggestion, En) => "Suggestion".into(),
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
            Msg::GrokCliNotFound,
            Msg::NpmNotFound,
            Msg::AgentInstallBusy,
            Msg::AgentInstallCancelled,
            Msg::AgentInstallBadFolder("x"),
            Msg::AgentInstallFailed("x"),
            Msg::AgentInstalledNotFound("grok"),
            Msg::MarketplaceCommandFailed("exit 1"),
            Msg::InvalidComponentName("../x"),
            Msg::CodexHasNoPlugins,
            Msg::McpRegistryUnreachable("timeout"),
            Msg::McpCodexHeadersUnsupported,
            Msg::McpNameTaken("github", "claude"),
            Msg::SourceKeyMissing,
            Msg::SourceKeyInvalid,
            Msg::SourceNeedsNoKey,
            Msg::SourceKeyNotSaved("locked"),
            Msg::SmitheryKeyRejected,
            Msg::SmitheryNotFound,
            Msg::SmitheryRateLimited,
            Msg::SmitheryUnreachable("timeout"),
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
            Msg::AgentLaunchUnsafePath(r"C:\a&b\codex.cmd"),
            Msg::AgentFavoriteInvalid,
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
            Msg::AskFolderRunsPrograms,
            Msg::OrchestrationPlanFailed("timeout"),
            Msg::OrchestrationReportFailed("timeout"),
            Msg::OrchestrationCanAsk,
            Msg::OrchestrationClarifications,
            Msg::OrchestrationQuestionClosed,
            Msg::OrchestrationServiceBlocked("Anthropic", "m", "403"),
            Msg::OrchestrationNoFolder("/x"),
            Msg::OrchestrationFolderRequired,
            Msg::OrchestrationNotFound,
            Msg::OrchestrationWrongState,
            Msg::OrchestrationEmptyPlan,
            Msg::OrchestrationSaveFailed("x"),
            Msg::OrchestrationStopped,
            Msg::OrchestrationInterrupted,
            Msg::OrchestrationOverBudget("$2.00", "$1.00"),
            Msg::OrchestrationBlocked,
            Msg::OrchestrationNoReview("timeout"),
            Msg::OrchestrationAgentTask,
            Msg::OrchestrationDoneWhen,
            Msg::OrchestrationBuildsOn,
            Msg::OrchestrationLastAttempt,
            Msg::GitNotFound,
            Msg::GitNotARepo,
            Msg::HarnessInvalid(".termo/harness.json", "x"),
            Msg::HarnessWriteFailed("x"),
            Msg::GitBusy,
            Msg::GitFailed("exit 1"),
            Msg::GitTimedOut,
            Msg::GitNeedsCredentials("terminal prompts disabled"),
            Msg::GitCancelled,
            Msg::GitBranchNameInvalid("-x"),
            Msg::GitNoUpstream,
            Msg::GitNoRemote,
            Msg::GitDetached,
            Msg::GitCommitNeedsMessage,
            Msg::PrCliMissing("gh"),
            Msg::PrNotSignedIn("gh"),
            Msg::PrUnreadable("gh"),
            Msg::PrFailed("x"),
            Msg::PrUnsupported,
            Msg::PrNeedsTitle,
            Msg::GitReviewBusy,
            Msg::GitReviewFailed("x"),
            Msg::GitReviewNothing,
            Msg::GitReviewCancelled,
            Msg::GitReviewTimedOut,
            Msg::GitReviewUnavailable,
            Msg::GitReviewNoBase,
            Msg::GitReviewGone,
            Msg::GitReviewFixNeedsEdit,
            Msg::GitReviewAgentGone,
            Msg::GitReviewNoPr,
            Msg::GitReviewCommentTitle,
            Msg::GitReviewSerious,
            Msg::GitReviewWarning,
            Msg::GitReviewSuggestion,
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
