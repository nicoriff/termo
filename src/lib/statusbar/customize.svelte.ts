import { hidePreferencesWindow, openPreferencesWindow } from "$lib/core/window-client";
import { uiState } from "$lib/core/ui-state.svelte";

/** Whether this window's status bar is being arranged: the rest of the window
 * dims, and the bar takes items dragged from a tray over it (StatusBarEditor). */
class StatusBarCustomize {
  active = $state(false);
  /** Started from Preferences, which stepped aside for it and comes back
   * when the user is done. */
  private fromPreferences = false;

  start(fromPreferences = false) {
    this.fromPreferences = fromPreferences;
    this.active = true;
    // From the bar's menu, Preferences may be open (it comes forward with
    // this window and would cover the bar); it isn't brought back after.
    if (!fromPreferences) void hidePreferencesWindow().catch(() => {});
  }

  finish() {
    if (!this.active) return;
    this.active = false;
    if (this.fromPreferences) void openPreferencesWindow("status-bar");
    else uiState.refocusTerminals();
    this.fromPreferences = false;
  }
}

export const statusBarCustomize = new StatusBarCustomize();
