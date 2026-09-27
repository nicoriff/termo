/**
 * What the rest of the app needs to know about open file tabs: which have
 * unsaved changes (the tab strip marks them, closing one asks first), how to
 * save one from outside its editor (that close prompt), and when anything
 * was saved (the explorer refreshes its git marks).
 */
class FileTabs {
  /** Keyed by tab id; only tabs with unsaved changes are present. */
  dirty = $state<Record<string, boolean>>({});
  /** Bumped on every save, for whoever shows something a save changes. */
  savedTick = $state(0);
  private savers = new Map<string, () => Promise<boolean>>();

  register(tabId: string, save: () => Promise<boolean>): void {
    this.savers.set(tabId, save);
  }

  unregister(tabId: string): void {
    this.savers.delete(tabId);
    this.setDirty(tabId, false);
  }

  setDirty(tabId: string, dirty: boolean): void {
    if (!!this.dirty[tabId] === dirty) return;
    const next = { ...this.dirty };
    if (dirty) next[tabId] = true;
    else delete next[tabId];
    this.dirty = next;
  }

  isDirty(tabId: string): boolean {
    return !!this.dirty[tabId];
  }

  /** Saves that tab's file; false when it couldn't (the editor shows why). */
  async save(tabId: string): Promise<boolean> {
    return (await this.savers.get(tabId)?.()) ?? false;
  }

  noteSaved(): void {
    this.savedTick++;
  }
}

export const fileTabs = new FileTabs();
