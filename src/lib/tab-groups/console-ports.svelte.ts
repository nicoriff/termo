import { invoke } from "@tauri-apps/api/core";
import { getTabId } from "$lib/terminal/pty-registry";

export interface ServedPort {
  port: number;
  /** It answered an HTTP request: it opens in the browser. */
  http: boolean;
}

interface ConsolePorts {
  id: string;
  ports: ServedPort[];
}

/** How often the panel asks, while it's open. */
const EVERY_MS = 3000;

/**
 * The ports something in each console listens on (see pty/ports.rs), by
 * tab id: asked for only while someone shows them, since it reads the
 * whole process and socket tables.
 */
class ConsolePortsStore {
  byTab = $state<Record<string, ServedPort[]>>({});
  private watchers = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  /** A poll is asked for or due: watching again mustn't start a second one. */
  private looping = false;

  /** Starts asking; the function it returns stops (when nobody else asks). */
  watch(): () => void {
    this.watchers += 1;
    if (!this.looping) {
      this.looping = true;
      void this.poll();
    }
    return () => {
      this.watchers -= 1;
      if (this.watchers > 0 || !this.timer) return;
      clearTimeout(this.timer);
      this.timer = null;
      this.looping = false;
    };
  }

  private async poll(): Promise<void> {
    this.timer = null;
    try {
      const list = await invoke<ConsolePorts[]>("console_ports");
      // By tab: the backend knows its consoles by their own ids. Other
      // windows' consoles have no tab here.
      const byTab: Record<string, ServedPort[]> = {};
      for (const entry of list) {
        const tabId = getTabId(entry.id);
        if (tabId) byTab[tabId] = entry.ports;
      }
      this.byTab = byTab;
    } catch {
      // Not this time: the next poll tries again.
    }
    if (this.watchers > 0) this.timer = setTimeout(() => void this.poll(), EVERY_MS);
    else this.looping = false;
  }
}

export const consolePortsStore = new ConsolePortsStore();
