import {
  listAgentFavorites,
  removeAgentFavorite,
  saveAgentFavorite,
  type AgentFavorite,
} from "$lib/core/agent-favorites-client";
import { collectTiles } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { agentSessionsStore } from "./agent-sessions-store.svelte";
import { canRestore, sessionLabel } from "./restore-session";
import { tabAgentSessions } from "./tab-sessions.svelte";

/** The agent conversations marked as favorites, as kept by the backend. Fed by
 * the `agent-favorites` event (every window) and one load at startup (see
 * ai-mode/attention.ts). */
class FavoritesStore {
  favorites = $state<AgentFavorite[]>([]);

  /** By the agent's session id. */
  readonly byId = $derived(new Map(this.favorites.map((favorite) => [favorite.agentSessionId, favorite])));

  set(favorites: AgentFavorite[]): void {
    this.favorites = favorites;
  }

  async load(): Promise<void> {
    try {
      this.favorites = await listAgentFavorites();
    } catch {
      // Nothing saved yet, or the file is unreadable — an empty list is fine.
    }
  }

  async save(favorite: AgentFavorite): Promise<void> {
    this.favorites = await saveAgentFavorite(favorite);
  }

  async remove(agentSessionId: string): Promise<void> {
    this.favorites = await removeAgentFavorite(agentSessionId);
  }

  /** The favorite the conversation open in this tab is, if it is one. */
  forTab(tabId: string): AgentFavorite | null {
    const session = tabAgentSessions.byTab[tabId];
    return session ? (this.byId.get(session.agentSessionId) ?? null) : null;
  }
}

export const favoritesStore = new FavoritesStore();

/** What a favorite goes by: its own name, or the agent and folder as a recent
 * session does. */
export function favoriteLabel(favorite: AgentFavorite): string {
  return favorite.name ?? sessionLabel(favorite);
}

/** The favorite for the conversation open in this tab: the saved one, or a new
 * one filled from what Modo IA knows about it. Null when the tab has no
 * conversation Termo can identify (which needs the agent's hooks), or one
 * whose id can't be reopened. */
export function favoriteDraftForTab(tabId: string): AgentFavorite | null {
  const session = tabAgentSessions.byTab[tabId];
  if (!session || !canRestore(session)) return null;
  const saved = favoritesStore.byId.get(session.agentSessionId);
  if (saved) return saved;
  const recent = agentSessionsStore.sessions.find((item) => item.agentSessionId === session.agentSessionId);
  return {
    tool: session.tool,
    agentSessionId: session.agentSessionId,
    name: null,
    cwd: session.cwd ?? recent?.cwd ?? null,
    projectDir: recent?.projectDir ?? null,
    model: recent?.model ?? null,
    addedAt: 0,
  };
}

/** The same for the tab that has the keyboard: the active one of the focused
 * group. */
export function favoriteDraftForFocusedTab(): AgentFavorite | null {
  if (!workspaceStore.root) return null;
  const tile = collectTiles(workspaceStore.root).find((item) => item.id === workspaceStore.focusedTileId);
  return tile?.activeSessionId ? favoriteDraftForTab(tile.activeSessionId) : null;
}
