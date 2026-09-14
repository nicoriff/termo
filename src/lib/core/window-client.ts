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

export function openPreferencesWindow(): Promise<void> {
  return invoke("open_preferences_window");
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

/** App icon badge (macOS Dock, some Linux docks); unsupported on Windows. */
export function setBadgeCount(count?: number): Promise<void> {
  return getCurrentWindow().setBadgeCount(count);
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

export function setWindowSize(width: number, height: number): Promise<void> {
  return getCurrentWindow().setSize(new PhysicalSize(width, height));
}
