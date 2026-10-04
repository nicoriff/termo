import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Appearance } from "./theme-types";

/** Each webview observes the OS independently. This never saves preferences
 * or forces a native window theme, which would override the OS signal. */
export function observeSystemAppearance(onChange: (appearance: Appearance) => void): {
  ready: Promise<void>;
  stop: () => void;
} {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  let disposed = false;
  let nativeActive = false;
  let nativeRevision = 0;
  let unlisten: (() => void) | undefined;

  const applyMedia = () => {
    if (!disposed && !nativeActive) onChange(media.matches ? "dark" : "light");
  };
  // Older WKWebView versions implement only the legacy MediaQueryList API.
  if (media.addEventListener) media.addEventListener("change", applyMedia);
  else media.addListener(applyMedia);
  applyMedia();

  const ready = (async () => {
    try {
      const currentWindow = getCurrentWindow();
      let listening = false;
      try {
        const remove = await currentWindow.onThemeChanged(({ payload }) => {
          if (disposed) return;
          nativeRevision++;
          nativeActive = true;
          onChange(payload);
        });
        if (disposed) {
          remove();
          return;
        }
        unlisten = remove;
        listening = true;
      } catch {
        // The media query can still track changes if native events are unavailable.
      }
      if (disposed) return;
      const revision = nativeRevision;
      const appearance = await currentWindow.theme();
      // A change received while reading takes precedence over the older snapshot.
      if (!disposed && nativeRevision === revision && appearance !== null) {
        nativeActive = listening;
        onChange(appearance);
      }
    } catch {
      // Also works in browser previews without the Tauri runtime.
    }
  })();

  return {
    ready,
    stop: () => {
      disposed = true;
      if (media.removeEventListener) media.removeEventListener("change", applyMedia);
      else media.removeListener(applyMedia);
      unlisten?.();
      unlisten = undefined;
    },
  };
}
