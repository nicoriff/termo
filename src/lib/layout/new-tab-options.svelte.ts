import { listShells } from "$lib/core/pty-client";
import { marketplaceStatus } from "$lib/core/marketplace-client";
import type { ShellInfo } from "$lib/core/types";
import type { ToolId } from "$lib/ai-mode/pattern-table";

/**
 * What the new-tab menu offers: the shells on this machine, and which agent
 * CLIs are installed. Loaded once at startup, so the menu opens already
 * filled — reading them on the first click (probing WSL included) showed a
 * "Loading…" row that then jumped to the full list — and refreshed in the
 * background on every open, so installing Claude or Codex while Termini runs
 * still shows up.
 */
class NewTabOptions {
  shells = $state<ShellInfo[]>([]);
  /** Null until the first check answers; the menu treats that as installed
   * rather than flicker the entries off. */
  agentClis = $state<Record<ToolId, boolean> | null>(null);

  refresh(): void {
    listShells()
      .then((list) => (this.shells = list))
      .catch(() => {});
    marketplaceStatus()
      .then((status) => (this.agentClis = { claude: !!status.claudeCli, codex: !!status.codexCli }))
      .catch(() => {});
  }
}

export const newTabOptions = new NewTabOptions();
