import { getContext, setContext, type Snippet } from "svelte";

/** What a section shows in its accordion header besides its name and icon:
 * its own buttons, and a word of context (the repository in view, say). */
export class SectionHost {
  actions = $state<Snippet | null>(null);
  note = $state("");
}

const KEY = Symbol("side-panel-section");

/** Called by the column around each section it mounts. */
export function provideSectionHost(host: SectionHost): void {
  setContext(KEY, host);
}

/**
 * Called by a section while it starts: hands its header's buttons (a
 * snippet of its own, so they keep its state and styles) and its note to the
 * accordion header above it, for as long as it's mounted.
 */
export function sectionHeader(header: { actions: Snippet; note?: () => string }): void {
  const host = getContext<SectionHost | undefined>(KEY);
  if (!host) return;
  $effect(() => {
    host.actions = header.actions;
    host.note = header.note?.() ?? "";
  });
  $effect(() => () => {
    host.actions = null;
    host.note = "";
  });
}
