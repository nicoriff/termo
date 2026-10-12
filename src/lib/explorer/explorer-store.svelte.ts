import { agentsStore } from "$lib/agents/agents-store.svelte";
import { gitStore } from "$lib/git/git-store.svelte";

/** Whether the file explorer is open: a side panel docked at the window's
 * left edge, showing the repository the focused tab is in. It shares that
 * edge with the agents panel and the Git panel, so opening one closes the
 * others. */
class ExplorerStore {
  open = $state(false);

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    if (open) {
      agentsStore.setOpen(false);
      gitStore.open = false;
    }
    this.open = open;
  }
}

export const explorerStore = new ExplorerStore();
