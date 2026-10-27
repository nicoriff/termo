import { Channel, invoke } from "@tauri-apps/api/core";
import type { PtyEvent, ShellInfo } from "./types";

export interface SpawnOptions {
  shellPath?: string;
  shellArgs?: string[];
  cwd?: string;
  /** Start this agent CLI instead of the shell (see SessionTab.agent). */
  agent?: { tool: string; args?: string[] };
  cols: number;
  rows: number;
  onData: (bytes: Uint8Array) => void;
  onExit: (code: number) => void;
  /** Unix only — never fires on Windows (see PtyEvent::ForegroundProcess). */
  onForegroundProcess?: (path: string | null) => void;
}

type ConsoleHandlers = Pick<SpawnOptions, "onData" | "onExit" | "onForegroundProcess">;

function eventChannel(handlers: ConsoleHandlers): Channel<PtyEvent> {
  const channel = new Channel<PtyEvent>();
  channel.onmessage = (event) => {
    if (event.type === "data") {
      handlers.onData(base64ToBytes(event.data));
    } else if (event.type === "exit") {
      handlers.onExit(event.code);
    } else {
      handlers.onForegroundProcess?.(event.path);
    }
  };
  return channel;
}

/** A console the sessions daemon runs (see src-tauri/src/mux/). */
export interface KeptConsole {
  id: string;
  pid: number | null;
  /** What it runs: the shell or the agent's CLI. */
  path: string;
  cwd: string | null;
  /** Unix seconds. */
  startedAt: number;
  cols: number;
  rows: number;
  /** The last title the console set, if any. */
  title: string | null;
  /** How many windows (here or elsewhere) are showing it. */
  attached: number;
  /** How many of them are this computer's Termo, not someone on the
   * tailnet (a phone); absent from a sessions daemon older than 0.24.1. */
  attachedHere?: number;
  exitCode: number | null;
}

/** Whether a Termo window on this computer shows it: someone on the
 * tailnet watching it from a phone doesn't count. */
export function shownHere(kept: KeptConsole): boolean {
  return (kept.attachedHere ?? kept.attached) > 0;
}

/** Shows a console the sessions daemon kept running: what it printed so
 * far arrives through `onData` first, then everything new. With `host` (an
 * address on the tailnet, `ip:port`), another computer's. */
export function attachPty(id: string, handlers: ConsoleHandlers, host?: string): Promise<KeptConsole> {
  return invoke<KeptConsole>("pty_attach", { id, host: host ?? null, onEvent: eventChannel(handlers) });
}

/** Every console the sessions daemon runs, shown or not. Empty when it
 * isn't running: asking doesn't start it. */
export function keptConsoles(): Promise<KeptConsole[]> {
  return invoke("pty_background");
}

/** Ends a console the sessions daemon runs, whether a tab shows it or not. */
export function killKeptConsole(id: string): Promise<void> {
  return invoke("pty_kill_background", { id });
}

/** Asks for where the agent in a console just attached to stood: it comes
 * back as the usual `agent-event`/`agent-status`, marked restored. */
export function restoreAgentEvents(id: string): Promise<void> {
  return invoke("agent_events_restore", { id });
}

export async function spawnPty(opts: SpawnOptions): Promise<string> {
  const channel = eventChannel(opts);

  return invoke<string>("pty_spawn", {
    shellPath: opts.shellPath,
    shellArgs: opts.shellArgs,
    cwd: opts.cwd,
    agent: opts.agent ?? null,
    cols: opts.cols,
    rows: opts.rows,
    onEvent: channel,
  });
}

export function writePty(id: string, data: string): Promise<void> {
  return invoke("pty_write", { id, data });
}

export function resizePty(id: string, cols: number, rows: number): Promise<void> {
  return invoke("pty_resize", { id, cols, rows });
}

/** The console back to its tab's size, if someone on the tailnet (a phone)
 * sized it meanwhile. Whether it changed. */
export function claimPtySize(id: string, cols: number, rows: number): Promise<boolean> {
  return invoke("pty_claim_size", { id, cols, rows });
}

/** Asks the shell to redraw its prompt at the console's new size. A no-op
 * for shells that redraw on their own, so callers can just always ask. */
export function promptRedrawPty(id: string): Promise<void> {
  return invoke("pty_prompt_redraw", { id });
}

export function killPty(id: string): Promise<void> {
  return invoke("pty_kill", { id });
}

/** Stops showing a console without ending it, when the sessions daemon runs
 * it; false when it doesn't, and nothing changed. */
export function releasePty(id: string): Promise<boolean> {
  return invoke("pty_release", { id });
}

/** Whether a command runs under the console's shell, rather than its prompt
 * sitting idle. Another computer's console counts as busy. */
export function ptyBusy(id: string): Promise<boolean> {
  return invoke("pty_busy", { id });
}

/** Resident memory of the console's shell plus every process running under
 * it, in bytes — or null once its process has already exited. */
export function getPtyMemory(id: string): Promise<number | null> {
  return invoke("pty_memory", { id });
}

/** What closing does to the consoles (see src-tauri's busy_consoles). */
export interface BusyConsoles {
  /** Ended by closing, with a command running under their shell. */
  lost: number;
  /** Kept running in the background, with a command running. */
  kept: number;
  /** Every console kept running, busy or not. */
  keptIds: string[];
}

/** How many consoles have a process running under their shell — a command in
 * progress, rather than a prompt sitting idle —, among those closing would
 * end and those it leaves running in the background. Every window's, or with
 * `thisWindow` only this window's (what closing it would end). */
export function busyConsoles(thisWindow = false): Promise<BusyConsoles> {
  return invoke("busy_consoles", { thisWindow });
}

let windowsBuildRequest: Promise<number | null> | null = null;

/** The Windows build number (null elsewhere), asked once per window. */
export function windowsBuild(): Promise<number | null> {
  windowsBuildRequest ??= invoke<number | null>("windows_build").catch(() => null);
  return windowsBuildRequest;
}

export function listShells(): Promise<ShellInfo[]> {
  return invoke("list_shells");
}

function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** What a process tree is using (see src-tauri/src/pty/memory.rs). */
export interface TreeUsage {
  /** Summed over the tree; 100 is one full core, so it can pass 100. */
  cpu: number;
  /** Resident bytes. */
  memory: number;
  processes: number;
  /** The newest process under the shell, or the shell at its prompt. */
  running: string;
  uptimeSecs: number;
}

export interface ConsoleUsage extends TreeUsage {
  /** The console's PTY id (see pty-registry.ts for its tab). */
  id: string;
  pid: number;
}

export interface Diagnostics {
  cores: number;
  /** CPU is 0–100 across all cores. */
  system: { cpu: number; memoryUsed: number; memoryTotal: number };
  /** Termo and everything under it, consoles included. */
  app: TreeUsage;
  /** Every window's consoles. */
  consoles: ConsoleUsage[];
}

/** CPU and memory of the app, the machine and each console. CPU is measured
 * between two calls, so the first one reads 0. */
export function getDiagnostics(): Promise<Diagnostics> {
  return invoke("diagnostics_snapshot");
}

/** Ends everything running under a console's shell, leaving the shell. */
export function killPtyTree(id: string): Promise<number> {
  return invoke("pty_kill_tree", { id });
}
