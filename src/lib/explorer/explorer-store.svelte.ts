import { agentsStore } from "$lib/agents/agents-store.svelte";
import { gitStore } from "$lib/git/git-store.svelte";
import { harnessStore } from "$lib/harness/harness-store.svelte";
import { tabGroupsStore } from "$lib/tab-groups/tab-groups-store.svelte";

/** Whether the file explorer is open: a side panel docked at the window's
 * left edge, showing the repository the focused tab is in. It shares that
 * edge with the agents panel, the Git panel and the Harness panel, so opening
 * one closes the others. */
class ExplorerStore {
  open = $state(false);

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    if (open) {
      agentsStore.setOpen(false);
      gitStore.open = false;
      harnessStore.setOpen(false);
      tabGroupsStore.open = false;
    }
    this.open = open;
  }
}

export const explorerStore = new ExplorerStore();
