// Termo's web client: the consoles a computer keeps running, from another
// one on the tailnet — a phone's browser. It speaks the sessions daemon's
// own protocol (src-tauri/src/mux/protocol.rs) over a WebSocket: frames of
// a little-endian u32 length, a kind byte and a body. It looks like the
// desktop app on that computer: its theme and console font (/look.json).
"use strict";

const PROTOCOL = 1;
const KIND_MESSAGE = 0;
const KIND_OUTPUT = 1;
const KIND_INPUT = 2;

// ---- Words, in Termo's two languages (the browser's picks).

const WORDS = {
  es: {
    connecting: "Conectando…",
    connected: "Conectado",
    consoles: "Consolas abiertas",
    allConsoles: "Todas las consolas",
    noConsoles: "No hay consolas abiertas en esta computadora.",
    disconnected: "Se cortó la conexión. Tocá ⟳ para volver a conectar.",
    rejected: "Esta computadora no te dejó entrar: {reason}",
    fullAccess: "Acceso total",
    readOnly: "Solo lectura",
    exited: "\r\n[La consola terminó]\r\n",
    ended: "Terminó",
    watching: "{count} mirando",
    fit: "Ajustar",
    fitHint: "Ajustar la consola a esta pantalla mientras la mirás (en la computadora vuelve a su tamaño cuando salís)",
    send: "Escribir…",
    sendPrompt: "Texto a enviar a la consola:",
    refresh: "Volver a cargar",
    smaller: "Letra más chica",
    bigger: "Letra más grande",
    up: "Arriba",
    down: "Abajo",
    left: "Izquierda",
    right: "Derecha",
    enter: "Enter",
    started: "abierta {when}",
    running: "trabajando",
    awaiting: "espera tu respuesta",
    error: "error",
    finished: "terminó",
    plan: "Plan",
    planHint: "Cuánto del límite del plan ya se usó, en cada ventana que informa el agente",
    fiveHour: "5 h",
    sevenDay: "semanal",
    windowHours: "{hours} h",
    windowDays: "{days} d",
    planWindow: "uso",
    context: "Contexto {value}",
    contextHint: "Cuánto de la ventana de contexto del modelo ocupa la conversación",
    cost: "Costo {value}",
    folder: "Carpeta",
    branch: "Rama",
    dirty: "Con cambios sin commitear",
    foreground: "Corriendo ahora",
    newConsole: "Abrir una consola",
    newConsoleHint: "Abrir una consola nueva",
    homeFolder: "Carpeta de inicio",
    opening: "Abriendo {name}…",
    openFailed: "No se pudo abrir: {reason}",
    openNoApp: "Para abrir consolas desde acá, Termo tiene que estar abierto en esta computadora.",
    openBadFolder: "Esa carpeta no existe en esta computadora.",
    openTimeout: "Termo no la abrió a tiempo.",
    openBusy: "Ya se están abriendo otras; probá de nuevo en un momento.",
    pickFolder: "Elegir carpeta",
    places: "Lugares",
    back: "Atrás",
    tagHome: "inicio",
    tagDrive: "unidad",
    tagCurrent: "actual",
    recentFolders: "Recientes",
    noSubfolders: "No tiene subcarpetas",
    loadingFolders: "Cargando…",
    useFolder: "Usar {folder}",
    useThisFolder: "Usar esta carpeta",
    newFolder: "Nueva carpeta",
    newFolderName: "Nombre de la carpeta",
    create: "Crear",
    folderUnreadable: "No se pudo leer esta carpeta.",
    folderBadName: "Ese nombre no sirve para una carpeta.",
    folderExists: "Ya hay una carpeta con ese nombre.",
    folderFailed: "No se pudo crear la carpeta.",
    closeConsole: "Cerrar {name}",
    closeConfirm: "¿Cerrar {name}? Se cierra también en la computadora, con lo que esté corriendo en ella.",
    closeFailed: "No se pudo cerrar: {reason}",
  },
  en: {
    connecting: "Connecting…",
    connected: "Connected",
    consoles: "Open consoles",
    allConsoles: "All consoles",
    noConsoles: "This computer has no open consoles.",
    disconnected: "The connection dropped. Tap ⟳ to connect again.",
    rejected: "This computer didn't let you in: {reason}",
    fullAccess: "Full access",
    readOnly: "Read-only",
    exited: "\r\n[The console ended]\r\n",
    ended: "Ended",
    watching: "{count} watching",
    fit: "Fit",
    fitHint: "Fit the console to this screen while you look at it (on the computer it gets its size back when you leave)",
    send: "Type…",
    sendPrompt: "Text to send to the console:",
    refresh: "Reload",
    smaller: "Smaller text",
    bigger: "Larger text",
    up: "Up",
    down: "Down",
    left: "Left",
    right: "Right",
    enter: "Enter",
    started: "opened {when}",
    running: "working",
    awaiting: "waiting for you",
    error: "error",
    finished: "done",
    plan: "Plan",
    planHint: "How much of the plan's limit is used, in each window the agent reports",
    fiveHour: "5h",
    sevenDay: "weekly",
    windowHours: "{hours}h",
    windowDays: "{days}d",
    planWindow: "used",
    context: "Context {value}",
    contextHint: "How much of the model's context window the conversation takes",
    cost: "Cost {value}",
    folder: "Folder",
    branch: "Branch",
    dirty: "With uncommitted changes",
    foreground: "Running now",
    newConsole: "Open a console",
    newConsoleHint: "Open a new console",
    homeFolder: "Home folder",
    opening: "Opening {name}…",
    openFailed: "Couldn't open it: {reason}",
    openNoApp: "To open consoles from here, Termo has to be open on this computer.",
    openBadFolder: "That folder doesn't exist on this computer.",
    openTimeout: "Termo didn't open it in time.",
    openBusy: "Others are opening already; try again in a moment.",
    pickFolder: "Choose folder",
    places: "Places",
    back: "Back",
    tagHome: "home",
    tagDrive: "drive",
    tagCurrent: "current",
    recentFolders: "Recent",
    noSubfolders: "No subfolders",
    loadingFolders: "Loading…",
    useFolder: "Use {folder}",
    useThisFolder: "Use this folder",
    newFolder: "New folder",
    newFolderName: "Folder name",
    create: "Create",
    folderUnreadable: "This folder couldn't be read.",
    folderBadName: "That name won't do for a folder.",
    folderExists: "There's a folder by that name already.",
    folderFailed: "The folder couldn't be created.",
    closeConsole: "Close {name}",
    closeConfirm: "Close {name}? It closes on the computer too, along with whatever runs in it.",
    closeFailed: "Couldn't close it: {reason}",
  },
};

const lang = (navigator.language || "en").toLowerCase().startsWith("es") ? "es" : "en";
document.documentElement.lang = lang;

function t(key, vars = {}) {
  let text = WORDS[lang][key] ?? WORDS.en[key] ?? key;
  for (const [name, value] of Object.entries(vars)) text = text.replace(`{${name}}`, value);
  return text;
}

for (const element of document.querySelectorAll("[data-text]")) element.textContent = t(element.dataset.text);
for (const element of document.querySelectorAll("[data-label]")) {
  element.setAttribute("aria-label", t(element.dataset.label));
  element.title = t(element.dataset.label);
}

/** This browser's own conveniences (font size, the last console shown):
 * nothing breaks without them. */
const remembered = {
  get(key) {
    try {
      return localStorage.getItem(`termo.${key}`);
    } catch {
      return null;
    }
  },
  set(key, value) {
    try {
      if (value === null) localStorage.removeItem(`termo.${key}`);
      else localStorage.setItem(`termo.${key}`, String(value));
    } catch {
      // Private mode, storage off: just not remembered.
    }
  },
};

// ---- Frames.

const encoder = new TextEncoder();
const decoder = new TextDecoder();

function uuidToBytes(id) {
  const hex = id.replaceAll("-", "");
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) bytes[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return bytes;
}

function bytesToUuid(bytes) {
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function frame(kind, body) {
  const out = new Uint8Array(5 + body.length);
  new DataView(out.buffer).setUint32(0, body.length + 1, true);
  out[4] = kind;
  out.set(body, 5);
  return out;
}

// ---- The connection.

class Connection {
  constructor() {
    this.ws = null;
    this.buffer = new Uint8Array(0);
    this.nextId = 1;
    this.pending = new Map();
    this.access = null;
    this.onOutput = () => {};
    this.onEvent = () => {};
    this.onClose = () => {};
  }

  open() {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);
      ws.binaryType = "arraybuffer";
      this.ws = ws;
      let welcomed = false;
      ws.onopen = () => {
        this.sendMessage({ type: "hello", protocol: PROTOCOL, token: "", version: "web" });
      };
      ws.onmessage = (event) => {
        for (const message of this.take(new Uint8Array(event.data))) {
          if (!welcomed) {
            if (message.type === "welcome") {
              welcomed = true;
              this.access = message.access ?? null;
              resolve();
            } else if (message.type === "rejected") {
              reject(new Error(message.reason));
            }
            continue;
          }
          this.dispatch(message);
        }
      };
      ws.onclose = () => {
        if (!welcomed) reject(new Error("closed"));
        for (const waiting of this.pending.values()) waiting.reject(new Error("closed"));
        this.pending.clear();
        this.onClose();
      };
    });
  }

  /** Appends what arrived and returns the messages it completes; output
   * goes straight to onOutput. */
  take(chunk) {
    const joined = new Uint8Array(this.buffer.length + chunk.length);
    joined.set(this.buffer);
    joined.set(chunk, this.buffer.length);
    this.buffer = joined;
    const messages = [];
    while (this.buffer.length >= 4) {
      const length = new DataView(this.buffer.buffer, this.buffer.byteOffset).getUint32(0, true);
      if (this.buffer.length < 4 + length) break;
      const body = this.buffer.subarray(4, 4 + length);
      const kind = body[0];
      if (kind === KIND_MESSAGE) {
        messages.push(JSON.parse(decoder.decode(body.subarray(1))));
      } else if (kind === KIND_OUTPUT) {
        this.onOutput(bytesToUuid(body.subarray(1, 17)), body.slice(25));
      }
      this.buffer = this.buffer.slice(4 + length);
    }
    return messages;
  }

  dispatch(message) {
    if (message.type === "reply") {
      const waiting = this.pending.get(message.id);
      if (!waiting) return;
      this.pending.delete(message.id);
      if ("Ok" in message.result) waiting.resolve(message.result.Ok);
      else waiting.reject(new Error(message.result.Err));
      return;
    }
    this.onEvent(message);
  }

  sendMessage(message) {
    this.ws?.send(frame(KIND_MESSAGE, encoder.encode(JSON.stringify(message))));
  }

  request(op) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.sendMessage({ type: "request", id, op });
    });
  }

  input(session, text) {
    const id = uuidToBytes(session);
    const bytes = encoder.encode(text);
    const body = new Uint8Array(16 + bytes.length);
    body.set(id);
    body.set(bytes, 16);
    this.ws?.send(frame(KIND_INPUT, body));
  }
}

// ---- Termo's look: the theme the app shows on that computer.

/** Nerd Fonts' symbols (style.css), for the icons in prompts like Oh My
 * Posh's; letters come from the fonts after it. */
const SYMBOLS = '"Symbols Nerd Font Mono"';
const MONO_FALLBACK = 'ui-monospace, "SF Mono", Menlo, Consolas, "Roboto Mono", monospace';
let look = null;

function terminalOptions() {
  const colors = look?.colors ?? {};
  const own = look?.fontFamily ? `${look.fontFamily}, ` : "";
  return {
    theme: { ...colors, background: colors.background ?? "#1e1e1e" },
    fontFamily: `${own}${SYMBOLS}, ${MONO_FALLBACK}`,
    lineHeight: look?.lineHeight || 1.15,
  };
}

function applyLook(next) {
  look = next;
  const ui = next.ui ?? {};
  const colors = next.colors ?? {};
  const root = document.documentElement.style;
  const set = (name, value) => {
    if (typeof value === "string" && value) root.setProperty(name, value);
  };
  set("--bg", ui.background);
  set("--fg", ui.foreground);
  set("--active-bg", ui.activeBackground);
  set("--active-fg", ui.activeForeground);
  set("--border", ui.border);
  set("--accent", ui.accent);
  set("--hover", ui.hoverBackground);
  set("--terminal-bg", colors.background);
  set("--danger", colors.red);
  set("--ok", colors.green);
  set("--warning", colors.yellow);
  root.setProperty("color-scheme", next.appearance === "light" ? "light" : "dark");
  if (ui.background) document.querySelector('meta[name="theme-color"]').content = ui.background;
  if (term) Object.assign(term.options, terminalOptions());
}

async function loadLook() {
  try {
    const response = await fetch("/look.json", { cache: "no-store" });
    if (response.ok) applyLook(await response.json());
  } catch {
    // The default theme stays.
  }
}

/** The symbols font, loaded before the first console is drawn (a console
 * measures its font once). Not waited on for long: without internet the
 * console just shows without the icons. */
const symbolsReady = Promise.race([
  document.fonts?.load(`13px ${SYMBOLS}`, "").catch(() => {}) ?? Promise.resolve(),
  new Promise((resolve) => setTimeout(resolve, 2500)),
]);

// ---- The page's parts.

const $ = (id) => document.getElementById(id);
const homeView = $("home");
const backdrop = $("backdrop");
const consoleView = $("console");
const terminalFrame = $("terminal-frame");
const statusLine = $("status");
const homeHeading = $("home-heading");
const sessionList = $("sessions");
const tabList = $("tab-list");
const menuButton = $("menu");
const keys = $("keys");
const linkDot = $("link");
const accessLine = $("access");
const details = $("details");
const sizeLine = $("size");
const fitButton = $("fit");
const ctrlKey = $("ctrl");
const altKey = $("alt");
const newButton = $("new");
const opener = $("opener");
const folderRow = $("open-folder-row");
const folderPick = $("open-folder");
const folderPickName = $("open-folder-name");
const folderPickPath = $("open-folder-path");
const folderView = $("folders");
const crumbsBar = $("crumbs");
const folderList = $("folder-list");
const folderError = $("folders-error");
const useFolderButton = $("use-folder");
const newFolderButton = $("new-folder");
const choiceList = $("open-choices");
const openStatus = $("open-status");
$("host").textContent = location.hostname;

/** How often the list of consoles is asked again: a console opened or
 * closed on the computer shows up here. */
const LIST_EVERY_MS = 5000;

let connection = null;
let sessions = [];
let shown = null;
let term = null;
let fit = null;
let fontSize = Number(remembered.get("fontSize")) || 13;
let ctrl = false;
let alt = false;
let listTimer = null;
let menuOpen = false;

function readOnly() {
  return connection?.access === "readOnly";
}

function setStatus(text) {
  statusLine.textContent = text;
}

function setLink(state) {
  linkDot.classList.toggle("up", state === "up");
  linkDot.classList.toggle("down", state === "down");
  const word = { up: "connected", down: "disconnected", connecting: "connecting" }[state];
  linkDot.title = t(word);
  linkDot.setAttribute("aria-label", t(word));
}

function programName(session) {
  return (session.path.split(/[\\/]/).pop() || session.path).replace(/\.(exe|cmd|bat)$/i, "");
}

function label(session) {
  if (session.title) return session.title;
  const folder = (session.cwd || "").split(/[\\/]/).filter(Boolean).pop();
  return folder || programName(session);
}

const AGENTS = ["claude", "codex", "gemini", "copilot", "grok"];

/** What kind of console it is, as the desktop's tabs tell it
 * (src/lib/terminal/tab-title.ts): the agent running in it, else its
 * shell. */
function kind(session) {
  const tool = session.agent?.tool?.split(" ")[0].toLowerCase();
  if (AGENTS.includes(tool)) return tool;
  const name = programName(session).toLowerCase();
  if (AGENTS.includes(name)) return name;
  if (name === "bash" && /[\\/]git[\\/]/i.test(session.path)) return "git-bash";
  return name === "wsl" ? "wsl-" : name;
}

// The desktop's console icons (src/lib/agent-launch/ConsoleIcon.svelte),
// drawn the same.
const PROMPT = (color) =>
  `<path d="M4.2 5.2l3 2.8-3 2.8" fill="none" stroke="${color}" stroke-width="1.4" stroke-linecap="round"/>` +
  `<line x1="8.4" y1="10.8" x2="11.8" y2="10.8" stroke="${color}" stroke-width="1.4" stroke-linecap="round"/>`;
const CONSOLE_ICONS = {
  claude:
    '<g stroke="#d97757" stroke-width="1.8" stroke-linecap="round"><line x1="8" y1="1.5" x2="8" y2="14.5"/>' +
    '<line x1="1.5" y1="8" x2="14.5" y2="8"/><line x1="3.4" y1="3.4" x2="12.6" y2="12.6"/>' +
    '<line x1="12.6" y1="3.4" x2="3.4" y2="12.6"/></g>',
  codex:
    '<path d="M8 1.2l5.9 3.4v6.8L8 14.8l-5.9-3.4V4.6z" fill="none" stroke="#10a37f" stroke-width="1.4"/>' +
    '<path d="M5.6 6.2l2 1.8-2 1.8M8.8 10h1.8" fill="none" stroke="#10a37f" stroke-width="1.3" stroke-linecap="round"/>',
  gemini: '<path d="M8 1c.6 4.2 2.8 6.4 7 7-4.2.6-6.4 2.8-7 7-.6-4.2-2.8-6.4-7-7 4.2-.6 6.4-2.8 7-7z" fill="#4796e3"/>',
  copilot:
    '<g fill="none" stroke="#8957e5" stroke-width="1.4"><rect x="1.5" y="4.5" width="5.6" height="5" rx="2.2"/>' +
    '<rect x="8.9" y="4.5" width="5.6" height="5" rx="2.2"/>' +
    '<path d="M7.1 7h1.8M3 12.5c1.4 1 3.2 1.5 5 1.5s3.6-.5 5-1.5" stroke-linecap="round"/></g>',
  grok:
    '<g fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><circle cx="8" cy="8" r="4.6"/>' +
    '<line x1="2.4" y1="13.6" x2="13.6" y2="2.4"/></g>',
  powershell: `<rect x="1" y="2" width="14" height="12" rx="2" fill="#2671be"/>${PROMPT("#fff")}`,
  cmd:
    '<rect x="1" y="2" width="14" height="12" rx="2" fill="#2b2b2b" stroke="#8a8a8a" stroke-width="1"/>' +
    '<path d="M4 5.5l2.4 2.5L4 10.5" fill="none" stroke="#e0e0e0" stroke-width="1.3" stroke-linecap="round"/>' +
    '<line x1="7.8" y1="10.5" x2="11.5" y2="10.5" stroke="#e0e0e0" stroke-width="1.3" stroke-linecap="round"/>',
  git:
    '<rect x="3" y="3" width="10" height="10" rx="1.6" transform="rotate(45 8 8)" fill="#f05032"/>' +
    '<g stroke="#fff" stroke-width="1.1" stroke-linecap="round" fill="#fff"><line x1="6.6" y1="5.4" x2="6.6" y2="10.6"/>' +
    '<line x1="6.6" y1="6.6" x2="9.4" y2="8.6"/><circle cx="6.6" cy="10.6" r="0.9"/><circle cx="9.4" cy="8.6" r="0.9"/></g>',
  wsl: `<rect x="1" y="2" width="14" height="12" rx="2" fill="#e95420"/>${PROMPT("#fff")}`,
  posix: `<rect x="1" y="2" width="14" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.1"/>${PROMPT("currentColor")}`,
};

function family(kind) {
  if (AGENTS.includes(kind)) return kind;
  if (kind === "pwsh" || kind === "powershell") return "powershell";
  if (kind === "cmd") return "cmd";
  if (kind === "git-bash") return "git";
  return kind.startsWith("wsl-") ? "wsl" : "posix";
}

/** The icon for a kind of console, in `className`'s box. */
function kindIcon(kind, className, size) {
  const box = document.createElement("span");
  box.className = className;
  box.innerHTML = `<svg width="${size}" height="${size}" viewBox="0 0 16 16" aria-hidden="true">${CONSOLE_ICONS[family(kind)]}</svg>`;
  return box;
}

/** A long folder path, cut in the middle: its drive or root and the last
 * two folders, which are the ones that tell it apart. */
function shortPath(cwd) {
  const separator = cwd.includes("\\") ? "\\" : "/";
  const parts = cwd.split(/[\\/]/);
  if (parts.length <= 4) return cwd;
  return [parts[0], "…", ...parts.slice(-2)].join(separator);
}

function since(seconds) {
  const minutes = Math.max(0, Math.round((Date.now() / 1000 - seconds) / 60));
  const format = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  if (minutes < 60) return format.format(-minutes, "minute");
  if (minutes < 60 * 24) return format.format(-Math.round(minutes / 60), "hour");
  return format.format(-Math.round(minutes / 60 / 24), "day");
}

function ended(session) {
  return session.exitCode !== null && session.exitCode !== undefined;
}

// ---- Connecting.

async function connect() {
  clearInterval(listTimer);
  setLink("connecting");
  setStatus(t("connecting"));
  sessionList.replaceChildren();
  homeHeading.hidden = true;
  connection = new Connection();
  connection.onOutput = (session, bytes) => {
    if (shown === session && term) term.write(bytes);
  };
  connection.onEvent = (message) => {
    const session = sessions.find((entry) => entry.id === message.session);
    if (message.type === "exit" && session) {
      session.exitCode = message.code;
      if (shown === session.id) term?.write(t("exited"));
      render();
    }
    if (message.type === "info" && session) {
      // Its repository only comes with the list (the daemon runs git for it).
      Object.assign(session, message.info, { git: session.git });
      if (shown === session.id) follow(session);
      render();
    }
    if (message.type === "gone") {
      sessions = sessions.filter((entry) => entry.id !== message.session);
      if (shown === message.session) showHome();
      else render();
    }
    if (message.type === "agent_event") agentsUi.event(message.name, message.payload);
  };
  connection.onClose = () => {
    connection = null;
    clearInterval(listTimer);
    setLink("down");
    agentsUi.disconnected();
    showHome();
    setStatus(t("disconnected"));
  };
  try {
    await connection.open();
  } catch (err) {
    setLink("down");
    setStatus(err.message === "closed" ? t("disconnected") : t("rejected", { reason: err.message }));
    return;
  }
  setLink("up");
  renderAccess();
  agentsUi.connected();
  await listSessions();
  listTimer = setInterval(() => void listSessions(), LIST_EVERY_MS);
  // Back where it was: the console shown last time, or the only one.
  const last = remembered.get("shown");
  const first = sessions.find((session) => session.id === last) ?? (sessions.length === 1 ? sessions[0] : null);
  if (first) void open(first);
}

async function listSessions() {
  if (!connection) return connect();
  const [reply] = await Promise.all([connection.request({ op: "list" }).catch(() => null), listChoices()]);
  if (!reply) return;
  sessions = (reply.sessions ?? []).sort((a, b) => a.startedAt - b.startedAt);
  const current = sessions.find((session) => session.id === shown);
  // A list asked for before this page last sized it may still say the old size.
  if (current && Date.now() - claimedAt > 1500) follow(current);
  if (shown && !current) showHome();
  else render();
}

// ---- Drawing the tabs and the menu.

function render() {
  if (connection) setStatus(sessions.length || agentsUi.shown() ? "" : t("noConsoles"));
  homeHeading.hidden = !sessions.length;

  // The menu: a page of its own with no console shown, else over it.
  const asPage = !shown;
  homeView.hidden = !asPage && !menuOpen;
  homeView.classList.toggle("sheet", !asPage);
  backdrop.hidden = asPage || !menuOpen;
  menuButton.setAttribute("aria-expanded", String(asPage || menuOpen));

  tabList.replaceChildren(
    ...sessions.map((session) => {
      const tab = document.createElement("button");
      tab.className = "tab";
      tab.classList.toggle("active", session.id === shown);
      tab.classList.toggle("ended", ended(session));
      const badge = kindIcon(kind(session), "tab-kind", 12);
      const title = document.createElement("span");
      title.className = "tab-title";
      title.textContent = label(session);
      tab.append(badge, title);
      tab.title = label(session);
      tab.onclick = () => void open(session);
      return tab;
    }),
  );
  tabList.querySelector(".tab.active")?.scrollIntoView({ block: "nearest", inline: "nearest" });
  renderDetails();

  sessionList.replaceChildren(
    ...sessions.map((session) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.className = "session";
      button.classList.toggle("current", session.id === shown);
      const icon = kindIcon(kind(session), "session-kind", 22);
      const text = document.createElement("span");
      text.className = "session-text";
      const name = document.createElement("span");
      name.className = "session-title";
      name.textContent = label(session);
      const when = document.createElement("span");
      when.className = "session-detail";
      when.textContent = [programName(session), t("started", { when: since(session.startedAt) })].join(" · ");
      text.append(name, when);
      if (session.cwd) {
        const path = document.createElement("span");
        path.className = "session-detail";
        path.textContent = shortPath(session.cwd);
        path.title = session.cwd;
        text.append(path);
      }
      button.append(icon, text);
      if (ended(session)) {
        const state = document.createElement("span");
        state.className = "session-state ended";
        state.textContent = t("ended");
        button.append(state);
      } else if (session.attached > (session.id === shown ? 2 : 1)) {
        const state = document.createElement("span");
        state.className = "session-state";
        state.textContent = t("watching", { count: session.attached });
        button.append(state);
      }
      button.onclick = () => void open(session);
      item.className = "session-item";
      item.classList.toggle("closing", closing.has(session.id));
      item.append(button);
      // Closed from here as on the computer: its tab there closes too.
      if (!readOnly()) {
        const close = document.createElement("button");
        close.className = "session-close";
        close.setAttribute("aria-label", t("closeConsole", { name: label(session) }));
        close.title = t("closeConsole", { name: label(session) });
        close.disabled = closing.has(session.id);
        close.append(svgIcon('<path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />'));
        close.onclick = () => void closeConsole(session);
        item.classList.add("closable");
        item.append(close);
      }
      return item;
    }),
  );
  renderOpener();
}

// ---- A new console: asked of Termo on that computer, which opens it as a
// tab of its own — a shell or an agent its new-tab menu offers —, then
// shown here. Full access only, and only while Termo is open there.

/** What Termo there offers to open: null until asked (or read-only), none
 * while it isn't open. */
let choices = null;
/** The folders its agents ran in lately, offered first. */
let recentFolders = [];
/** The folder to open it in: "" for the shell's own; null until picked,
 * which is the folder of the console shown. */
let folderPicked = null;
let opening = false;
let openMessage = "";
/** What the choices were last drawn from: redrawn only when it changes, so
 * a list arriving doesn't eat a tap. */
let choiceKey = "";

async function listChoices() {
  if (!connection || readOnly()) {
    choices = null;
    return;
  }
  const reply = await connection.request({ op: "open_choices" }).catch(() => null);
  if (!reply) return;
  choices = reply.choices ?? [];
  recentFolders = reply.recent ?? [];
}

/** A choice's id is "shell:<the shell's id>" or "agent:<the tool>". */
function choiceKind(choice) {
  return choice.id.slice(choice.id.indexOf(":") + 1);
}

function renderOpener() {
  const can = Boolean(connection) && choices !== null;
  newButton.hidden = !can;
  opener.hidden = !can;
  if (!can) return;
  const offered = choices.length > 0;
  folderRow.hidden = !offered;
  openStatus.textContent = offered ? openMessage : t("openNoApp");

  if (folderPicked === null) folderPicked = sessions.find((session) => session.id === shown)?.cwd ?? "";
  folderPickName.textContent = folderPicked ? folderLabel(folderPicked) : t("homeFolder");
  folderPickPath.textContent = folderPicked;
  folderPick.title = folderPicked || t("homeFolder");

  const nextChoiceKey = JSON.stringify([choices, opening]);
  if (nextChoiceKey === choiceKey) return;
  choiceKey = nextChoiceKey;
  choiceList.replaceChildren(
    ...choices.map((choice) => {
      const button = document.createElement("button");
      button.className = "open-choice";
      button.disabled = opening;
      const badge = kindIcon(choiceKind(choice), "session-kind", 18);
      const name = document.createElement("span");
      name.className = "open-choice-name";
      name.textContent = choice.name;
      button.append(badge, name);
      button.onclick = () => void openNew(choice);
      return button;
    }),
  );
}

folderPick.onclick = () => openFolderBrowser();

function openError(code) {
  const known = { "no-app": "openNoApp", "bad-folder": "openBadFolder", timeout: "openTimeout", busy: "openBusy" };
  return known[code] ? t(known[code]) : t("openFailed", { reason: code });
}

async function openNew(choice) {
  if (!connection || opening) return;
  opening = true;
  openMessage = t("opening", { name: choice.name });
  renderOpener();
  let reply = null;
  try {
    reply = await connection.request({ op: "open", choice: choice.id, cwd: folderPicked || null });
    openMessage = "";
  } catch (err) {
    openMessage = openError(err.message);
  }
  opening = false;
  if (!reply) return renderOpener();
  await listSessions();
  const session = sessions.find((entry) => entry.id === reply.session);
  if (session) void open(session);
}

// "+": the menu, at its part for a new console, offering the folder of the
// console shown.
newButton.onclick = () => {
  folderPicked = null;
  if (!opening) openMessage = "";
  if (shown) setMenu(true);
  else render();
  requestAnimationFrame(() => opener.scrollIntoView({ block: "nearest", behavior: "smooth" }));
};

// ---- Picking the folder: the desktop's folder browser, by touch. It starts
// in the folder picked, its parents behind it in the crumbs; a tap on a
// folder goes into it, the crumbs and ‹ go back up, and "Use …" picks the
// one it's in. At the top, the places: the folder picked, home and the
// drives, then the open consoles' folders and those agents ran in lately.

// As the desktop's (src/lib/agent-launch/folder-nav.ts).
const DRIVE_ROOT = /^[A-Za-z]:\\?$/;

function trimSeparator(path) {
  if (path === "/" || DRIVE_ROOT.test(path)) return path;
  return path.replace(/[\\/]+$/, "");
}

/** The folder one level up; null at a drive or the root. */
function parentPath(path) {
  const trimmed = trimSeparator(path);
  if (trimmed === "/" || DRIVE_ROOT.test(trimmed)) return null;
  if (/^[A-Za-z]:/.test(trimmed)) {
    const cut = trimmed.lastIndexOf("\\");
    return cut <= 2 ? `${trimmed.slice(0, 2)}\\` : trimmed.slice(0, cut);
  }
  const cut = trimmed.lastIndexOf("/");
  return cut <= 0 ? "/" : trimmed.slice(0, cut);
}

function samePath(a, b) {
  const normalize = (path) => {
    const trimmed = trimSeparator(path);
    return /^[A-Za-z]:/.test(trimmed) ? trimmed.replace(/\//g, "\\").replace(/\\$/, "").toLowerCase() : trimmed;
  };
  return normalize(a) === normalize(b);
}

function folderLabel(path) {
  const trimmed = trimSeparator(path);
  if (trimmed === "/") return "/";
  if (DRIVE_ROOT.test(trimmed)) return trimmed.slice(0, 2);
  return trimmed.split(/[\\/]/).pop() || trimmed;
}

const ICONS = {
  folder: '<path d="M1.5 4.5v8h13v-7h-6l-1.5-1.5h-5.5z" />',
  home: '<path d="M2 8l6-5.5L14 8M3.8 6.5v7h8.4v-7" />',
  drive: '<rect x="1.5" y="4.5" width="13" height="7" rx="1.5" /><circle cx="12" cy="8" r="0.6" />',
};

function svgIcon(markup, className = "") {
  const element = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  element.setAttribute("viewBox", "0 0 16 16");
  element.setAttribute("aria-hidden", "true");
  if (className) element.setAttribute("class", className);
  element.innerHTML = markup;
  return element;
}

/** Each level walked into, from the places (path null) down to the one
 * shown: its folders once read. */
let walk = [];
/** Folders already read while the browser is open. */
const folderCache = new Map();
let creatingFolder = false;
let folderErrorTimer = null;

function folderErrorText(code) {
  const known = { unreadable: "folderUnreadable", "bad-name": "folderBadName", exists: "folderExists", failed: "folderFailed" };
  return known[code] ? t(known[code]) : t("openFailed", { reason: code });
}

function showFolderError(text) {
  folderError.textContent = text;
  clearTimeout(folderErrorTimer);
  folderErrorTimer = setTimeout(() => (folderError.textContent = ""), 3500);
}

async function placeItems() {
  const reply = await connection.request({ op: "folder_roots" });
  const items = [];
  const place = (item) => {
    if (!items.some((other) => samePath(other.path, item.path))) items.push(item);
  };
  if (folderStart) place({ name: folderLabel(folderStart), path: folderStart, tag: "current" });
  if (reply.roots.home) place({ ...reply.roots.home, tag: "home" });
  for (const drive of reply.roots.drives) place({ ...drive, tag: "drive" });
  for (const path of [...sessions.map((session) => session.cwd), ...recentFolders]) {
    if (path) place({ name: folderLabel(path), path, tag: "recent" });
  }
  return items;
}

async function loadLevel(level) {
  if (level.path !== null && folderCache.has(level.path)) {
    level.items = folderCache.get(level.path);
    level.status = "ready";
    return;
  }
  level.status = "loading";
  try {
    level.items =
      level.path === null
        ? await placeItems()
        : ((await connection.request({ op: "list_folders", path: level.path })).folders ?? []);
    if (level.path !== null) folderCache.set(level.path, level.items);
    level.status = "ready";
  } catch (err) {
    level.items = [];
    level.status = "error";
    level.error = folderErrorText(err.message);
  }
}

/** Where the browser started, and what gets the folder picked: a new
 * console's, unless another part of the page asked (an agent's folder,
 * see agents.js). */
let folderStart = "";
let folderOnPick = null;

function openFolderBrowser(start = folderPicked, onPick = null) {
  if (!connection) return;
  folderStart = start || "";
  folderOnPick = onPick;
  folderCache.clear();
  creatingFolder = false;
  folderError.textContent = "";
  walk = [{ path: null }];
  const chain = [];
  for (let path = folderStart || null; path !== null; path = parentPath(path)) chain.unshift({ path });
  walk.push(...chain);
  folderView.hidden = false;
  void showLevel(walk.length - 1, null);
}

function closeFolderBrowser() {
  folderView.hidden = true;
  creatingFolder = false;
}

async function showLevel(index, direction) {
  walk = walk.slice(0, index + 1);
  const level = walk[index];
  creatingFolder = false;
  if (!level.items) {
    level.status = "loading";
    renderFolders(direction);
    await loadLevel(level);
    if (walk[walk.length - 1] !== level) return;
    return renderFolders(null);
  }
  renderFolders(direction);
}

function goInto(item) {
  walk.push({ path: item.path });
  void showLevel(walk.length - 1, "forward");
}

function goBack() {
  if (creatingFolder) {
    creatingFolder = false;
    return renderFolders(null);
  }
  if (walk.length <= 1) return closeFolderBrowser();
  void showLevel(walk.length - 2, "back");
}

function useFolder(path) {
  closeFolderBrowser();
  if (folderOnPick) return folderOnPick(path);
  folderPicked = path;
  renderOpener();
}

function renderFolders(direction) {
  const level = walk[walk.length - 1];
  if (!level) return;

  crumbsBar.replaceChildren(
    ...walk.flatMap((step, index) => {
      const last = index === walk.length - 1;
      const crumb = document.createElement(last ? "span" : "button");
      crumb.className = last ? "crumb current" : "crumb";
      crumb.textContent = step.path === null ? t("places") : folderLabel(step.path);
      if (!last) crumb.onclick = () => void showLevel(index, "back");
      if (index === 0) return [crumb];
      const separator = document.createElement("span");
      separator.className = "crumb-sep";
      separator.textContent = "›";
      return [separator, crumb];
    }),
  );
  crumbsBar.scrollLeft = crumbsBar.scrollWidth;

  const rows = [];
  if (creatingFolder) {
    const row = document.createElement("form");
    row.className = "new-folder-row";
    const input = document.createElement("input");
    input.placeholder = t("newFolderName");
    input.setAttribute("aria-label", t("newFolderName"));
    input.spellcheck = false;
    input.autocapitalize = "off";
    input.setAttribute("autocomplete", "off");
    const create = document.createElement("button");
    create.className = "pill primary";
    create.style.flex = "0 0 auto";
    create.textContent = t("create");
    row.append(input, create);
    row.onsubmit = (event) => {
      event.preventDefault();
      void commitFolder(input.value);
    };
    rows.push(row);
    requestAnimationFrame(() => input.focus());
  }
  let inRecent = false;
  for (const item of level.items ?? []) {
    if (item.tag === "recent" && !inRecent) {
      inRecent = true;
      const title = document.createElement("p");
      title.className = "folder-section";
      title.textContent = t("recentFolders");
      rows.push(title);
    }
    const row = document.createElement("button");
    row.className = "folder-row";
    row.title = item.path;
    row.append(svgIcon(ICONS[item.tag === "home" || item.tag === "drive" ? item.tag : "folder"]));
    const text = document.createElement("span");
    text.className = "folder-row-text";
    const name = document.createElement("span");
    name.className = "folder-row-name";
    name.textContent = item.name;
    text.append(name);
    if (item.tag === "recent" || item.tag === "current") {
      const path = document.createElement("span");
      path.className = "folder-row-path";
      path.textContent = item.path;
      text.append(path);
    }
    row.append(text);
    const tags = { home: "tagHome", drive: "tagDrive", current: "tagCurrent" };
    if (tags[item.tag]) {
      const tag = document.createElement("span");
      tag.className = "folder-tag";
      tag.textContent = t(tags[item.tag]);
      row.append(tag);
    }
    row.append(svgIcon('<path d="M1 1l4 4-4 4" />', "chevron"));
    row.lastChild.setAttribute("viewBox", "0 0 6 10");
    row.onclick = () => goInto(item);
    rows.push(row);
  }
  if (!level.items?.length && !creatingFolder) {
    const note = document.createElement("p");
    note.className = "folder-note";
    note.textContent =
      level.status === "loading" ? t("loadingFolders") : level.status === "error" ? level.error : t("noSubfolders");
    rows.push(note);
  }
  folderList.replaceChildren(...rows);
  folderList.scrollTop = 0;
  folderList.classList.remove("forward", "back");
  if (direction) {
    void folderList.offsetWidth;
    folderList.classList.add(direction);
  }

  const inside = level.path !== null;
  newFolderButton.disabled = !inside || level.status !== "ready" || creatingFolder;
  useFolderButton.disabled = !inside;
  useFolderButton.textContent = inside ? t("useFolder", { folder: folderLabel(level.path) }) : t("useThisFolder");
}

/** Creates it in the folder shown, and goes into it: likely where the new
 * console goes. */
async function commitFolder(name) {
  const level = walk[walk.length - 1];
  if (!name.trim() || !level?.path) {
    creatingFolder = false;
    return renderFolders(null);
  }
  try {
    const reply = await connection.request({ op: "create_folder", parent: level.path, name });
    folderCache.delete(level.path);
    level.items = null;
    await loadLevel(level);
    creatingFolder = false;
    goInto(reply.folder);
  } catch (err) {
    showFolderError(folderErrorText(err.message));
  }
}

$("folders-back").onclick = () => goBack();
newFolderButton.onclick = () => {
  creatingFolder = true;
  renderFolders(null);
};
useFolderButton.onclick = () => {
  const level = walk[walk.length - 1];
  if (level?.path) useFolder(level.path);
};

/** The consoles asked to close, until they're gone. */
const closing = new Set();

async function closeConsole(session) {
  if (!connection || closing.has(session.id)) return;
  if (!confirm(t("closeConfirm", { name: label(session) }))) return;
  closing.add(session.id);
  render();
  try {
    await connection.request({ op: "kill", session: session.id });
  } catch (err) {
    closing.delete(session.id);
    render();
    alert(t("closeFailed", { reason: err.message === "no-app" ? t("openNoApp") : err.message }));
    return;
  }
  // Gone once Termo there has closed its tab: asked a few times until then.
  for (const wait of [300, 900, 2500]) {
    await new Promise((resolve) => setTimeout(resolve, wait));
    await listSessions();
    if (!sessions.some((entry) => entry.id === session.id)) break;
  }
  closing.delete(session.id);
  render();
}

function setMenu(open) {
  menuOpen = open;
  // Opened again: a new console goes where the one shown is, by default.
  if (open && !opening) {
    folderPicked = null;
    openMessage = "";
  }
  render();
}

function renderAccess() {
  accessLine.replaceChildren();
  if (!connection) return;
  const dot = document.createElement("span");
  dot.className = "access-dot";
  dot.classList.toggle("read-only", readOnly());
  accessLine.append(dot, t(readOnly() ? "readOnly" : "fullAccess"));
}

// ---- The status bar: what the desktop's says of the console shown.

const HOUR = 60;
const DAY = 24 * HOUR;

/** A plan window named from its length, as the desktop names it. */
function windowLabel(minutes) {
  if (!minutes || minutes <= 0) return t("planWindow");
  if (minutes === 5 * HOUR) return t("fiveHour");
  if (minutes === 7 * DAY) return t("sevenDay");
  const round = (value) => (Number.isInteger(value) ? value : Math.round(value * 10) / 10);
  if (minutes < DAY) return t("windowHours", { hours: round(minutes / HOUR) });
  return t("windowDays", { days: round(minutes / DAY) });
}

function item(className, title, ...children) {
  const element = document.createElement("span");
  element.className = `status-item ${className}`;
  if (title) element.title = title;
  element.append(...children);
  return element;
}

function icon(glyph) {
  const element = document.createElement("span");
  element.className = "status-icon";
  element.setAttribute("aria-hidden", "true");
  element.textContent = glyph;
  return element;
}

function gauge(percent) {
  const element = document.createElement("span");
  element.className = "gauge";
  element.style.setProperty("--fill", `${Math.min(100, Math.max(0, percent))}%`);
  const value = document.createElement("span");
  value.className = "gauge-text";
  value.textContent = `${Math.round(percent)}%`;
  element.append(value);
  return element;
}

function renderDetails() {
  const session = sessions.find((entry) => entry.id === shown);
  if (!session || !term) return details.replaceChildren();
  const parts = [];
  const agent = session.agent;
  if (agent) {
    const dot = document.createElement("span");
    dot.className = "agent-dot";
    parts.push(item(`agent ${agent.status}`, null, dot, `${agent.tool} · ${t(agent.status)}`));
    if (agent.planWindows?.length) {
      const windows = agent.planWindows.flatMap((window) => {
        const name = document.createElement("span");
        name.className = "gauge-label";
        name.textContent = windowLabel(window.windowMinutes);
        return [name, gauge(window.percent)];
      });
      parts.push(item("plan", t("planHint"), t("plan"), ...windows));
    }
    if (agent.contextPercent !== null && agent.contextPercent !== undefined) {
      parts.push(item("context", t("contextHint"), t("context", { value: `${Math.round(agent.contextPercent)}%` })));
    }
    if (agent.costUsd !== null && agent.costUsd !== undefined) {
      const cost = agent.costUsd > 0 && agent.costUsd < 0.01 ? "<$0.01" : `$${agent.costUsd.toFixed(2)}`;
      parts.push(item("cost", null, t("cost", { value: cost })));
    }
  }
  if (session.cwd) {
    const folder = session.cwd.split(/[\\/]/).filter(Boolean).pop() || session.cwd;
    parts.push(item("folder", `${t("folder")}: ${session.cwd}`, icon("\uf07b"), folder));
  }
  if (session.git) {
    const branch = item("branch", t("branch"), icon("\ue725"), session.git.branch);
    if (session.git.dirty) {
      const mark = document.createElement("span");
      mark.className = "dirty";
      mark.textContent = "●";
      mark.title = t("dirty");
      branch.append(mark);
      branch.title = `${t("branch")} · ${t("dirty")}`;
    }
    parts.push(branch);
  }
  if (session.foreground) {
    const program = (session.foreground.split(/[\\/]/).pop() || "").replace(/\.(exe|cmd|bat)$/i, "");
    if (program) parts.push(item("foreground", t("foreground"), icon("\uea85"), program));
  }
  details.replaceChildren(...parts);
}

function renderSize() {
  sizeLine.hidden = !term;
  if (term) sizeLine.textContent = `${term.cols}×${term.rows}`;
}

function renderConsoleControls() {
  const showing = Boolean(term);
  keys.hidden = !showing || readOnly();
  fitButton.hidden = !showing || readOnly();
  fitButton.classList.toggle("on", fitting());
  fitButton.setAttribute("aria-pressed", String(fitting()));
  terminalFrame.classList.toggle("fitted", fitting() && !following);
  $("smaller").hidden = !showing;
  $("bigger").hidden = !showing;
  renderSize();
  renderDetails();
}

// ---- Sized to this screen.
//
// A console has one size, the same everywhere it's shown. Fitted, it takes
// this screen's when it's opened here, and again when the screen changes
// (the phone turned, its keyboard came up) or something is typed — the
// computer's window shows it that size too —, and the daemon gives it its
// own size back when this page stops showing it. Sized meanwhile somewhere
// else (the computer's tab taken up again, another phone), it's shown at
// that size until then: taking it back on its own would have the two
// screens wrest it from each other. Read-only can't resize it: it scrolls
// sideways.

let fitToScreen = remembered.get("fit") !== "0";
/** Sized somewhere else since this page last sized it. */
let following = false;
/** The size this page last gave it, and when. */
let claimed = null;
let claimedAt = 0;

/** What the daemon says its size is now: shown at it. */
function follow(session) {
  if (!term || (term.cols === session.cols && term.rows === session.rows)) return;
  term.resize(session.cols, session.rows);
  following = fitting() && !(claimed?.cols === session.cols && claimed?.rows === session.rows);
  renderConsoleControls();
}

function claim(size) {
  following = false;
  claimed = size;
  claimedAt = Date.now();
}

function fitting() {
  return Boolean(term) && fitToScreen && !readOnly();
}

/** The size that fills the console's space here, if it differs. */
function screenSize() {
  const proposed = fit?.proposeDimensions();
  if (!proposed || !Number.isFinite(proposed.cols) || !Number.isFinite(proposed.rows)) return null;
  const cols = Math.max(20, proposed.cols);
  const rows = Math.max(5, proposed.rows);
  return cols === term.cols && rows === term.rows ? null : { cols, rows };
}

/** Resized: a prompt drawn for the old width stays as it was (Oh My
 * Posh's right-hand part wraps), so the shell draws it again — if it sits
 * at its prompt, which the daemon checks. */
function redrawPrompt(session) {
  connection?.request({ op: "prompt_redraw", session }).catch(() => {});
}

async function fitNow() {
  if (!fitting() || !connection || !shown) return;
  // Measured filling the screen, not at the size it follows.
  if (following) {
    following = false;
    renderConsoleControls();
  }
  const size = screenSize();
  if (!size) return;
  claim(size);
  term.resize(size.cols, size.rows);
  renderSize();
  const session = shown;
  await connection.request({ op: "resize", session, ...size }).catch(() => {});
  redrawPrompt(session);
}

let fitTimer = null;
// The space changes: the phone turned, its keyboard came up or went.
new ResizeObserver(() => {
  clearTimeout(fitTimer);
  fitTimer = setTimeout(() => void fitNow(), 150);
}).observe(terminalFrame);

// ---- A console.

async function open(session, { again = false } = {}) {
  if (!connection) return;
  menuOpen = false;
  if (shown === session.id && term && !again) return render();
  if (shown) connection.request({ op: "detach", session: shown }).catch(() => {});
  shown = session.id;
  remembered.set("shown", session.id);
  consoleView.hidden = false;
  render();

  term?.dispose();
  term = null;
  await symbolsReady;
  if (shown !== session.id) return;
  term = new Terminal({
    cols: session.cols,
    rows: session.rows,
    fontSize,
    scrollback: 3000,
    disableStdin: readOnly(),
    cursorBlink: !readOnly(),
    allowTransparency: true,
    ...terminalOptions(),
  });
  fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open($("terminal"));
  term.onData((data) => send(data));
  term.onTitleChange((title) => {
    const entry = sessions.find((item) => item.id === shown);
    if (entry && title) {
      entry.title = title;
      render();
    }
  });
  renderConsoleControls();
  try {
    // Sized first: what's on the screen then arrives drawn for this size.
    let resized = false;
    following = false;
    claimed = null;
    if (fitting()) {
      const size = screenSize();
      if (size) {
        claim(size);
        term.resize(size.cols, size.rows);
        await connection.request({ op: "resize", session: session.id, ...size });
        resized = true;
      }
    } else {
      // Its own size, as it is now (given back if this page had it fitted).
      const reply = await connection.request({ op: "list" });
      const now = reply.sessions?.find((entry) => entry.id === session.id);
      if (now && (now.cols !== term.cols || now.rows !== term.rows)) term.resize(now.cols, now.rows);
    }
    renderSize();
    await connection.request({ op: "attach", session: session.id, since: 0 });
    if (resized) redrawPrompt(session.id);
  } catch {
    showHome();
  }
}

function showHome() {
  if (shown && connection) connection.request({ op: "detach", session: shown }).catch(() => {});
  shown = null;
  menuOpen = false;
  term?.dispose();
  term = null;
  consoleView.hidden = true;
  renderConsoleControls();
  render();
}

function send(data) {
  if (!connection || !shown || readOnly()) return;
  // Typing here takes the console back to this screen's size.
  if (following) void fitNow();
  let text = data;
  // Ctrl, held for the next key: a letter becomes its control character.
  if (ctrl && text.length === 1 && /[a-z@[\\\]^_ ]/i.test(text)) {
    text = String.fromCharCode(text.toUpperCase().charCodeAt(0) & 0x1f);
    setCtrl(false);
  }
  // Alt: the key, after an Escape (what terminals send for Meta).
  if (alt && text.length === 1) {
    text = `\x1b${text}`;
    setAlt(false);
  }
  connection.input(shown, text);
}

function setCtrl(on) {
  ctrl = on;
  ctrlKey.classList.toggle("on", on);
}

function setAlt(on) {
  alt = on;
  altKey.classList.toggle("on", on);
}

const SEQUENCES = {
  esc: "\x1b",
  tab: "\t",
  up: "\x1b[A",
  down: "\x1b[B",
  right: "\x1b[C",
  left: "\x1b[D",
  pgup: "\x1b[5~",
  pgdn: "\x1b[6~",
  home: "\x1b[H",
  end: "\x1b[F",
  enter: "\r",
  ctrlc: "\x03",
};

// Keys keep the phone's keyboard up: pressing one doesn't take the focus
// away from the console.
keys.addEventListener("pointerdown", (event) => {
  if (event.target.closest("button")?.dataset.key !== "send") event.preventDefault();
});
keys.addEventListener("click", (event) => {
  const key = event.target.closest("button")?.dataset.key;
  if (!key) return;
  if (key === "ctrl") setCtrl(!ctrl);
  else if (key === "alt") setAlt(!alt);
  else if (key === "send") {
    const text = prompt(t("sendPrompt"));
    if (text) send(text);
  } else {
    const sequence = SEQUENCES[key];
    // Straight through, past Ctrl/Alt: these are whole keys already.
    if (sequence && connection && shown && !readOnly()) connection.input(shown, sequence);
  }
  term?.focus();
});

// ☰ opens the menu over the console, and closes it. With no console shown
// (the menu is the page), it goes back to the last one.
menuButton.onclick = () => {
  if (shown) return setMenu(!menuOpen);
  const last = sessions.find((session) => session.id === remembered.get("shown"));
  if (last) void open(last);
};
backdrop.onclick = () => setMenu(false);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !folderView.hidden) goBack();
  else if (event.key === "Escape" && menuOpen) setMenu(false);
});

$("refresh").onclick = (event) => {
  const button = event.currentTarget;
  button.classList.remove("spinning");
  void button.offsetWidth;
  button.classList.add("spinning");
  void loadLook();
  void (connection ? listSessions() : connect());
};
fitButton.onclick = () => {
  if (!term || !connection || !shown) return;
  fitToScreen = !fitToScreen;
  remembered.set("fit", fitToScreen ? "1" : "0");
  const session = sessions.find((entry) => entry.id === shown);
  // Fitted, or back to its own size: shown anew, so the daemon gives that
  // size back (leaving) and the screen arrives drawn for it.
  if (session) void open(session, { again: true });
};
for (const [id, step] of [
  ["smaller", -1],
  ["bigger", 1],
]) {
  $(id).onclick = () => {
    fontSize = Math.min(24, Math.max(7, fontSize + step));
    remembered.set("fontSize", fontSize);
    if (term) term.options.fontSize = fontSize;
    void fitNow();
  };
}

// ---- Scrolling with a finger. xterm leaves touch to the browser, which
// finds nothing to scroll: the console's lines live in xterm's own buffer.
// A drag up or down moves through them, and a flick carries on a while; a
// full-screen program (Claude Code, less, vim) has none, so it's told the
// way a mouse wheel would tell it, and scrolls itself. A sideways drag is
// left alone: an unfitted console wider than the screen scrolls that way.

/** Drags shorter than this are taps: they bring the keyboard up. */
const DRAG_PX = 8;

let touch = null;
let flick = null;

function lineHeight() {
  const screen = term?.element?.querySelector(".xterm-screen");
  return screen && term.rows ? screen.clientHeight / term.rows : 16;
}

/** Positive: towards the newest lines. */
function scrollConsole(lines, x, y) {
  if (!term || !lines) return;
  if (term.buffer.active.type === "normal") {
    term.scrollLines(lines);
    return;
  }
  term.element.querySelector(".xterm-screen")?.dispatchEvent(
    new WheelEvent("wheel", {
      deltaY: lines * lineHeight(),
      deltaMode: WheelEvent.DOM_DELTA_PIXEL,
      clientX: x,
      clientY: y,
      bubbles: true,
      cancelable: true,
    }),
  );
}

function stopFlick() {
  if (flick) cancelAnimationFrame(flick.frame);
  flick = null;
}

terminalFrame.addEventListener(
  "touchstart",
  (event) => {
    stopFlick();
    if (event.touches.length !== 1 || !term) return (touch = null);
    const { clientX: x, clientY: y } = event.touches[0];
    touch = { x, y, lastY: y, axis: null, rest: 0, moves: [{ y, at: event.timeStamp }] };
  },
  { passive: true },
);

terminalFrame.addEventListener(
  "touchmove",
  (event) => {
    if (!touch || event.touches.length !== 1) return;
    const { clientX: x, clientY: y } = event.touches[0];
    if (!touch.axis) {
      const across = Math.abs(x - touch.x);
      const along = Math.abs(y - touch.y);
      if (Math.max(across, along) < DRAG_PX) return;
      touch.axis = along > across ? "y" : "x";
    }
    // Not fitted and taller than the screen: the browser pans down to its
    // last rows first.
    const pans = !terminalFrame.classList.contains("fitted") && terminalFrame.scrollHeight > terminalFrame.clientHeight;
    if (touch.axis !== "y" || pans) return;
    event.preventDefault();
    // The finger going up brings later lines into view.
    const pixels = touch.lastY - y + touch.rest;
    const height = lineHeight();
    const lines = Math.trunc(pixels / height);
    touch.rest = pixels - lines * height;
    touch.lastY = y;
    scrollConsole(lines, x, y);
    touch.moves.push({ y, at: event.timeStamp });
    if (touch.moves.length > 6) touch.moves.shift();
  },
  { passive: false },
);

terminalFrame.addEventListener("touchend", (event) => {
  const ended = touch;
  touch = null;
  if (!ended || ended.axis !== "y" || term?.buffer.active.type !== "normal") return;
  // How fast the finger was going as it let go, over its last moves.
  const recent = ended.moves.filter((move) => event.timeStamp - move.at < 100);
  if (recent.length < 2) return;
  const first = recent[0];
  const last = recent[recent.length - 1];
  let speed = (first.y - last.y) / Math.max(1, last.at - first.at); // px per ms
  if (Math.abs(speed) < 0.3) return;
  let rest = 0;
  let before = performance.now();
  const step = (now) => {
    const pixels = speed * (now - before) + rest;
    before = now;
    const height = lineHeight();
    const lines = Math.trunc(pixels / height);
    rest = pixels - lines * height;
    scrollConsole(lines);
    speed *= 0.95;
    if (Math.abs(speed) < 0.05 || !term) return stopFlick();
    flick.frame = requestAnimationFrame(step);
  };
  flick = { frame: requestAnimationFrame(step) };
});

terminalFrame.addEventListener("touchcancel", () => (touch = null));

// ---- The phone's keyboard: the page shrinks to what's above it, so the
// console's last line and the extra keys stay in sight.

function followViewport() {
  const viewport = window.visualViewport;
  if (!viewport) return;
  document.documentElement.style.setProperty("--app-height", `${viewport.height}px`);
  document.body.classList.toggle("typing", viewport.height < window.innerHeight * 0.8);
  window.scrollTo(0, 0);
}
window.visualViewport?.addEventListener("resize", followViewport);
window.visualViewport?.addEventListener("scroll", followViewport);
followViewport();

// Back to the page after a while away (the phone locked, another app): the
// connection may have dropped meanwhile.
document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible" && !connection) void connect();
});

void loadLook();
void connect();
