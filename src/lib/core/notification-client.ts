import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Where clicking a notification leads, in the window that sent it. */
export type NotificationTarget =
  | { kind: "session"; sessionId: string }
  | { kind: "agentRun"; agentId: string; runId: string; title: string }
  | { kind: "orchestration"; orchestrationId: string; title: string };

/** A system notification that, clicked, brings this window forward and
 * comes back through `onNotificationOpened` (Windows; elsewhere the click
 * isn't heard — see notification_commands.rs). */
export function notify(title: string, body: string, target: NotificationTarget): Promise<void> {
  return invoke("notify", { title, body, target });
}

/** One of this window's notifications was clicked. */
export function onNotificationOpened(handler: (target: NotificationTarget) => void): Promise<() => void> {
  return getCurrentWebviewWindow().listen<NotificationTarget>("notification-opened", ({ payload }) => handler(payload));
}
