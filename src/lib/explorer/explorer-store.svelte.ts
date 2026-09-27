/** Which tiles have their file explorer open. Per tile, so each group of
 * tabs shows the repository its own console is in. */
class ExplorerStore {
  open = $state<Record<string, boolean>>({});

  toggle(tileId: string): void {
    this.open = { ...this.open, [tileId]: !this.open[tileId] };
  }

  close(tileId: string): void {
    if (!this.open[tileId]) return;
    this.open = { ...this.open, [tileId]: false };
  }
}

export const explorerStore = new ExplorerStore();
