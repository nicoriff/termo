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

export async function spawnPty(opts: SpawnOptions): Promise<string> {
  const channel = new Channel<PtyEvent>();
  channel.onmessage = (event) => {
    if (event.type === "data") {
      opts.onData(base64ToBytes(event.data));
    } else if (event.type === "exit") {
      opts.onExit(event.code);
    } else {
      opts.onForegroundProcess?.(event.path);
    }
  };

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

/** Asks the shell to redraw its prompt at the console's new size. A no-op
 * for shells that redraw on their own, so callers can just always ask. */
export function promptRedrawPty(id: string): Promise<void> {
  return invoke("pty_prompt_redraw", { id });
}

export function killPty(id: string): Promise<void> {
  return invoke("pty_kill", { id });
}

/** Resident memory of the console's shell plus every process running under
 * it, in bytes — or null once its process has already exited. */
export function getPtyMemory(id: string): Promise<number | null> {
  return invoke("pty_memory", { id });
}

/** How many consoles have a process running under their shell — a command in
 * progress, rather than a prompt sitting idle. Every window's, or with
 * `thisWindow` only this window's (what closing it would end). */
export function busyConsoles(thisWindow = false): Promise<number> {
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
