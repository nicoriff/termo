class UiState {
  maximized = $state(false);
  commandPaletteOpen = $state(false);
  /** Bumped to ask the focused terminal to take keyboard focus back after an
   * overlay (the command palette) had it — Terminal.svelte only focuses on its
   * own when the active tab changes, which doesn't happen here. */
  terminalFocusNonce = $state(0);

  setMaximized(value: boolean): void {
    this.maximized = value;
  }

  openCommandPalette(): void {
    this.commandPaletteOpen = true;
  }

  closeCommandPalette(): void {
    this.commandPaletteOpen = false;
  }

  refocusTerminals(): void {
    this.terminalFocusNonce++;
  }
}

export const uiState = new UiState();
