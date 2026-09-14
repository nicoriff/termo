import { invoke } from "@tauri-apps/api/core";
import { Effect, EffectState, getCurrentWindow } from "@tauri-apps/api/window";
import { isMac } from "./platform";
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

/** Native blur behind the window's translucent areas: macOS vibrancy (HUD
 * material, corner radius matching the window's 10px CSS radius) or Windows
 * Acrylic. Tauri has no Linux support, so the call rejects there. */
export function setBackgroundBlur(enabled: boolean): Promise<void> {
  const window = getCurrentWindow();
  if (!enabled) return window.clearEffects();
  return window.setEffects({
    effects: [isMac ? Effect.HudWindow : Effect.Acrylic],
    state: EffectState.Active,
    radius: 10,
  });
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
