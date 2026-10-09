import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Appearance } from "./theme-types";

/** Each webview observes the OS independently. This never saves preferences
 * or forces a native window theme, which would override the OS signal. */
export function observeSystemAppearance(onChange: (appearance: Appearance) => void): {
  ready: Promise<void>;
  stop: () => void;
} {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  let stopped = false;
  let nativeActive = false;
  let nativeRevision = 0;
  let unlisten: (() => void) | undefined;

  const applyMedia = () => {
    if (!stopped && !nativeActive) onChange(media.matches ? "dark" : "light");
  };
  // Older WKWebView versions implement only the legacy MediaQueryList API.
  if (media.addEventListener) media.addEventListener("change", applyMedia);
  else media.addListener(applyMedia);
  applyMedia();

  const ready = (async () => {
    if (!isTauri()) return;
    let currentWindow: ReturnType<typeof getCurrentWindow>;
    try {
      currentWindow = getCurrentWindow();
    } catch (error) {
      console.warn("Could not access the native window for system appearance; using the media query.", error);
      return;
    }

    unlisten = await currentWindow.onThemeChanged(({ payload }) => {
      if (stopped) return;
      nativeRevision++;
      nativeActive = true;
      onChange(payload);
    }).catch((error) => {
      console.warn("Could not subscribe to native appearance changes; using the media query.", error);
      return undefined;
    });
    if (stopped) {
      unlisten?.();
      return;
    }
    const revision = nativeRevision;
    const appearance = await currentWindow.theme().catch((error) => {
      console.warn("Could not read the native system appearance; keeping the appearance observer active.", error);
      return null;
    });
    // A change received while reading takes precedence over the older snapshot.
    if (!stopped && nativeRevision === revision && appearance !== null) {
      nativeActive = unlisten !== undefined;
      onChange(appearance);
    }
  })();

  return {
    ready,
    stop: () => {
      if (stopped) return;
      stopped = true;
      if (media.removeEventListener) media.removeEventListener("change", applyMedia);
      else media.removeListener(applyMedia);
      unlisten?.();
      unlisten = undefined;
    },
  };
}
