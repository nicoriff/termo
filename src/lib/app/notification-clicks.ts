import { onNotificationOpened, type NotificationTarget } from "$lib/core/notification-client";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { openPreferencesWindow } from "$lib/core/window-client";

function open(target: NotificationTarget) {
  switch (target.kind) {
    case "session":
      // Closed since: the window coming forward is all there is to show.
      workspaceStore.focusSession(target.sessionId);
      break;
    case "agentRun":
      workspaceStore.openAgentRun(target.agentId, target.runId, target.title);
      break;
    case "orchestration":
      workspaceStore.openOrchestration(target.orchestrationId, target.title);
      break;
    case "preferences":
      void openPreferencesWindow(target.section);
      break;
  }
}

/** Clicking a notification leads to what it was about: the tab that needs
 * attention, the agent's run, the orchestration. Returns a teardown function. */
export function startNotificationClicks(): () => void {
  const unlisten = onNotificationOpened(open);
  return () => {
    void unlisten.then((stop) => stop());
  };
}
