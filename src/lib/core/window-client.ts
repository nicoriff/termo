import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PhysicalSize } from "@tauri-apps/api/dpi";

export function minimizeWindow(): Promise<void> {
  return invoke("minimize_window");
}

export function toggleMaximizeWindow(): Promise<void> {
  return invoke("toggle_maximize");
}

export function closeWindow(): Promise<void> {
  return invoke("close_window");
}

/** `section`: the page to show, when one in particular is wanted. */
export function openPreferencesWindow(section?: string): Promise<void> {
  return invoke("open_preferences_window", { section: section ?? null });
}

/** From Preferences: arranges the status bar on the console window it was
 * opened over. Preferences hides meanwhile, and comes back when that's done. */
export function customizeStatusBar(): Promise<void> {
  return invoke("customize_status_bar");
}

/** Hides Preferences if it's open (it stays in front of the console window). */
export function hidePreferencesWindow(): Promise<void> {
  return invoke("hide_preferences_window");
}

/** Ends the app. Not the same as closing this window: the hidden, preloaded
 * Preferences window would keep the process alive (see quit_app). */
export function quitApp(): Promise<void> {
  return invoke("quit_app");
}

/** Tells the backend the (preloaded, hidden) Preferences page has applied its
 * config and painted, so it can be shown without flashing defaults. */
export function preferencesReady(): Promise<void> {
  return invoke("preferences_ready");
}

/** Tells the backend this console window's tabs are in place, so the window,
 * built hidden, can show whole (see startup.rs). `marks` are the page's
 * startup timings, in ms since it started loading, for startup.log. */
export function windowReady(marks: [string, number][]): Promise<void> {
  return invoke("window_ready", { marks });
}

/** GTK's titlebar button layout, e.g. "appmenu:minimize,maximize,close";
 * null off Linux. See decoration_layout in window_commands.rs. */
export function getDecorationLayout(): Promise<string | null> {
  return invoke("decoration_layout");
}

export function isWindowMaximized(): Promise<boolean> {
  return invoke("is_maximized");
}

/** Native blur behind this window's translucent areas (macOS vibrancy,
 * Windows Acrylic; no-op on Linux). Goes through a Rust command rather than
 * the JS setEffects/clearEffects API: clearEffects never removes vibrancy
 * on macOS — see set_background_blur in window_commands.rs. */
export function setBackgroundBlur(enabled: boolean): Promise<void> {
  return invoke("set_background_blur", { enabled });
}

/** This window's sessions waiting on the user. The app icon's badge (macOS
 * Dock, some Linux docks; unsupported on Windows) is the app's, so the
 * backend shows the sum of every window's (see terminal_windows.rs). */
export function reportPending(count: number): Promise<void> {
  return invoke("report_pending", { count });
}

/** One more console window, with its own tabs and panes. */
export function openNewWindow(): Promise<void> {
  return invoke("open_new_window");
}

/** Whether this is the window that speaks for all of them, where only one
 * should (the notice that a background agent finished, say). */
export function isPrimaryWindow(): Promise<boolean> {
  return invoke("is_primary_window");
}

/** A console window, as opposed to Preferences: `main`, `main-2`… */
export function isConsoleWindow(): boolean {
  return getCurrentWindow().label !== "preferences";
}

/** The first console window, the one Termo opens with: the only one that
 * restores the default layout and shows what an update brought. */
export function isFirstWindow(): boolean {
  return getCurrentWindow().label === "main";
}

export function isWindowFullscreen(): Promise<boolean> {
  return getCurrentWindow().isFullscreen();
}

export function startDragging(): Promise<void> {
  return getCurrentWindow().startDragging();
}

export type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

export function startResizeDragging(direction: ResizeDirection): Promise<void> {
  return getCurrentWindow().startResizeDragging(direction);
}

export function onWindowResized(handler: () => void): Promise<() => void> {
  return getCurrentWindow().onResized(handler);
}

export async function getWindowSize(): Promise<{ width: number; height: number }> {
  const size = await getCurrentWindow().innerSize();
  return { width: size.width, height: size.height };
}
