<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import type { EditorView } from "@codemirror/view";
  import {
    SearchQuery,
    closeSearchPanel,
    findNext,
    findPrevious,
    getSearchQuery,
    replaceAll,
    replaceNext,
    setSearchQuery,
  } from "@codemirror/search";
  import { isMac } from "$lib/core/platform";
  import { t } from "$lib/i18n/i18n.svelte";

  /** Find and replace in the file, floating over its top middle. The query
   * lives in CodeMirror's search state, so its highlighting and F3 / Mod-g
   * keep working; this only stands in for its panel. */
  let {
    view,
    replacing = $bindable(false),
    focusNonce,
    revision,
  }: {
    view: EditorView;
    replacing?: boolean;
    /** Bumped to put the keyboard back in the search field. */
    focusNonce: number;
    /** Bumped on every change to the text or the selection: the count. */
    revision: number;
  } = $props();

  /** Past this the count stops: "999+". */
  const MAX_COUNT = 999;

  // Opens with what was looked for last (or the word at the cursor).
  const start = untrack(() => getSearchQuery(view.state));
  let search = $state(start.search);
  let replace = $state(start.replace);
  let caseSensitive = $state(start.caseSensitive);
  let regexp = $state(start.regexp);
  let wholeWord = $state(start.wholeWord);
  let searchField: HTMLInputElement | undefined = $state();
  let replaceField: HTMLInputElement | undefined = $state();

  const query = $derived(new SearchQuery({ search, replace, caseSensitive, regexp, wholeWord }));

  /** How many matches there are, and which one is selected (from 1, or 0). */
  const count = $derived.by(() => {
    void revision;
    if (!search || !query.valid) return { total: 0, at: 0 };
    const selection = view.state.selection.main;
    const cursor = query.getCursor(view.state);
    let total = 0;
    let at = 0;
    for (let next = cursor.next(); !next.done && total <= MAX_COUNT; next = cursor.next()) {
      total++;
      if (next.value.from === selection.from && next.value.to === selection.to) at = total;
    }
    return { total, at };
  });

  const countText = $derived.by(() => {
    if (!search) return "";
    if (!query.valid) return t("editor.search.invalid");
    if (count.total === 0) return t("editor.search.none");
    const total = count.total > MAX_COUNT ? `${MAX_COUNT}+` : String(count.total);
    return count.at ? t("editor.search.count", { n: String(count.at), total }) : t("editor.search.total", { total });
  });

  const enter = isMac ? "↩" : "Enter";
  const shiftEnter = isMac ? "⇧↩" : "Shift+Enter";

  // Every change of the query goes to the editor: what it highlights, and
  // what its own commands look for.
  $effect(() => {
    const next = query;
    if (!next.eq(getSearchQuery(view.state))) view.dispatch({ effects: setSearchQuery.of(next) });
  });

  // Asked for again (Ctrl+F with it open, a click on Find or Replace): the
  // field takes the keyboard, and the selection, when there's one, becomes
  // what's looked for.
  $effect(() => {
    void focusNonce;
    untrack(() => {
      const selection = view.state.selection.main;
      if (!regexp && !selection.empty && selection.to - selection.from < 200) {
        const text = view.state.sliceDoc(selection.from, selection.to);
        if (!text.includes("\n")) search = text;
      }
      focusField(replacing && !!search);
    });
  });

  function focusField(replaceIt: boolean) {
    void tick().then(() => {
      const field = replaceIt ? replaceField : searchField;
      field?.focus();
      field?.select();
    });
  }

  function toggleReplace() {
    replacing = !replacing;
    focusField(replacing);
  }

  function run(command: (target: EditorView) => boolean) {
    if (search && query.valid) command(view);
  }

  /** CodeMirror's replaceNext only selects a match the first time, when the
   * selection isn't one: this replaces from the first press. */
  function replaceOne(target: EditorView): boolean {
    if (count.at === 0) findNext(target);
    return replaceNext(target);
  }

  function close() {
    closeSearchPanel(view);
    view.focus();
  }

  function onKeydown(event: KeyboardEvent) {
    // The editor's own shortcuts for these don't reach here: the keyboard
    // is in a field of this, not in the editor.
    const mod = isMac ? event.metaKey : event.ctrlKey;
    const key = event.key.toLowerCase();
    if (mod && (isMac ? key === "f" && event.altKey : key === "h")) {
      event.preventDefault();
      replacing = true;
      focusField(true);
    } else if (mod && key === "f" && !event.altKey) {
      event.preventDefault();
      focusField(false);
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === "Enter" && event.target === searchField) {
      event.preventDefault();
      run(event.shiftKey ? findPrevious : findNext);
    } else if (event.key === "Enter" && event.target === replaceField) {
      event.preventDefault();
      run(event.ctrlKey || event.metaKey ? replaceAll : replaceOne);
    }
  }

  onMount(() => {
    // Looking for something the moment it opens shows the first match.
    if (search && query.valid && count.at === 0) findNext(view);
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="editor-search" role="search" aria-label={t("editor.search.label")} onkeydown={onKeydown}>
  <button
    class="icon expand"
    class:open={replacing}
    title={replacing ? t("editor.search.hideReplace") : t("editor.search.showReplace")}
    aria-label={replacing ? t("editor.search.hideReplace") : t("editor.search.showReplace")}
    aria-expanded={replacing}
    onclick={toggleReplace}
  >
    <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" stroke-width="1.3" /></svg>
  </button>

  <div class="rows">
    <div class="row">
      <div class="field" class:invalid={!!search && !query.valid}>
        <svg class="field-icon" viewBox="0 0 16 16" aria-hidden="true">
          <circle cx="7" cy="7" r="4.25" fill="none" stroke="currentColor" stroke-width="1.2" />
          <path d="M10.2 10.2 13.5 13.5" stroke="currentColor" stroke-width="1.2" />
        </svg>
        <input
          bind:this={searchField}
          bind:value={search}
          type="text"
          spellcheck="false"
          placeholder={t("editor.search.find")}
          aria-label={t("editor.search.find")}
        />
        <button class="toggle" class:on={caseSensitive} title={t("editor.search.matchCase")} aria-label={t("editor.search.matchCase")} aria-pressed={caseSensitive} onclick={() => (caseSensitive = !caseSensitive)}>Aa</button>
        <button class="toggle" class:on={wholeWord} title={t("editor.search.wholeWord")} aria-label={t("editor.search.wholeWord")} aria-pressed={wholeWord} onclick={() => (wholeWord = !wholeWord)}>
          <span class="word">ab</span>
        </button>
        <button class="toggle" class:on={regexp} title={t("editor.search.regexp")} aria-label={t("editor.search.regexp")} aria-pressed={regexp} onclick={() => (regexp = !regexp)}>.*</button>
      </div>
      <span class="count" class:none={!!search && count.total === 0} aria-live="polite">{countText}</span>
      <button class="icon" disabled={!count.total} title={`${t("editor.search.previous")} (${shiftEnter})`} aria-label={t("editor.search.previous")} onclick={() => run(findPrevious)}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 10 8 5.5l4.5 4.5" fill="none" stroke="currentColor" stroke-width="1.3" /></svg>
      </button>
      <button class="icon" disabled={!count.total} title={`${t("editor.search.next")} (${enter})`} aria-label={t("editor.search.next")} onclick={() => run(findNext)}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3.5 6 8 10.5 12.5 6" fill="none" stroke="currentColor" stroke-width="1.3" /></svg>
      </button>
      <button class="icon" title={`${t("editor.search.close")} (Esc)`} aria-label={t("editor.search.close")} onclick={close}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M4 4l8 8M12 4l-8 8" stroke="currentColor" stroke-width="1.3" /></svg>
      </button>
    </div>

    {#if replacing}
      <div class="row">
        <div class="field">
          <svg class="field-icon" viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2.5 5h8.5M8.5 2.5 11 5 8.5 7.5M13.5 11H5M7.5 8.5 5 11l2.5 2.5" fill="none" stroke="currentColor" stroke-width="1.2" />
          </svg>
          <input
            bind:this={replaceField}
            bind:value={replace}
            type="text"
            spellcheck="false"
            placeholder={t("editor.search.replace")}
            aria-label={t("editor.search.replace")}
          />
        </div>
        <button class="text-button" disabled={!count.total} title={enter} onclick={() => run(replaceOne)}>{t("editor.search.replaceOne")}</button>
        <button class="text-button" disabled={!count.total} title={isMac ? "⌘↩" : "Ctrl+Enter"} onclick={() => run(replaceAll)}>{t("editor.search.replaceAll")}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .editor-search {
    position: absolute;
    top: 56px;
    left: 50%;
    z-index: 6;
    transform: translateX(-50%);
    display: flex;
    align-items: flex-start;
    gap: 4px;
    width: min(520px, calc(100% - 32px));
    padding: 6px 6px 6px 4px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 12px;
    background: var(--tabbar-active-bg, #1e1e1e);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.4);
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 12px;
    animation: search-in 220ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  @keyframes search-in {
    from {
      opacity: 0;
      transform: translate(-50%, -6px) scale(0.98);
    }
  }

  .rows {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .field {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 2px;
    height: 28px;
    padding: 0 3px 0 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    transition: border-color 120ms ease;
  }

  .field:focus-within {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field.invalid {
    border-color: #e05a5a;
  }

  .field-icon {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
    opacity: 0.5;
  }

  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0 4px;
    border: none;
    background: transparent;
    color: inherit;
    font: inherit;
    outline: none;
  }

  .toggle {
    flex-shrink: 0;
    min-width: 24px;
    height: 22px;
    padding: 0 4px;
    border: 1px solid transparent;
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    opacity: 0.6;
    cursor: pointer;
  }

  .toggle:hover {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .toggle.on {
    opacity: 1;
    border-color: color-mix(in srgb, var(--pane-active-border, #3a6df0) 60%, transparent);
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 22%, transparent);
  }

  .word {
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .count {
    flex-shrink: 0;
    min-width: 64px;
    padding: 0 4px;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    text-align: center;
    opacity: 0.65;
    white-space: nowrap;
  }

  .count.none {
    color: #e05a5a;
    opacity: 1;
  }

  .icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    opacity: 0.75;
    cursor: pointer;
  }

  .icon svg {
    width: 14px;
    height: 14px;
  }

  .icon:hover:not(:disabled) {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .icon:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .expand {
    height: 28px;
  }

  .expand svg {
    transition: transform 200ms cubic-bezier(0.22, 1, 0.36, 1);
  }

  .expand.open svg {
    transform: rotate(90deg);
  }

  .text-button {
    flex-shrink: 0;
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .text-button:hover:not(:disabled) {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .text-button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  @media (prefers-reduced-motion: reduce) {
    .editor-search {
      animation: none;
    }

    .expand svg {
      transition: none;
    }
  }
</style>
