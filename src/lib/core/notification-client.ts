import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

/** Where clicking a notification leads, in the window that sent it. */
export type NotificationTarget =
  | { kind: "session"; sessionId: string }
  | { kind: "agentRun"; agentId: string; runId: string; title: string }
  | { kind: "orchestration"; orchestrationId: string; title: string }
  /** A page of Preferences (see Preferences.svelte's "preferences-section"). */
  | { kind: "preferences"; section: string };

/** A system notification that, clicked, brings this window forward and
 * comes back through `onNotificationOpened` (Windows; elsewhere the click
 * isn't heard — see notification_commands.rs). */
export function notify(title: string, body: string, target: NotificationTarget): Promise<void> {
  return invoke("notify", { title, body, target });
}

/** What a notification Termo was started from was about (one the agents'
 * own process showed with Termo closed), once. */
export function takeOpenedNotification(): Promise<NotificationTarget | null> {
  return invoke("notification_take_opened");
}

/** One of this window's notifications was clicked. */
export function onNotificationOpened(handler: (target: NotificationTarget) => void): Promise<() => void> {
  return getCurrentWebviewWindow().listen<NotificationTarget>("notification-opened", ({ payload }) => handler(payload));
}
