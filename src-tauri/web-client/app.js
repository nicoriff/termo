// Termo's web client: the consoles a computer keeps running, from another
// one on the tailnet — a phone's browser. It speaks the sessions daemon's
// own protocol (src-tauri/src/mux/protocol.rs) over a WebSocket: frames of
// a little-endian u32 length, a kind byte and a body.
"use strict";

const PROTOCOL = 1;
const KIND_MESSAGE = 0;
const KIND_OUTPUT = 1;
const KIND_INPUT = 2;

// ---- Words, in Termo's two languages (the browser's picks).

const WORDS = {
  es: {
    connecting: "Conectando…",
    noConsoles: "No hay consolas abiertas que se puedan mostrar.",
    disconnected: "Se cortó la conexión. Tocá ⟳ para volver a conectar.",
    rejected: "Esta computadora no te dejó entrar: {reason}",
    readOnly: "Solo lectura",
    exited: "\r\n[La consola terminó]\r\n",
    fit: "Ajustar",
    send: "Escribir…",
    sendPrompt: "Texto a enviar a la consola:",
    refresh: "Volver a cargar",
    back: "Volver",
    smaller: "Letra más chica",
    bigger: "Letra más grande",
    started: "abierta {when}",
  },
  en: {
    connecting: "Connecting…",
    noConsoles: "No open consoles that can be shown.",
    disconnected: "The connection dropped. Tap ⟳ to connect again.",
    rejected: "This computer didn't let you in: {reason}",
    readOnly: "Read-only",
    exited: "\r\n[The console ended]\r\n",
    fit: "Fit",
    send: "Type…",
    sendPrompt: "Text to send to the console:",
    refresh: "Reload",
    back: "Back",
    smaller: "Smaller text",
    bigger: "Larger text",
    started: "opened {when}",
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

// ---- The screens: the list of consoles, and one console.

const listView = document.getElementById("list-view");
const termView = document.getElementById("term-view");
const statusLine = document.getElementById("status");
const sessionList = document.getElementById("sessions");
const titleLine = document.getElementById("title");
const badge = document.getElementById("badge");
const keys = document.getElementById("keys");
const ctrlKey = document.getElementById("ctrl");
document.getElementById("host").textContent = location.hostname;

let connection = null;
let shown = null;
let term = null;
let fit = null;
let fontSize = 13;
let ctrl = false;

function setStatus(text) {
  statusLine.textContent = text;
}

function label(session) {
  if (session.title) return session.title;
  const folder = (session.cwd || "").split(/[\\/]/).filter(Boolean).pop();
  return folder || (session.path.split(/[\\/]/).pop() || session.path).replace(/\.exe$/i, "");
}

function since(seconds) {
  const minutes = Math.max(0, Math.round((Date.now() / 1000 - seconds) / 60));
  const format = new Intl.RelativeTimeFormat(lang, { numeric: "auto" });
  if (minutes < 60) return format.format(-minutes, "minute");
  if (minutes < 60 * 24) return format.format(-Math.round(minutes / 60), "hour");
  return format.format(-Math.round(minutes / 60 / 24), "day");
}

async function connect() {
  setStatus(t("connecting"));
  sessionList.replaceChildren();
  connection = new Connection();
  connection.onOutput = (session, bytes) => {
    if (shown === session && term) term.write(bytes);
  };
  connection.onEvent = (message) => {
    if (message.session !== shown || !term) return;
    if (message.type === "exit") term.write(t("exited"));
    if (message.type === "gone") showList();
  };
  connection.onClose = () => {
    connection = null;
    showList();
    setStatus(t("disconnected"));
  };
  try {
    await connection.open();
  } catch (err) {
    setStatus(err.message === "closed" ? t("disconnected") : t("rejected", { reason: err.message }));
    return;
  }
  await listSessions();
}

async function listSessions() {
  if (!connection) return connect();
  const reply = await connection.request({ op: "list" }).catch(() => null);
  const sessions = reply?.sessions ?? [];
  setStatus(sessions.length ? "" : t("noConsoles"));
  sessionList.replaceChildren(
    ...sessions.map((session) => {
      const item = document.createElement("li");
      const button = document.createElement("button");
      const name = document.createElement("span");
      name.className = "session-title";
      name.textContent = label(session);
      const detail = document.createElement("span");
      detail.className = "session-detail";
      detail.textContent = [session.cwd, t("started", { when: since(session.startedAt) })].filter(Boolean).join(" · ");
      button.append(name, detail);
      button.onclick = () => void open(session);
      item.append(button);
      return item;
    }),
  );
}

async function open(session) {
  shown = session.id;
  listView.hidden = true;
  termView.hidden = false;
  titleLine.textContent = label(session);
  const readOnly = connection.access === "readOnly";
  badge.hidden = !readOnly;
  badge.textContent = t("readOnly");
  keys.hidden = readOnly;
  document.getElementById("fit").hidden = readOnly;

  term?.dispose();
  // The console's own size, not the phone's: fitting it to this screen
  // would rearrange it for the computer it's on too ("Fit" does, on purpose).
  term = new Terminal({
    cols: session.cols,
    rows: session.rows,
    fontSize,
    scrollback: 3000,
    disableStdin: readOnly,
    theme: { background: "#1e1e1e" },
  });
  fit = new FitAddon.FitAddon();
  term.loadAddon(fit);
  term.open(document.getElementById("terminal"));
  term.onData((data) => send(data));
  try {
    await connection.request({ op: "attach", session: session.id, since: 0 });
  } catch {
    showList();
  }
}

function send(data) {
  if (!connection || !shown) return;
  let text = data;
  // Ctrl, held for the next key: a letter becomes its control character.
  if (ctrl && text.length === 1 && /[a-z@[\\\]^_]/i.test(text)) {
    text = String.fromCharCode(text.toUpperCase().charCodeAt(0) & 0x1f);
    ctrl = false;
    ctrlKey.classList.remove("on");
  }
  connection.input(shown, text);
}

function showList() {
  if (shown && connection) connection.request({ op: "detach", session: shown }).catch(() => {});
  shown = null;
  term?.dispose();
  term = null;
  termView.hidden = true;
  listView.hidden = false;
  if (connection) void listSessions();
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
  enter: "\r",
};

keys.addEventListener("click", (event) => {
  const key = event.target.closest("button")?.dataset.key;
  if (!key) return;
  if (key === "ctrl") {
    ctrl = !ctrl;
    ctrlKey.classList.toggle("on", ctrl);
  } else if (key === "send") {
    const text = prompt(t("sendPrompt"));
    if (text) send(text);
  } else {
    send(SEQUENCES[key]);
  }
  term?.focus();
});

document.getElementById("back").onclick = () => showList();
document.getElementById("refresh").onclick = () => void (connection ? listSessions() : connect());
document.getElementById("fit").onclick = () => {
  if (!term || !fit || !connection || !shown) return;
  fit.fit();
  connection.request({ op: "resize", session: shown, cols: term.cols, rows: term.rows }).catch(() => {});
};
for (const [id, step] of [
  ["smaller", -1],
  ["bigger", 1],
]) {
  document.getElementById(id).onclick = () => {
    fontSize = Math.min(24, Math.max(8, fontSize + step));
    if (term) term.options.fontSize = fontSize;
  };
}

void connect();
