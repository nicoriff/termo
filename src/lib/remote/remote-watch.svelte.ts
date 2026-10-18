import { invoke } from "@tauri-apps/api/core";
import { notify } from "$lib/core/notification-client";
import type { RemotePending, RemoteViewer } from "$lib/core/remote-client";
import { t } from "$lib/i18n/i18n.svelte";

/** How often it asks: who's connected, who's waiting to. */
const POLL_MS = 5000;

interface RemoteActivity {
  enabled: boolean;
  pending: RemotePending[];
  viewers: RemoteViewer[];
}

/**
 * Keeps an eye on remote access while it's on (see src-tauri/src/mux/
 * remote.rs): who is connected from the tailnet now, for the status bar,
 * and someone new asking to — a notification, clicked, opens where to
 * answer them.
 */
class RemoteWatch {
  viewers = $state<RemoteViewer[]>([]);
  /** The pending requests already told about (or there at startup). */
  private told: Set<string> | null = null;

  start(notifyPending: boolean): () => void {
    void this.poll(notifyPending);
    const timer = setInterval(() => void this.poll(notifyPending), POLL_MS);
    return () => clearInterval(timer);
  }

  private async poll(notifyPending: boolean): Promise<void> {
    const activity = await invoke<RemoteActivity>("remote_activity").catch(() => null);
    if (!activity) return;
    this.viewers = activity.viewers;
    const keys = activity.pending.map((entry) => `${entry.login}|${entry.node}|${entry.at}`);
    if (this.told === null) {
      // What was waiting before this window opened isn't news.
      this.told = new Set(keys);
      return;
    }
    for (const [index, entry] of activity.pending.entries()) {
      if (this.told.has(keys[index])) continue;
      this.told.add(keys[index]);
      if (!notifyPending) continue;
      void notify(
        t("remote.pendingNotification", { login: entry.login, node: entry.node }),
        t("remote.pendingNotificationBody"),
        { kind: "preferences", section: "remote" },
      ).catch(() => {});
    }
  }
}

export const remoteWatch = new RemoteWatch();
