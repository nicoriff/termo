import { appConfigStore } from "./app-config.svelte";
import { listShells } from "./pty-client";
import { resolveShellArgs } from "./shell-args";
import type { ShellInfo } from "./types";

export interface ShellLaunch {
  path?: string;
  args?: string[];
  title: string;
}

// Installed shells don't change while the app runs, and listing them on
// Windows spawns `wsl.exe`, so the list is fetched once.
let shellsPromise: Promise<ShellInfo[]> | null = null;

/** The shell new consoles open with: the one picked in Preferences → Consola,
 * or the first detected shell if none was picked (or it's gone). Resolved to
 * a concrete shell here rather than letting the backend pick blind, since
 * resolveShellArgs needs the shell id (e.g. for the PSReadLine opt-in). */
export async function resolveDefaultShell(): Promise<ShellLaunch> {
  shellsPromise ??= listShells().catch((err) => {
    shellsPromise = null;
    throw err;
  });
  const shells = await shellsPromise;
  const shell = shells.find((s) => s.id === appConfigStore.defaultShellId) ?? shells[0];
  return {
    path: shell?.path,
    args: resolveShellArgs(shell?.id, shell?.args, appConfigStore.psreadlineMenuComplete),
    title: shell?.display_name ?? "Shell",
  };
}
