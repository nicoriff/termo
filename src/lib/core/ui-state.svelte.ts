import type { SavedLayout } from "./layouts-client";

class UiState {
  maximized = $state(false);
  commandPaletteOpen = $state(false);
  /** The "save layout" name prompt, opened from the title bar menu or the
   * palette. Like the palette, it owns the keyboard while it's up. */
  saveLayoutPromptOpen = $state(false);
  /** The layout waiting on its "this closes everything you have open"
   * confirmation, set from both places that can restore one. */
  restoreLayoutTarget = $state<SavedLayout | null>(null);
  /** The "saved layouts" window, where they're picked and deleted. */
  layoutManagerOpen = $state(false);
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
      this.restoreLayoutTarget !== null
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

  openLayoutManager(): void {
    this.layoutManagerOpen = true;
  }

  closeLayoutManager(): void {
    this.layoutManagerOpen = false;
  }

  refocusTerminals(): void {
    this.terminalFocusNonce++;
  }
}

export const uiState = new UiState();
