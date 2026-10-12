import type { ToolId } from "../ai-mode/pattern-table";
import { nodeVersion } from "../core/agent-install-client";
import { appConfigStore } from "../core/app-config.svelte";
import { marketplaceStatus } from "../core/marketplace-client";
import { uiState } from "../core/ui-state.svelte";
import { checkMissingAgents, forgetAgents, sameKnown, type MissingCheck } from "./missing-agents";

/**
 * Notices agents' CLIs that a change of Node.js left behind, and offers to
 * put them back (see missing-agents.ts for how it tells).
 */
class MissingAgents {
  /** The Node.js running, as the last look found it: what the dialog names. */
  node = $state<string | null>(null);

  /** Looks at what's installed against what's remembered, and remembers the
   * difference. Null when it couldn't look. */
  private async look(): Promise<MissingCheck | null> {
    try {
      const [status, node] = await Promise.all([marketplaceStatus(), nodeVersion().catch(() => null)]);
      this.node = node;
      const installed: Record<ToolId, boolean> = {
        claude: !!status.claudeCli,
        codex: !!status.codexCli,
        gemini: !!status.geminiCli,
        copilot: !!status.copilotCli,
        grok: !!status.grokCli,
      };
      const check = checkMissingAgents(appConfigStore.agentClisKnown, installed, node);
      if (!sameKnown(check.known, appConfigStore.agentClisKnown)) appConfigStore.setAgentClisKnown(check.known);
      return check;
    } catch {
      return null;
    }
  }

  /** At startup: asks about the ones that went missing, if any did. */
  async check(): Promise<void> {
    const check = await this.look();
    if (check && check.missing.length > 0) uiState.openMissingAgents(check.missing);
  }

  /** After putting some back: remembers them where they are now. */
  async settle(): Promise<void> {
    await this.look();
  }

  /** "Don't ask again": they're taken as gone on purpose. */
  forget(tools: readonly ToolId[]): void {
    appConfigStore.setAgentClisKnown(forgetAgents(appConfigStore.agentClisKnown, tools));
  }
}

export const missingAgents = new MissingAgents();
