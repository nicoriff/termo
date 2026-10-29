/** Code loaded the first time it's shown — the editors, with CodeMirror —
 * instead of with the bundle every window parses before it can start. The
 * loads still under way are tracked, so a window starting up can wait for
 * what it shows before it shows (see app/startup.ts). */
const pending = new Set<Promise<unknown>>();

/** `load`, run once, on the first call. */
export function lazy<T>(load: () => Promise<T>): () => Promise<T> {
  let loaded: Promise<T> | null = null;
  return () => {
    if (loaded) return loaded;
    const loading = load();
    loaded = loading;
    pending.add(loading);
    loading.then(
      () => pending.delete(loading),
      () => pending.delete(loading),
    );
    return loading;
  };
}

/** Once every load under way — and any one they start meanwhile — is done,
 * whether it worked or not. */
export async function lazyLoadsSettled(): Promise<void> {
  while (pending.size > 0) await Promise.allSettled([...pending]);
}
