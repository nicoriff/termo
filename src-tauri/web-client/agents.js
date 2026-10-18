// The agents on the computer the consoles run on, from here: what they're
// doing, what they wait on an answer for, each one's runs and what a run
// did, step by step as it happens. Running one takes full access; creating
// or changing one, also the user's say-so in Termo's Preferences there. It
// asks the sessions daemon (src-tauri/src/mux/agents.rs), which hands what
// only whoever runs the agents can do to Termo, or to the agents' own
// process when Termo is closed. Loaded after app.js, whose parts it uses.
"use strict";

const AGENT_WORDS = {
  es: {
    consolesPart: "Consolas",
    agentsPart: "Agentes",
    agentsHeading: "Tus agentes",
    newAgent: "Nuevo agente",
    noAgents: "Todavía no hay agentes en esta computadora.",
    aiModeOff: "El Modo IA está apagado en Termo en esta computadora: los agentes no corren.",
    noRuntime:
      "Ahora nadie está corriendo los agentes: abrí Termo en esta computadora, o activá \"Seguir corriendo los agentes con Termo cerrado\" en sus Preferencias.",
    editOffHint:
      "Para crear o cambiar agentes desde acá, activá \"Permitir crear y editar agentes desde ahí\" en Termo → Preferencias → Acceso remoto.",
    readOnlyAgents: "Con solo lectura podés ver los agentes, no correrlos.",
    running: "Corriendo",
    success: "Terminó bien",
    error: "Falló",
    stopped: "Detenida",
    timedOut: "Se pasó del tiempo",
    paused: "Pausado",
    neverRan: "Nunca corrió",
    lastRun: "{status} {when}",
    nextRun: "Próxima {when}",
    runNow: "Correr ahora",
    stop: "Detener",
    starting: "Arrancando…",
    addContext: "Agregar indicaciones para esta corrida",
    contextPlaceholder: "Opcional: algo para tener en cuenta solo esta vez",
    edit: "Editar",
    duplicate: "Duplicar",
    copyOf: "Copia de {name}",
    delete: "Borrar",
    deleteConfirm: "¿Borrar {name}? Se borra también su historial.",
    enabled: "Activado",
    enabledHint: "Apagado, no corre solo (por horario ni por eventos); a mano sí.",
    engine: "Motor",
    model: "Modelo",
    modelDefault: "El de siempre (el que usa el CLI por defecto)",
    modelUnlisted: "{model} (no está en la lista actual)",
    notInstalled: "{name} (no instalado)",
    folder: "Carpeta",
    ownFolder: "Carpeta propia del agente",
    autonomy: "Cuánto puede hacer solo",
    autonomyHint: "Mientras corre nadie le aprueba nada: lo que no esté permitido acá, simplemente no lo puede hacer.",
    read: "Solo mirar",
    readText: "Lee y busca archivos y consulta git. No cambia nada.",
    editLevel: "Editar archivos",
    editText: "Además puede crear y modificar archivos en su carpeta.",
    runLevel: "Correr comandos",
    runText: "Además puede ejecutar comandos. Dale este nivel solo si lo necesita.",
    schedule: "Horario",
    noSchedule: "Solo a mano",
    everyMinutes: "Cada {n} min",
    everyHours: "Cada {n} h",
    dailyAt: "Todos los días a las {at}",
    daysAt: "{days} a las {at}",
    alsoRuns: "También corre",
    triggerWatch: "cuando cambia una carpeta",
    triggerGit: "con git",
    triggerWebhook: "por webhook",
    triggerAfter: "después de otro agente",
    runs: "Corridas",
    noRuns: "Sin corridas todavía",
    loading: "Cargando…",
    triggerManual: "A mano",
    triggerTest: "Prueba",
    triggerSchedule: "Programada",
    triggerWatchRun: "Por cambios en la carpeta",
    triggerGitRun: "Por git",
    triggerWebhookRun: "Por webhook",
    triggerChain: "Después de otro agente",
    triggerAgent: "Pedida por otro agente",
    triggerMessage: "Mensaje de otro agente",
    triggerOrchestrator: "Tarea de una orquestación",
    triggerConsole: "Pedido desde una consola",
    runWord: "Corrida",
    runTitle: "Corrida · {when}",
    took: "duró {time}",
    cost: "costo {value}",
    summary: "Resultado",
    log: "Lo que hizo",
    earlierSteps: "{count} pasos anteriores no se muestran",
    noSteps: "Todavía no hay pasos.",
    question: "Te pregunta",
    answerPlaceholder: "Tu respuesta",
    send: "Enviar",
    answered: "Respondiste",
    questionGone: "La pregunta ya no esperaba respuesta.",
    decidesAlone: "Si no respondés antes de las {time}, decide solo.",
    newAgentTitle: "Nuevo agente",
    editTitle: "Editar {name}",
    name: "Nombre",
    namePlaceholder: "Por ejemplo: Revisor de commits",
    task: "Qué hace en cada corrida",
    taskPlaceholder: "La tarea, como se la dirías a alguien",
    instructions: "Instrucciones",
    instructionsHint: "Opcional: los pasos que sigue, sus reglas, cuándo terminó.",
    moreOptions: "Más opciones",
    web: "Buscar y leer en internet",
    webHint: "Para agentes que necesitan información actual: documentación, noticias, precios.",
    timeout: "Límite de tiempo (minutos)",
    scheduleNone: "Sin horario",
    scheduleInterval: "Cada tantos minutos",
    scheduleDaily: "A una hora fija",
    minutes: "Minutos",
    at: "Hora",
    everyDay: "Todos los días",
    save: "Guardar",
    create: "Crear",
    cancel: "Cancelar",
    saving: "Guardando…",
    advancedNote: "Este agente tiene opciones que solo se cambian en Termo ({list}). Se mantienen como están.",
    advWatch: "carpeta vigilada",
    advGit: "git",
    advWebhook: "webhook",
    advAfter: "después de otro agente",
    advConnectors: "integraciones",
    advTeam: "otros agentes",
    advDeploy: "GitHub",
    advProject: "proyecto",
    failed: "No se pudo: {reason}",
    errNoRuntime: "Ahora nadie está corriendo los agentes en esta computadora.",
    errNotAllowed: "Con tu acceso no se puede.",
    errEditOff: "Crear y cambiar agentes desde acá está apagado en Termo.",
    errNotFound: "Ese agente ya no está.",
    errBadName: "Ponele un nombre.",
    errBadFolder: "Esa carpeta no existe en esta computadora.",
    errBadSchedule: "Ese horario no sirve.",
    errTooLong: "El texto es demasiado largo.",
    errTimeout: "No contestó a tiempo.",
    errBusy: "Está ocupado; probá de nuevo en un momento.",
    errNeedsApp: "Este agente corre con git: para cambiarlo, Termo tiene que estar abierto en esta computadora.",
    errBadArguments: "Algún dato del formulario no sirve.",
  },
  en: {
    consolesPart: "Consoles",
    agentsPart: "Agents",
    agentsHeading: "Your agents",
    newAgent: "New agent",
    noAgents: "There are no agents on this computer yet.",
    aiModeOff: "AI Mode is off in Termo on this computer: agents don't run.",
    noRuntime:
      "Nobody is running the agents now: open Termo on this computer, or turn on \"Keep agents running when Termo closes\" in its Preferences.",
    editOffHint:
      "To create or change agents from here, turn on \"Allow creating and editing agents from there\" in Termo → Preferences → Remote access.",
    readOnlyAgents: "With read-only access you can see the agents, not run them.",
    running: "Running",
    success: "Finished fine",
    error: "Failed",
    stopped: "Stopped",
    timedOut: "Ran out of time",
    paused: "Paused",
    neverRan: "Never ran",
    lastRun: "{status} {when}",
    nextRun: "Next {when}",
    runNow: "Run now",
    stop: "Stop",
    starting: "Starting…",
    addContext: "Add instructions for this run",
    contextPlaceholder: "Optional: something to keep in mind just this time",
    edit: "Edit",
    duplicate: "Duplicate",
    copyOf: "Copy of {name}",
    delete: "Delete",
    deleteConfirm: "Delete {name}? Its history goes too.",
    enabled: "On",
    enabledHint: "Off, it doesn't run on its own (on a schedule or on events); by hand it still does.",
    engine: "Engine",
    model: "Model",
    modelDefault: "The usual one (the CLI's default)",
    modelUnlisted: "{model} (not in the current list)",
    notInstalled: "{name} (not installed)",
    folder: "Folder",
    ownFolder: "The agent's own folder",
    autonomy: "How much it can do on its own",
    autonomyHint: "Nobody approves anything while it runs: whatever isn't allowed here, it just can't do.",
    read: "Only look",
    readText: "Reads and searches files and looks things up in git. Changes nothing.",
    editLevel: "Edit files",
    editText: "Can also create and modify files in its folder.",
    runLevel: "Run commands",
    runText: "Can also run commands. Only give it this level if it needs it.",
    schedule: "Schedule",
    noSchedule: "By hand only",
    everyMinutes: "Every {n} min",
    everyHours: "Every {n} h",
    dailyAt: "Every day at {at}",
    daysAt: "{days} at {at}",
    alsoRuns: "Also runs",
    triggerWatch: "when a folder changes",
    triggerGit: "on git",
    triggerWebhook: "on a webhook",
    triggerAfter: "after another agent",
    runs: "Runs",
    noRuns: "No runs yet",
    loading: "Loading…",
    triggerManual: "By hand",
    triggerTest: "Test",
    triggerSchedule: "Scheduled",
    triggerWatchRun: "Folder changed",
    triggerGitRun: "Git",
    triggerWebhookRun: "Webhook",
    triggerChain: "After another agent",
    triggerAgent: "Asked by another agent",
    triggerMessage: "Message from another agent",
    triggerOrchestrator: "A task of an orchestration",
    triggerConsole: "Asked from a console",
    runWord: "Run",
    runTitle: "Run · {when}",
    took: "took {time}",
    cost: "cost {value}",
    summary: "Result",
    log: "What it did",
    earlierSteps: "{count} earlier steps aren't shown",
    noSteps: "No steps yet.",
    question: "It asks you",
    answerPlaceholder: "Your answer",
    send: "Send",
    answered: "You answered",
    questionGone: "The question wasn't waiting for an answer any more.",
    decidesAlone: "If you don't answer by {time}, it decides alone.",
    newAgentTitle: "New agent",
    editTitle: "Edit {name}",
    name: "Name",
    namePlaceholder: "For example: Commit reviewer",
    task: "What it does on each run",
    taskPlaceholder: "The task, as you'd tell someone",
    instructions: "Instructions",
    instructionsHint: "Optional: the steps it follows, its rules, when it's done.",
    moreOptions: "More options",
    web: "Search and read the web",
    webHint: "For agents that need current information: documentation, news, prices.",
    timeout: "Time limit (minutes)",
    scheduleNone: "No schedule",
    scheduleInterval: "Every so many minutes",
    scheduleDaily: "At a set time",
    minutes: "Minutes",
    at: "Time",
    everyDay: "Every day",
    save: "Save",
    create: "Create",
    cancel: "Cancel",
    saving: "Saving…",
    advancedNote: "This agent has options only Termo changes ({list}). They're kept as they are.",
    advWatch: "watched folder",
    advGit: "git",
    advWebhook: "webhook",
    advAfter: "after another agent",
    advConnectors: "integrations",
    advTeam: "other agents",
    advDeploy: "GitHub",
    advProject: "project",
    failed: "Couldn't: {reason}",
    errNoRuntime: "Nobody is running the agents on this computer now.",
    errNotAllowed: "Your access doesn't allow it.",
    errEditOff: "Creating and changing agents from here is off in Termo.",
    errNotFound: "That agent isn't there any more.",
    errBadName: "Give it a name.",
    errBadFolder: "That folder doesn't exist on this computer.",
    errBadSchedule: "That schedule won't do.",
    errTooLong: "The text is too long.",
    errTimeout: "It didn't answer in time.",
    errBusy: "It's busy; try again in a moment.",
    errNeedsApp: "This agent runs on git: to change it, Termo has to be open on this computer.",
    errBadArguments: "Something in the form won't do.",
  },
};

function at(key, vars = {}) {
  let text = AGENT_WORDS[lang][key] ?? AGENT_WORDS.en[key] ?? key;
  for (const [name, value] of Object.entries(vars)) text = text.replace(`{${name}}`, value);
  return text;
}

for (const language of ["es", "en"]) {
  for (const key of ["consolesPart", "agentsPart", "agentsHeading", "newAgent", "editOffHint"]) WORDS[language][key] = AGENT_WORDS[language][key];
}

// The page's words are in now: the ones in its markup again, these included.
for (const element of document.querySelectorAll("[data-text]")) element.textContent = t(element.dataset.text);
for (const element of document.querySelectorAll("[data-label]")) {
  element.setAttribute("aria-label", t(element.dataset.label));
  element.title = t(element.dataset.label);
}

const agentsUi = (() => {
  const partsBar = $("parts");
  const consolesPart = $("consoles-part");
  const agentsPart = $("agents-part");
  const showConsolesButton = $("show-consoles");
  const showAgentsButton = $("show-agents");
  const note = $("agents-note");
  const questionsBox = $("agent-questions");
  const list = $("agent-list");
  const newAgentButton = $("new-agent");
  const editOffLine = $("agents-edit-off");
  const panel = $("agent-panel");
  const panelTitle = $("agent-panel-title");
  const panelActions = $("agent-panel-actions");
  const panelBody = $("agent-panel-body");
  const panelFoot = $("agent-panel-foot");

  /** How often the list is asked again while it's shown: a schedule's
   * next run, a run that started on its own. Events say the rest. */
  const OVERVIEW_EVERY_MS = 5000;
  /** A run's steps kept on the page, as many as the daemon sends at first. */
  const MAX_LOG_EVENTS = 400;
  /** Minutes the form sends at most: whole ones that fit what the daemon reads. */
  const MAX_MINUTES = 1_000_000_000;
  const ENGINES = [
    ["claude", "Claude Code"],
    ["codex", "Codex"],
    ["gemini", "Gemini CLI"],
    ["copilot", "Copilot"],
    ["grok", "Grok"],
  ];
  const STATUS_WORDS = { running: "running", success: "success", error: "error", stopped: "stopped", timedOut: "timedOut" };
  const TRIGGER_WORDS = {
    manual: "triggerManual",
    test: "triggerTest",
    schedule: "triggerSchedule",
    watch: "triggerWatchRun",
    git: "triggerGitRun",
    webhook: "triggerWebhookRun",
    chain: "triggerChain",
    agent: "triggerAgent",
    message: "triggerMessage",
    orchestrator: "triggerOrchestrator",
    console: "triggerConsole",
  };
  const ERRORS = {
    "no-runtime": "errNoRuntime",
    "no-agents": "errNoRuntime",
    "not-allowed": "errNotAllowed",
    "not allowed": "errNotAllowed",
    "edit-off": "errEditOff",
    "not-found": "errNotFound",
    "bad-name": "errBadName",
    "bad-folder": "errBadFolder",
    "bad-schedule": "errBadSchedule",
    "too-long": "errTooLong",
    timeout: "errTimeout",
    busy: "errBusy",
    "needs-app": "errNeedsApp",
    "bad-arguments": "errBadArguments",
  };

  let part = remembered.get("part") === "agents" ? "agents" : "consoles";
  /** What the daemon said last: the agents and what can be done here. */
  let overview = null;
  let overviewTimer = null;
  let watching = false;
  /** The panel's views, the one shown last: {kind: "agent"|"run"|"form", …}. */
  let stack = [];
  /** Each agent's runs, once asked, by agent id. */
  const runsOf = new Map();
  /** A run's steps, once asked, by run id: {events, skipped}. */
  const logs = new Map();
  /** What the run box holds for each agent while typed into. */
  const contexts = new Map();
  /** Agents being started or stopped from here, by id. */
  const busy = new Set();

  function errorText(err) {
    const code = err?.message ?? String(err);
    return ERRORS[code] ? at(ERRORS[code]) : at("failed", { reason: code });
  }

  function call(name, args = {}) {
    if (!connection) return Promise.reject(new Error("closed"));
    return connection.request({ op: "agents", call: name, args }).then((reply) => reply.value);
  }

  function el(tag, className, text) {
    const element = document.createElement(tag);
    if (className) element.className = className;
    if (text !== undefined && text !== null) element.textContent = text;
    return element;
  }

  function button(className, text, onclick) {
    const element = el("button", className, text);
    element.type = "button";
    element.onclick = onclick;
    return element;
  }

  function agentById(id) {
    return overview?.agents.find((agent) => agent.id === id) ?? null;
  }

  function canRun() {
    return Boolean(overview?.canRun);
  }

  function canEdit() {
    return Boolean(overview?.canEdit);
  }

  // ---- Time, said the way the console list says it.

  function ago(ms) {
    return since(ms / 1000);
  }

  function until(ms) {
    const minutes = Math.max(0, Math.round((ms - Date.now()) / 60000));
    const format = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
    if (minutes < 60) return format.format(minutes, "minute");
    if (minutes < 60 * 24) return format.format(Math.round(minutes / 60), "hour");
    return format.format(Math.round(minutes / 60 / 24), "day");
  }

  function duration(ms) {
    const seconds = Math.max(0, Math.round(ms / 1000));
    if (seconds < 60) return `${seconds} s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes} min ${seconds % 60} s`;
    return `${Math.floor(minutes / 60)} h ${minutes % 60} min`;
  }

  function clock(ms) {
    return new Intl.DateTimeFormat(lang, { hour: "2-digit", minute: "2-digit" }).format(new Date(ms));
  }

  function money(usd) {
    return usd > 0 && usd < 0.01 ? "<$0.01" : `$${usd.toFixed(2)}`;
  }

  /** Weekday `day` (0 = Sunday), short, in the page's language. */
  function weekday(day) {
    // 2024-01-07 was a Sunday.
    return new Intl.DateTimeFormat(lang, { weekday: "short" }).format(new Date(2024, 0, 7 + day));
  }

  function scheduleText(schedule) {
    if (!schedule) return at("noSchedule");
    if (schedule.kind === "interval") {
      const minutes = schedule.minutes;
      return minutes % 60 === 0 ? at("everyHours", { n: minutes / 60 }) : at("everyMinutes", { n: minutes });
    }
    const days = schedule.weekdays ?? [];
    if (!days.length || days.length === 7) return at("dailyAt", { at: schedule.at });
    return at("daysAt", { days: [...days].sort().map(weekday).join(", "), at: schedule.at });
  }

  function engineName(id) {
    return ENGINES.find(([engine]) => engine === id)?.[1] ?? id;
  }

  /** The run each agent is on now, or its last one. */
  function currentRun(agent) {
    const live = (overview?.running ?? []).find((run) => run.agentId === agent.id);
    return live ?? agent.lastRun ?? null;
  }

  function stateLine(agent) {
    const run = currentRun(agent);
    if (run?.status === "running") return run.activity ? `${at("running")} · ${run.activity}` : at("running");
    const parts = [];
    if (!agent.enabled) parts.push(at("paused"));
    if (run) parts.push(at("lastRun", { status: at(STATUS_WORDS[run.status] ?? "error"), when: ago(run.endedAt ?? run.startedAt) }));
    else parts.push(at("neverRan"));
    if (agent.nextRun) parts.push(at("nextRun", { when: until(agent.nextRun) }));
    return parts.join(" · ");
  }

  /** The start of its last run's answer, on one line; none while it runs. */
  function lastAnswer(agent) {
    const run = currentRun(agent);
    if (!run || run.status === "running" || !run.summary) return "";
    return run.summary.replace(/[*_`#>]+/g, "").replace(/\s+/g, " ").trim();
  }

  function statusClass(run) {
    if (!run) return "idle";
    return { running: "running", success: "ok", error: "bad", timedOut: "bad", stopped: "idle" }[run.status] ?? "idle";
  }

  /** The agent's emoji, in the console list's icon box. */
  function agentIcon(agent, size = "") {
    const box = el("span", `session-kind agent-icon ${size}`.trim(), agent.icon || "🤖");
    box.setAttribute("aria-hidden", "true");
    return box;
  }

  // ---- The part of the menu that lists them.

  function shown() {
    return part === "agents" && Boolean(connection);
  }

  function setPart(next) {
    part = next;
    remembered.set("part", next);
    renderPart();
    render();
    if (next === "agents") void refresh();
  }

  function renderPart() {
    partsBar.hidden = !connection;
    const agents = shown();
    consolesPart.hidden = agents;
    agentsPart.hidden = !agents;
    showConsolesButton.classList.toggle("on", !agents);
    showAgentsButton.classList.toggle("on", agents);
    showConsolesButton.setAttribute("aria-selected", String(!agents));
    showAgentsButton.setAttribute("aria-selected", String(agents));
    clearInterval(overviewTimer);
    overviewTimer = agents || !panel.hidden ? setInterval(() => void refresh(), OVERVIEW_EVERY_MS) : null;
  }

  showConsolesButton.onclick = () => setPart("consoles");
  showAgentsButton.onclick = () => setPart("agents");

  /** One overview at a time: a slow answer (Termo busy, the agents'
   * process restarting) mustn't pile up a new one every few seconds. */
  let refreshing = false;

  async function refresh() {
    if (!connection || refreshing) return;
    refreshing = true;
    try {
      overview = await call("overview");
    } catch {
      return;
    } finally {
      refreshing = false;
    }
    if (!watching) {
      watching = true;
      call("watch").catch(() => (watching = false));
    }
    renderList();
    renderPanel({ keepForm: true });
  }

  function renderList() {
    if (!overview) return;
    note.textContent = !overview.aiMode
      ? at("aiModeOff")
      : !overview.runtime
        ? at("noRuntime")
        : readOnly()
          ? at("readOnlyAgents")
          : "";
    newAgentButton.hidden = !canEdit();
    editOffLine.hidden = !overview.editOff;
    if (!typingIn(questionsBox)) {
      questionsBox.replaceChildren(...(overview.questions ?? []).map((question) => questionCard(question, true)));
    }
    if (!overview.agents.length) {
      list.replaceChildren(el("li", "agents-empty", at("noAgents")));
      return;
    }
    list.replaceChildren(
      ...overview.agents.map((agent) => {
        const item = el("li", "session-item");
        const row = button("session agent-row", null, () => openAgent(agent.id));
        const text = el("span", "session-text");
        text.append(el("span", "session-title", agent.name));
        const state = el("span", "session-detail agent-state", stateLine(agent));
        text.append(state);
        // What it answered last: a run can finish fine with the agent
        // saying it couldn't do it.
        const answer = lastAnswer(agent);
        if (answer) text.append(el("span", "session-detail agent-answer", answer));
        text.append(el("span", "session-detail", [engineName(agent.engine), agent.model].filter(Boolean).join(" · ")));
        const dot = el("span", `run-dot ${statusClass(currentRun(agent))}`);
        dot.setAttribute("aria-hidden", "true");
        row.append(agentIcon(agent), text, dot);
        row.classList.toggle("paused", !agent.enabled);
        item.append(row);
        return item;
      }),
    );
  }

  newAgentButton.onclick = () => openForm(null);

  // ---- What a run asks.

  /** A question waiting on the user: what the agent asks, its suggested
   * answers, and a box for another. */
  function questionCard(question, withAgent) {
    const card = el("div", "question-card");
    const head = el("div", "question-head");
    const agent = agentById(question.agentId);
    head.append(el("span", "question-label", withAgent && agent ? `${agent.icon || "🤖"} ${agent.name} · ${at("question")}` : at("question")));
    card.append(head, el("p", "question-text", question.question));
    if (question.expiresAt) card.append(el("p", "hint", at("decidesAlone", { time: clock(question.expiresAt) })));
    if (!canRun()) return card;
    const answer = async (text, controls) => {
      if (!text.trim()) return;
      for (const control of controls) control.disabled = true;
      try {
        const taken = await call("answer", { questionId: question.id, answer: text });
        if (taken === false) card.append(el("p", "hint", at("questionGone")));
      } catch (err) {
        card.append(el("p", "hint error-text", errorText(err)));
        for (const control of controls) control.disabled = false;
        return;
      }
      void refresh();
    };
    const controls = [];
    if (question.options?.length) {
      const options = el("div", "question-options");
      for (const option of question.options) {
        const choice = button("pill compact", option, () => void answer(option, controls));
        controls.push(choice);
        options.append(choice);
      }
      card.append(options);
    }
    const form = el("form", "question-form");
    const input = el("input");
    input.placeholder = at("answerPlaceholder");
    input.setAttribute("aria-label", at("answerPlaceholder"));
    const send = el("button", "pill primary compact", at("send"));
    controls.push(input, send);
    form.append(input, send);
    form.onsubmit = (event) => {
      event.preventDefault();
      void answer(input.value, controls);
    };
    card.append(form);
    return card;
  }

  // ---- The panel: an agent, a run, or the form.

  function openAgent(id) {
    stack = [{ kind: "agent", id }];
    showPanel();
    void loadRuns(id);
  }

  function openRun(agentId, runId) {
    stack.push({ kind: "run", agentId, runId });
    showPanel();
    void loadLog(agentId, runId);
  }

  async function openForm(id, { copy = false } = {}) {
    const view = { kind: "form", id: copy ? null : id, form: null, advanced: [], models: null, saving: false, error: "" };
    stack.push(view);
    if (!id) {
      view.form = blankForm();
      showPanel();
    } else {
      showPanel();
      try {
        const reply = await call("agent", { id });
        view.form = { ...reply.form };
        view.advanced = reply.advanced ?? [];
        if (copy) {
          view.form.id = "";
          view.form.name = at("copyOf", { name: view.form.name });
          view.advanced = [];
        }
      } catch (err) {
        view.error = errorText(err);
        view.form = blankForm();
      }
      if (stack[stack.length - 1] === view) renderPanel();
    }
    void loadModels(view);
  }

  function blankForm() {
    const installed = overview?.engines;
    const engine = installed?.length ? (installed.includes("claude") ? "claude" : installed[0]) : "claude";
    return {
      id: "",
      name: "",
      engine,
      model: "",
      workDir: "",
      task: "",
      instructions: "",
      autonomy: "read",
      web: false,
      schedule: null,
      timeoutMin: 15,
      enabled: true,
    };
  }

  function showPanel() {
    panel.hidden = false;
    menuOpen = false;
    render();
    renderPart();
    renderPanel();
  }

  function back() {
    stack.pop();
    if (!stack.length) {
      panel.hidden = true;
      renderPart();
      return;
    }
    renderPanel();
    const top = stack[stack.length - 1];
    if (top.kind === "agent") void loadRuns(top.id);
  }

  $("agent-back").onclick = () => back();
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && folderView.hidden && !panel.hidden) back();
  });

  async function loadRuns(agentId) {
    try {
      runsOf.set(agentId, await call("runs", { agentId }));
    } catch {
      if (!runsOf.has(agentId)) runsOf.set(agentId, []);
    }
    const top = stack[stack.length - 1];
    if (top?.kind === "agent" && top.id === agentId) renderPanel();
  }

  async function loadLog(agentId, runId) {
    try {
      logs.set(runId, await call("run_log", { agentId, runId }));
    } catch {
      if (!logs.has(runId)) logs.set(runId, { events: [], skipped: 0 });
    }
    const top = stack[stack.length - 1];
    if (top?.kind === "run" && top.runId === runId) renderPanel();
  }

  async function loadModels(view) {
    if (!view.form) return;
    const engine = view.form.engine;
    try {
      const models = await call("models", { engine });
      if (view.form.engine === engine) view.models = models;
    } catch {
      view.models = [];
    }
    if (stack[stack.length - 1] === view) renderPanel({ keepForm: true });
  }

  /** Whether something in `box` is being typed into: drawing it again now
   * would take the keyboard away, and what's half typed. */
  function typingIn(box) {
    const focused = document.activeElement;
    return Boolean(focused) && box.contains(focused) && ["INPUT", "TEXTAREA", "SELECT"].includes(focused.tagName);
  }

  /** Draws the view on top. One drawn because something changed elsewhere
   * (`keepForm`: a refresh, an event) leaves a form, and whatever is being
   * typed into, as they are. */
  function renderPanel({ keepForm = false } = {}) {
    if (panel.hidden) return;
    const view = stack[stack.length - 1];
    if (!view) return;
    if (view.kind === "form" && keepForm && view.drawn) return renderModelChoices(view);
    if (keepForm && typingIn(panelBody)) return;
    panelActions.replaceChildren();
    panelFoot.replaceChildren();
    panelFoot.hidden = true;
    if (view.kind === "agent") renderAgent(view);
    else if (view.kind === "run") renderRun(view);
    else renderForm(view);
  }

  // ---- An agent.

  function renderAgent(view) {
    const agent = agentById(view.id);
    if (!agent) {
      panelTitle.textContent = "";
      panelBody.replaceChildren(el("p", "status", at("errNotFound")));
      return;
    }
    panelTitle.replaceChildren(el("span", "title-icon", agent.icon || "🤖"), el("span", "title-text", agent.name));
    if (canEdit()) panelActions.append(button("pill compact", at("edit"), () => void openForm(agent.id)));

    const body = [];
    if (agent.description) body.push(el("p", "agent-description", agent.description));

    // Running it, or stopping it.
    const run = currentRun(agent);
    const live = run?.status === "running";
    const card = el("div", "run-card");
    const state = el("div", "run-state");
    const dot = el("span", `run-dot ${statusClass(run)}`);
    state.append(dot, el("span", "run-state-text", stateLine(agent)));
    card.append(state);
    const answer = lastAnswer(agent);
    if (answer) card.append(el("p", "card-answer", answer));
    if (canRun()) {
      if (live) {
        const stop = button("pill danger wide", busy.has(agent.id) ? at("stopped") : at("stop"), () => void stopAgent(agent.id));
        stop.disabled = busy.has(agent.id);
        card.append(stop);
      } else {
        const details = el("details", "context-box");
        details.open = Boolean(contexts.get(agent.id));
        details.append(el("summary", null, at("addContext")));
        const box = el("textarea");
        box.rows = 3;
        box.placeholder = at("contextPlaceholder");
        box.value = contexts.get(agent.id) ?? "";
        box.oninput = () => contexts.set(agent.id, box.value);
        details.append(box);
        const start = button("pill primary wide", busy.has(agent.id) ? at("starting") : at("runNow"), () => void startAgent(agent.id));
        start.disabled = busy.has(agent.id);
        card.append(details, start);
      }
    }
    body.push(card);
    view.error && body.push(el("p", "status error-text", view.error));

    const question = (overview?.questions ?? []).find((entry) => entry.agentId === agent.id);
    if (question) body.push(questionCard(question, false));

    // What it is.
    const facts = el("dl", "facts");
    const fact = (label, value, title) => {
      const dt = el("dt", null, label);
      const dd = el("dd", null, value);
      if (title) dd.title = title;
      facts.append(dt, dd);
    };
    fact(at("engine"), [engineName(agent.engine), agent.model].filter(Boolean).join(" · "));
    fact(at("folder"), agent.workDir ? shortPath(agent.workDir) : at("ownFolder"), agent.workDir);
    fact(at("autonomy"), at({ read: "read", edit: "editLevel", run: "runLevel" }[agent.autonomy] ?? "read"));
    fact(at("schedule"), scheduleText(agent.schedule) + (agent.nextRun ? ` · ${at("nextRun", { when: until(agent.nextRun) })}` : ""));
    if (agent.triggers.length) {
      const words = { watch: "triggerWatch", git: "triggerGit", webhook: "triggerWebhook", after: "triggerAfter" };
      fact(at("alsoRuns"), agent.triggers.map((name) => at(words[name] ?? name)).join(", "));
    }
    body.push(facts);

    if (canEdit()) {
      const toggle = el("label", "toggle-line");
      const text = el("span", "toggle-text");
      text.append(el("span", null, at("enabled")), el("span", "hint", at("enabledHint")));
      const input = el("input");
      input.type = "checkbox";
      input.checked = agent.enabled;
      input.onchange = async () => {
        input.disabled = true;
        try {
          await call("set_enabled", { id: agent.id, enabled: input.checked });
        } catch (err) {
          view.error = errorText(err);
        }
        await refresh();
      };
      toggle.append(text, input);
      body.push(toggle);
    }

    // Its runs.
    body.push(el("h3", "home-heading panel-heading", at("runs")));
    const runs = mergedRuns(agent);
    if (!runsOf.has(agent.id)) body.push(el("p", "hint", at("loading")));
    else if (!runs.length) body.push(el("p", "hint", at("noRuns")));
    else {
      const items = el("ul", "run-list");
      for (const entry of runs) {
        const row = button("run-row", null, () => openRun(agent.id, entry.id));
        const runDot = el("span", `run-dot ${statusClass(entry)}`);
        const text = el("span", "run-row-text");
        text.append(el("span", "run-row-title", `${at(STATUS_WORDS[entry.status] ?? "error")} · ${at(TRIGGER_WORDS[entry.trigger] ?? "triggerManual")}`));
        const detail = [ago(entry.startedAt)];
        if (entry.endedAt) detail.push(at("took", { time: duration(entry.endedAt - entry.startedAt) }));
        if (entry.costUsd) detail.push(at("cost", { value: money(entry.costUsd) }));
        text.append(el("span", "run-row-detail", detail.join(" · ")));
        const said = entry.status === "running" ? entry.activity : entry.summary;
        if (said) text.append(el("span", "run-row-summary", said));
        row.append(runDot, text);
        const item = el("li");
        item.append(row);
        items.append(item);
      }
      body.push(items);
    }

    if (canEdit()) {
      const actions = el("div", "panel-bottom");
      actions.append(button("pill compact", at("duplicate"), () => void openForm(agent.id, { copy: true })));
      const remove = button("pill compact danger-text", at("delete"), () => void deleteAgent(agent));
      actions.append(remove);
      body.push(actions);
    }
    panelBody.replaceChildren(...body);
  }

  /** Its runs as last asked, with what events said since. */
  function mergedRuns(agent) {
    const runs = [...(runsOf.get(agent.id) ?? [])];
    const live = currentRun(agent);
    if (live && !runs.some((run) => run.id === live.id)) runs.unshift(live);
    return runs.map((run) => (live && run.id === live.id ? live : run));
  }

  async function startAgent(id) {
    const view = stack[stack.length - 1];
    busy.add(id);
    renderPanel();
    try {
      const run = await call("start", { agentId: id, context: contexts.get(id) || null });
      contexts.delete(id);
      if (run?.id) noteRun(run);
      if (view) view.error = "";
    } catch (err) {
      if (view) view.error = errorText(err);
    }
    busy.delete(id);
    await refresh();
  }

  async function stopAgent(id) {
    busy.add(id);
    renderPanel();
    try {
      await call("stop", { agentId: id });
    } catch (err) {
      const view = stack[stack.length - 1];
      if (view) view.error = errorText(err);
    }
    busy.delete(id);
    await refresh();
  }

  async function deleteAgent(agent) {
    if (!confirm(at("deleteConfirm", { name: agent.name }))) return;
    try {
      await call("delete", { id: agent.id });
    } catch (err) {
      alert(errorText(err));
      return;
    }
    stack = [];
    panel.hidden = true;
    renderPart();
    await refresh();
  }

  // ---- A run.

  function renderRun(view) {
    const agent = agentById(view.agentId);
    const run = (runsOf.get(view.agentId) ?? []).find((entry) => entry.id === view.runId) ??
      (agent && currentRun(agent)?.id === view.runId ? currentRun(agent) : null);
    panelTitle.textContent = run ? at("runTitle", { when: ago(run.startedAt) }) : at("runWord");
    const body = [];
    if (run) {
      const card = el("div", "run-card");
      const state = el("div", "run-state");
      const said = [at(STATUS_WORDS[run.status] ?? "error"), at(TRIGGER_WORDS[run.trigger] ?? "triggerManual")];
      if (run.endedAt) said.push(at("took", { time: duration(run.endedAt - run.startedAt) }));
      if (run.costUsd) said.push(at("cost", { value: money(run.costUsd) }));
      state.append(el("span", `run-dot ${statusClass(run)}`), el("span", "run-state-text", said.join(" · ")));
      card.append(state);
      if (run.status === "running" && run.activity) card.append(el("p", "hint", run.activity));
      if (run.status === "running" && canRun() && agent) {
        const stop = button("pill danger wide", at("stop"), () => void stopAgent(agent.id));
        stop.disabled = busy.has(agent.id);
        card.append(stop);
      }
      body.push(card);
      const question = (overview?.questions ?? []).find((entry) => entry.runId === run.id);
      if (question) body.push(questionCard(question, false));
      if (run.status !== "running" && run.summary) {
        body.push(el("h3", "home-heading panel-heading", at("summary")));
        body.push(el("div", `run-summary ${run.status === "success" ? "" : "bad"}`.trim(), run.summary));
      }
    }
    body.push(el("h3", "home-heading panel-heading", at("log")));
    const log = logs.get(view.runId);
    const steps = el("div", "steps");
    steps.id = "run-steps";
    if (!log) steps.append(el("p", "hint", at("loading")));
    else {
      if (log.skipped) {
        const note = el("p", "hint", at("earlierSteps", { count: log.skipped }));
        note.id = "run-skipped";
        steps.append(note);
      }
      for (const event of log.events) {
        const step = stepElement(event);
        if (step) steps.append(step);
      }
      if (!steps.children.length) steps.append(el("p", "hint", at("noSteps")));
    }
    body.push(steps);
    panelBody.replaceChildren(...body);
  }

  /** One step of a run, as the desktop's run view shows it; nothing for
   * what only the desktop shows (usage warnings, its own start and end). */
  function stepElement(event) {
    const text = event.text ?? "";
    switch (event.kind) {
      case "text":
        return text ? el("p", "step text", text) : null;
      case "result":
        return text ? el("div", "step result", text) : null;
      case "tool": {
        const step = el("div", "step tool");
        step.append(el("span", "step-tool", event.tool || "tool"));
        if (text) step.append(el("span", "step-tool-text", text));
        return step;
      }
      case "stderr":
        return text ? el("pre", "step stderr", text) : null;
      case "question":
        return el("div", "step asked", `${at("question")}: ${text}`);
      case "answer":
        return text ? el("div", "step answered", `${at("answered")}: ${text}`) : null;
      case "failure":
        return el("div", "step failure", text || event.failure?.message || at("error"));
      case "session": {
        const session = event.session ?? {};
        const parts = [engineName(session.engine), session.model, session.cwd].filter(Boolean);
        return parts.length ? el("p", "step meta", parts.join(" · ")) : null;
      }
      default:
        return null;
    }
  }

  // ---- The form.

  function renderForm(view) {
    const form = view.form;
    panelTitle.textContent = view.id ? at("editTitle", { name: form?.name || "" }) : at("newAgentTitle");
    if (!form) {
      panelBody.replaceChildren(el("p", "hint", at("loading")));
      return;
    }
    const fields = [];
    if (view.error) fields.push(el("p", "status error-text", view.error));
    if (view.advanced.length) {
      const names = view.advanced.map((name) => at(`adv${name[0].toUpperCase()}${name.slice(1)}`));
      fields.push(el("p", "status", at("advancedNote", { list: names.join(", ") })));
    }

    const field = (label, control, hint) => {
      const wrap = el("label", "field");
      wrap.append(el("span", "field-label", label), control);
      if (hint) wrap.append(el("span", "hint", hint));
      return wrap;
    };

    const name = el("input");
    name.value = form.name;
    name.placeholder = at("namePlaceholder");
    name.maxLength = 120;
    name.oninput = () => (form.name = name.value);
    fields.push(field(at("name"), name));

    // Engine and model.
    const engine = el("select");
    const installed = overview?.engines;
    for (const [id, label] of ENGINES) {
      const missing = Array.isArray(installed) && !installed.includes(id);
      const option = el("option", null, missing ? at("notInstalled", { name: label }) : label);
      option.value = id;
      option.disabled = missing && form.engine !== id;
      engine.append(option);
    }
    engine.value = form.engine;
    engine.onchange = () => {
      form.engine = engine.value;
      form.model = "";
      view.models = null;
      renderModelChoices(view);
      void loadModels(view);
    };
    fields.push(field(at("engine"), engine));
    const model = el("select");
    model.id = "form-model";
    model.onchange = () => (form.model = model.value);
    fields.push(field(at("model"), model));

    // Folder: the folder browser the consoles' part uses.
    const folder = el("button", "folder-pick");
    folder.type = "button";
    folder.append(svgIcon(ICONS.folder));
    const folderText = el("span", "folder-pick-text");
    folderText.append(
      el("span", "folder-pick-name", form.workDir ? folderLabel(form.workDir) : at("ownFolder")),
      el("span", "folder-pick-path", form.workDir),
    );
    folder.append(folderText);
    folder.onclick = () =>
      openFolderBrowser(form.workDir, (path) => {
        form.workDir = path;
        view.drawn = false;
        renderPanel();
      });
    const folderField = el("div", "field");
    const folderRow = el("div", "field-row");
    folderRow.append(folder);
    if (form.workDir) {
      const clear = button("round-button", null, () => {
        form.workDir = "";
        view.drawn = false;
        renderPanel();
      });
      clear.append(svgIcon('<path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />'));
      clear.setAttribute("aria-label", at("ownFolder"));
      clear.title = at("ownFolder");
      folderRow.append(clear);
    }
    folderField.append(el("span", "field-label", at("folder")), folderRow);
    fields.push(folderField);

    const task = el("textarea");
    task.rows = 4;
    task.value = form.task;
    task.placeholder = at("taskPlaceholder");
    task.oninput = () => (form.task = task.value);
    fields.push(field(at("task"), task));

    const instructions = el("textarea");
    instructions.rows = 4;
    instructions.value = form.instructions;
    instructions.oninput = () => (form.instructions = instructions.value);
    const instructionsBox = el("details", "form-more");
    instructionsBox.open = Boolean(form.instructions);
    instructionsBox.append(el("summary", null, at("instructions")), field("", instructions, at("instructionsHint")));
    fields.push(instructionsBox);

    // How far it may go.
    const autonomy = el("fieldset", "autonomy");
    autonomy.append(el("legend", "field-label", at("autonomy")));
    for (const [level, title, text] of [
      ["read", "read", "readText"],
      ["edit", "editLevel", "editText"],
      ["run", "runLevel", "runText"],
    ]) {
      const choice = el("label", "autonomy-choice");
      const radio = el("input");
      radio.type = "radio";
      radio.name = "autonomy";
      radio.value = level;
      radio.checked = form.autonomy === level;
      radio.onchange = () => (form.autonomy = level);
      const words = el("span", "autonomy-words");
      words.append(el("span", "autonomy-title", at(title)), el("span", "hint", at(text)));
      choice.append(radio, words);
      autonomy.append(choice);
    }
    autonomy.append(el("p", "hint", at("autonomyHint")));
    fields.push(autonomy);

    const check = (label, value, set, hint) => {
      const line = el("label", "toggle-line");
      const input = el("input");
      input.type = "checkbox";
      input.checked = value;
      input.onchange = () => set(input.checked);
      const text = el("span", "toggle-text");
      text.append(el("span", null, label));
      if (hint) text.append(el("span", "hint", hint));
      line.append(text, input);
      return line;
    };
    // In sight, not among the rest: without it an agent can't look anything
    // up (a price, the news), and only says so in its answer.
    const web = check(at("web"), form.web, (value) => (form.web = value), at("webHint"));
    web.classList.add("form-toggle");
    fields.push(web);

    // When it runs on its own.
    fields.push(scheduleField(view));

    // The rest.
    const more = el("details", "form-more");
    more.open = form.timeoutMin !== 15 || !form.enabled;
    more.append(el("summary", null, at("moreOptions")));
    more.append(check(at("enabled"), form.enabled, (value) => (form.enabled = value)));
    const timeout = el("input");
    timeout.type = "number";
    timeout.min = "1";
    timeout.inputMode = "numeric";
    timeout.value = String(form.timeoutMin);
    timeout.oninput = () => (form.timeoutMin = Math.min(MAX_MINUTES, Math.max(1, Math.round(Number(timeout.value) || 15))));
    more.append(field(at("timeout"), timeout));
    fields.push(more);

    panelBody.replaceChildren(...fields);
    renderModelChoices(view);

    panelFoot.hidden = false;
    const cancel = button("pill", at("cancel"), () => back());
    const save = button("pill primary", view.saving ? at("saving") : view.id ? at("save") : at("create"), () => void saveForm(view));
    save.disabled = view.saving;
    panelFoot.append(cancel, save);
    view.drawn = true;
    if (!view.id && !form.name) requestAnimationFrame(() => name.focus());
  }

  /** The engine's models, as the desktop's wizard offers them. */
  function renderModelChoices(view) {
    const select = document.getElementById("form-model");
    if (!select || !view.form) return;
    const form = view.form;
    const options = [["", at("modelDefault")]];
    for (const model of view.models ?? []) options.push([model.id, model.note ? `${model.name} — ${model.note}` : model.name]);
    if (form.model && !options.some(([id]) => id === form.model)) {
      options.push([form.model, at("modelUnlisted", { model: form.model })]);
    }
    const key = JSON.stringify(options);
    if (select.dataset.key !== key) {
      select.dataset.key = key;
      select.replaceChildren(
        ...options.map(([id, label]) => {
          const option = el("option", null, label);
          option.value = id;
          return option;
        }),
      );
    }
    select.value = form.model;
  }

  function scheduleField(view) {
    const form = view.form;
    const box = el("div", "field");
    box.append(el("span", "field-label", at("schedule")));
    const kind = el("select");
    for (const [value, label] of [
      ["none", "scheduleNone"],
      ["interval", "scheduleInterval"],
      ["daily", "scheduleDaily"],
    ]) {
      const option = el("option", null, at(label));
      option.value = value;
      kind.append(option);
    }
    kind.value = form.schedule?.kind ?? "none";
    const details = el("div", "schedule-details");
    const draw = () => {
      details.replaceChildren();
      const schedule = form.schedule;
      if (!schedule) return;
      if (schedule.kind === "interval") {
        const minutes = el("input");
        minutes.type = "number";
        minutes.min = "1";
        minutes.inputMode = "numeric";
        minutes.value = String(schedule.minutes);
        minutes.oninput = () => (schedule.minutes = Math.min(MAX_MINUTES, Math.max(1, Math.round(Number(minutes.value) || 60))));
        const wrap = el("label", "field inline");
        wrap.append(el("span", "field-label", at("minutes")), minutes);
        details.append(wrap);
        return;
      }
      const time = el("input");
      time.type = "time";
      time.value = schedule.at;
      time.oninput = () => (schedule.at = time.value || "09:00");
      const wrap = el("label", "field inline");
      wrap.append(el("span", "field-label", at("at")), time);
      const days = el("div", "weekdays");
      // Monday first, as most calendars here show it; Sunday last.
      for (const day of [1, 2, 3, 4, 5, 6, 0]) {
        const chip = button("weekday", weekday(day), () => {
          const set = new Set(schedule.weekdays);
          if (set.has(day)) set.delete(day);
          else set.add(day);
          schedule.weekdays = [...set].sort();
          chip.classList.toggle("on", set.has(day));
          everyDay.hidden = schedule.weekdays.length > 0;
        });
        chip.classList.toggle("on", schedule.weekdays.includes(day));
        chip.setAttribute("aria-pressed", String(schedule.weekdays.includes(day)));
        days.append(chip);
      }
      const everyDay = el("span", "hint", at("everyDay"));
      everyDay.hidden = schedule.weekdays.length > 0;
      details.append(wrap, days, everyDay);
    };
    kind.onchange = () => {
      form.schedule =
        kind.value === "interval"
          ? { kind: "interval", minutes: 60 }
          : kind.value === "daily"
            ? { kind: "daily", at: "09:00", weekdays: [] }
            : null;
      draw();
    };
    draw();
    box.append(kind, details);
    return box;
  }

  async function saveForm(view) {
    if (view.saving) return;
    view.saving = true;
    view.error = "";
    renderForm(view);
    try {
      const saved = await call("save", { form: { ...view.form, id: view.id ?? "" } });
      view.saving = false;
      stack.pop();
      await refresh();
      // A new one: shown, to run it.
      if (!view.id && saved?.id) {
        stack = [{ kind: "agent", id: saved.id }];
        void loadRuns(saved.id);
      }
      if (!stack.length) {
        panel.hidden = true;
        renderPart();
      } else renderPanel();
    } catch (err) {
      view.saving = false;
      view.error = errorText(err);
      renderForm(view);
      panelBody.scrollTop = 0;
    }
  }

  // ---- What the agents say, as it happens.

  /** A run started or moved on: the list and the panel follow it. */
  function noteRun(run) {
    if (!overview) return;
    overview.running = (overview.running ?? []).filter((entry) => entry.agentId !== run.agentId);
    if (run.status === "running") overview.running.push(run);
    const agent = agentById(run.agentId);
    if (agent) agent.lastRun = run;
    const runs = runsOf.get(run.agentId);
    if (runs) {
      const index = runs.findIndex((entry) => entry.id === run.id);
      if (index >= 0) runs[index] = run;
      else runs.unshift(run);
    }
  }

  function event(name, payload) {
    if (!payload) return;
    if (name === "agent-run") {
      noteRun(payload);
      renderList();
      const top = stack[stack.length - 1];
      if (top && top.kind !== "form") renderPanel({ keepForm: true });
      if (payload.status !== "running") void refresh();
    } else if (name === "agent-run-event") {
      const log = logs.get(payload.runId);
      if (!log) return;
      log.events.push(payload.event);
      // As many as the daemon sends at first: a long run's log can hold
      // thousands, and a phone shows the latest.
      const dropped = Math.max(0, log.events.length - MAX_LOG_EVENTS);
      if (dropped) {
        log.events.splice(0, dropped);
        log.skipped += dropped;
      }
      const top = stack[stack.length - 1];
      if (top?.kind !== "run" || top.runId !== payload.runId) return;
      const steps = document.getElementById("run-steps");
      const step = stepElement(payload.event);
      if (!steps || !step) return;
      steps.querySelector(".hint:only-child")?.remove();
      const atBottom = panelBody.scrollHeight - panelBody.scrollTop - panelBody.clientHeight < 80;
      steps.append(step);
      const shown = steps.querySelectorAll(".step");
      for (let index = 0; index < shown.length - MAX_LOG_EVENTS; index++) shown[index].remove();
      if (log.skipped) {
        let note = document.getElementById("run-skipped");
        if (!note) {
          note = el("p", "hint");
          note.id = "run-skipped";
          steps.prepend(note);
        }
        note.textContent = at("earlierSteps", { count: log.skipped });
      }
      if (atBottom) panelBody.scrollTop = panelBody.scrollHeight;
    } else if (name === "agent-question") {
      if (!overview) return;
      const others = (overview.questions ?? []).filter((question) => question.id !== payload.id);
      overview.questions = payload.state === "pending" ? [...others, payload] : others;
      renderList();
      const top = stack[stack.length - 1];
      if (top && top.kind !== "form") renderPanel({ keepForm: true });
    }
  }

  return {
    shown,
    event,
    connected() {
      watching = false;
      renderPart();
      if (part === "agents" || !panel.hidden) void refresh();
    },
    disconnected() {
      watching = false;
      overview = null;
      clearInterval(overviewTimer);
      stack = [];
      panel.hidden = true;
      partsBar.hidden = true;
      consolesPart.hidden = false;
      agentsPart.hidden = true;
    },
  };
})();
