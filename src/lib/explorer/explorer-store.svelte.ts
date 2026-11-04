import { sidePanelStore } from "$lib/app/side-panel-store.svelte";

/** Whether the file explorer is showing: a section of the side panel docked
 * at the window's left edge, showing the repository the focused tab is in. */
class ExplorerStore {
  get open(): boolean {
    return sidePanelStore.isShown("explorer");
  }

  set open(open: boolean) {
    this.setOpen(open);
  }

  toggle(): void {
    sidePanelStore.toggle("explorer");
  }

  setOpen(open: boolean): void {
    if (open) sidePanelStore.show("explorer");
    else sidePanelStore.close("explorer");
  }
}

export const explorerStore = new ExplorerStore();
