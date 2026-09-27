import type { SavedLayout } from "./layouts-client";
import type { LaunchTarget } from "$lib/agent-launch/launch-target";

class UiState {
  maximized = $state(false);
  commandPaletteOpen = $state(false);
  /** The "save layout" name prompt, opened from the title bar menu or the
   * palette. Like the palette, it owns the keyboard while it's up. */
  saveLayoutPromptOpen = $state(false);
  /** The layout waiting on its "this closes everything you have open"
   * confirmation, set from both places that can restore one. */
  restoreLayoutTarget = $state<SavedLayout | null>(null);
  /** The layout waiting on its "this can't be undone" delete confirmation,
   * set from the command palette (the layout manager has its own inline
   * two-click confirm instead). */
  deleteLayoutTarget = $state<SavedLayout | null>(null);
  /** The "saved layouts" window, where they're picked and deleted. */
  layoutManagerOpen = $state(false);
  /** The "About Termo" window, opened from the title bar menu. */
  aboutOpen = $state(false);
  /** The "where should this run" dialog, opened from a tile's new-tab menu:
   * what to open (an agent or a shell), which tile gets the tab, and the
   * folder that tile's console is in (null when it hasn't reported one). */
  agentLaunch = $state<{ target: LaunchTarget; tileId: string; here: string | null } | null>(null);
  /** A file tab with unsaved changes the user asked to close: waiting on
   * save / don't save / cancel. */
  closeFileTarget = $state<{ tileId: string; sessionId: string; name: string } | null>(null);
  /** The extensions browser, sliding in from the right. Part of
   * `overlayOpen` below: it owns a search box, so every window-level
   * shortcut has to stand down while it's up. */
  marketplaceOpen = $state(false);
  /** What quitting would interrupt, when it's worth asking about (see
   * app/quit.ts). Null while no such question is on screen. */
  quitPrompt = $state<{ consoles: number; agents: number } | null>(null);
  /** Where the console's right-click Edit menu is open, in window
   * coordinates. Deliberately not part of `overlayOpen`: it never takes the
   * keyboard from the console, so shortcuts keep working under it. */
  terminalContextMenu = $state<{ x: number; y: number } | null>(null);
  /** Bumped to ask the focused terminal to take keyboard focus back after an
   * overlay (the command palette) had it — Terminal.svelte only focuses on its
   * own when the active tab changes, which doesn't happen here. */
  terminalFocusNonce = $state(0);

  /** Whether an overlay currently owns the keyboard, so the window-level
   * handlers (tile picker, AI Mode panel) stay out of the way. */
  get overlayOpen(): boolean {
    return (
      this.commandPaletteOpen ||
      this.saveLayoutPromptOpen ||
      this.layoutManagerOpen ||
      this.aboutOpen ||
      this.agentLaunch !== null ||
      this.closeFileTarget !== null ||
      this.marketplaceOpen ||
      this.restoreLayoutTarget !== null ||
      this.deleteLayoutTarget !== null ||
      this.quitPrompt !== null
    );
  }

  setMaximized(value: boolean): void {
    this.maximized = value;
  }

  openCommandPalette(): void {
    this.commandPaletteOpen = true;
  }

  closeCommandPalette(): void {
    this.commandPaletteOpen = false;
  }

  openSaveLayoutPrompt(): void {
    this.saveLayoutPromptOpen = true;
  }

  closeSaveLayoutPrompt(): void {
    this.saveLayoutPromptOpen = false;
  }

  /** Restoring never happens straight away: it closes every console that's
   * open, so it goes through a confirmation first. */
  requestRestoreLayout(layout: SavedLayout): void {
    this.restoreLayoutTarget = layout;
  }

  cancelRestoreLayout(): void {
    this.restoreLayoutTarget = null;
  }

  /** Deleting can't be undone, so the palette goes through a confirmation
   * too, same as restoring. */
  requestDeleteLayout(layout: SavedLayout): void {
    this.deleteLayoutTarget = layout;
  }

  cancelDeleteLayout(): void {
    this.deleteLayoutTarget = null;
  }

  askBeforeQuit(counts: { consoles: number; agents: number }): void {
    this.quitPrompt = counts;
  }

  cancelQuit(): void {
    this.quitPrompt = null;
  }

  openLayoutManager(): void {
    this.layoutManagerOpen = true;
  }

  closeLayoutManager(): void {
    this.layoutManagerOpen = false;
  }

  openAbout(): void {
    this.aboutOpen = true;
  }

  closeAbout(): void {
    this.aboutOpen = false;
  }

  openAgentLaunch(target: LaunchTarget, tileId: string, here: string | null): void {
    this.agentLaunch = { target, tileId, here };
  }

  requestCloseFile(target: { tileId: string; sessionId: string; name: string }): void {
    this.closeFileTarget = target;
  }

  cancelCloseFile(): void {
    this.closeFileTarget = null;
  }

  closeAgentLaunch(): void {
    this.agentLaunch = null;
  }

  openMarketplace(): void {
    this.marketplaceOpen = true;
  }

  closeMarketplace(): void {
    this.marketplaceOpen = false;
    // The search box took focus away from the console; hand it back.
    this.refocusTerminals();
  }

  openTerminalContextMenu(x: number, y: number): void {
    this.terminalContextMenu = { x, y };
  }

  closeTerminalContextMenu(): void {
    this.terminalContextMenu = null;
  }

  refocusTerminals(): void {
    this.terminalFocusNonce++;
  }
}

export const uiState = new UiState();
