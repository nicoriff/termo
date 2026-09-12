import { Channel, invoke } from "@tauri-apps/api/core";
import type { PtyEvent, ShellInfo } from "./types";

export interface SpawnOptions {
  shellPath?: string;
  shellArgs?: string[];
  cwd?: string;
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

export function killPty(id: string): Promise<void> {
  return invoke("pty_kill", { id });
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
