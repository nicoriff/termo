import { invoke } from "@tauri-apps/api/core";
import { keptConsoles, listShells, type KeptConsole } from "$lib/core/pty-client";
import { remoteOverview, tailscaleRunning } from "$lib/core/remote-client";
import { marketplaceStatus } from "$lib/core/marketplace-client";
import type { ShellInfo } from "$lib/core/types";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/**
 * What the new-tab menu offers: the shells on this machine, and which agent
 * CLIs are installed. Loaded once at startup, so the menu opens already
 * filled — reading them on the first click (probing WSL included) showed a
 * "Loading…" row that then jumped to the full list — and refreshed in the
 * background on every open, so installing an agent CLI while Termo runs
 * still shows up.
 */
class NewTabOptions {
  shells = $state<ShellInfo[]>([]);
  /** Null until the first check answers; the menu treats that as installed
   * rather than flicker the entries off. */
  agentClis = $state<Record<ToolId, boolean> | null>(null);
  /** Consoles the sessions daemon runs that no window shows (see mux/). */
  background = $state<KeptConsole[]>([]);
  /** Whether Tailscale is up here, for "Another computer…". */
  tailscale = $state(false);
  /** Zellij, when it's installed: where, and its sessions. */
  zellij = $state<{ path: string; sessions: { name: string; exited: boolean }[] } | null>(null);

  refresh(): void {
    listShells()
      .then((list) => (this.shells = list))
      .catch(() => {});
    void this.refreshBackground();
    remoteOverview()
      .then((overview) => (this.tailscale = tailscaleRunning(overview)))
      .catch(() => {});
    invoke<{ path: string | null; sessions: { name: string; exited: boolean }[] }>("zellij_sessions")
      .then((found) => (this.zellij = found.path ? { path: found.path, sessions: found.sessions } : null))
      .catch(() => {});
    marketplaceStatus()
      .then(
        (status) =>
          (this.agentClis = { claude: !!status.claudeCli, codex: !!status.codexCli, gemini: !!status.geminiCli, copilot: !!status.copilotCli, grok: !!status.grokCli }),
      )
      .catch(() => {});
  }

  refreshBackground(): Promise<void> {
    return keptConsoles()
      .then((list) => {
        this.background = list.filter((kept) => kept.attached === 0);
      })
      .catch(() => {});
  }
}

/** How a console in the background is named: the title it set, else its
 * folder, else what it runs. */
export function backgroundLabel(kept: KeptConsole): string {
  if (kept.title) return kept.title;
  const folder = kept.cwd?.split(/[\\/]/).filter(Boolean).pop();
  return folder ?? programName(kept.path);
}

/** What a console runs, as ConsoleIcon names it: `pwsh`, `claude`… */
export function programName(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  return base.replace(/\.(exe|cmd|bat|ps1)$/i, "").toLowerCase();
}

export const newTabOptions = new NewTabOptions();
