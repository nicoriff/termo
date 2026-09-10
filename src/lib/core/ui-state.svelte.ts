class UiState {
  settingsOpen = $state(false);
  maximized = $state(false);

  toggleSettings(): void {
    this.settingsOpen = !this.settingsOpen;
  }

  setMaximized(value: boolean): void {
    this.maximized = value;
  }
}

export const uiState = new UiState();
