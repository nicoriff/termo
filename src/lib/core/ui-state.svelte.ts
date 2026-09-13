class UiState {
  maximized = $state(false);

  setMaximized(value: boolean): void {
    this.maximized = value;
  }
}

export const uiState = new UiState();
