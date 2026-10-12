<script lang="ts">
  import { onMount } from "svelte";
  import { basicSetup } from "codemirror";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView } from "@codemirror/view";
  import { LanguageDescription } from "@codemirror/language";
  import { languages } from "@codemirror/language-data";
  import { unifiedMergeView } from "@codemirror/merge";
  import { readTextFile } from "$lib/core/explorer-client";
  import { gitBlob } from "$lib/core/git-client";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { fileName, type DiffTarget } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { diffJump } from "./diff-jump.svelte";
  import { editorTheme } from "./editor-theme";
  import { fileTabs } from "./file-tabs.svelte";

  /** What changed in a file between two of its versions (see DiffTarget):
   * the newer one in full, with what was taken out shown in place, read only.
   * A diff against the file on disk follows it as it's saved. */
  let { diff, active = false }: { diff: DiffTarget; active?: boolean } = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  let error = $state<string | null>(null);
  let loaded = $state(false);
  let same = $state(false);
  let stats = $state({ added: 0, removed: 0 });
  /** What the view was last built from, to tell a real change from a poll. */
  let shown: { before: string; after: string } | null = null;
  /** Brings the line a finding is on into view, once: the file is on
   * screen, and the request is for this file. */
  function jump() {
    if (!view) return;
    const asked = diffJump.take(diff);
    if (asked === null) return;
    const line = view.state.doc.line(Math.min(Math.max(1, asked), view.state.doc.lines));
    view.dispatch({ selection: { anchor: line.from }, effects: EditorView.scrollIntoView(line.from, { y: "center" }) });
  }

  const themeSlot = new Compartment();
  const languageSlot = new Compartment();
  const separator = $derived(diff.root.includes("\\") ? "\\" : "/");
  const absPath = $derived(`${diff.root}${separator}${diff.path.replaceAll("/", separator)}`);
  /** Only a diff that ends on disk or in the index can move under it. */
  const live = $derived(diff.to === null || diff.to === "" || diff.from === "");

  function sideLabel(rev: string | null): string {
    if (rev === null) return t("git.diff.disk");
    if (rev === "") return t("git.diff.index");
    return rev === "HEAD" ? t("git.diff.head") : rev.slice(0, 7);
  }
  const fromLabel = $derived(diff.from === null ? t("git.diff.nothing") : sideLabel(diff.from));
  const toLabel = $derived(sideLabel(diff.to));

  function fontFamily(family: string): string {
    return family.includes(",") ? family : `"${family}", monospace`;
  }

  function currentTheme() {
    const c = themeStore.current.colors;
    return [
      editorTheme(themeStore.current, {
        family: fontFamily(themeStore.effectiveFontFamily),
        size: themeStore.effectiveFontSize,
      }),
      // The theme's own green and red over what the merge view marks.
      EditorView.theme({
        "&.cm-merge-b .cm-changedLine, .cm-changedLine": { backgroundColor: `${c.green}22` },
        "&.cm-merge-b .cm-changedText, .cm-changedText": { background: `${c.green}44` },
        ".cm-deletedChunk": { backgroundColor: `${c.red}22` },
        ".cm-deletedChunk .cm-deletedText, .cm-deletedText": { background: `${c.red}44` },
        ".cm-changeGutter": { width: "3px" },
        ".cm-changedLineGutter": { background: c.green },
        ".cm-deletedLineGutter": { background: c.red },
        ".cm-collapsedLines": { color: c.brightBlack, background: "transparent" },
      }),
    ];
  }

  /** A version of the file; null when it isn't there (new, or deleted). */
  async function read(rev: string | null, path: string): Promise<string | null> {
    if (rev !== null) return gitBlob(diff.root, rev, path);
    // A file git reports as deleted reads as nothing rather than as a failure.
    return readTextFile(absPath).catch((reason) => {
      const known = gitStore.root === diff.root ? gitStore.snapshot?.files.find((file) => file.path === path) : undefined;
      if (known?.unstaged === "deleted" || known?.staged === "deleted") return null;
      throw reason;
    });
  }

  /** Added and removed lines, counted the cheap way: by what each side has
   * that the other doesn't. Enough for the header's "+12 −3". */
  function countLines(before: string, after: string) {
    const tally = new Map<string, number>();
    for (const line of before.split("\n")) tally.set(line, (tally.get(line) ?? 0) + 1);
    let added = 0;
    for (const line of after.split("\n")) {
      const left = tally.get(line) ?? 0;
      if (left > 0) tally.set(line, left - 1);
      else added++;
    }
    let removed = 0;
    for (const left of tally.values()) removed += left;
    return { added, removed };
  }

  async function load() {
    let before: string | null;
    let after: string | null;
    try {
      [before, after] = await Promise.all([
        diff.from === null ? Promise.resolve(null) : read(diff.from, diff.fromPath ?? diff.path),
        read(diff.to, diff.path),
      ]);
    } catch (reason) {
      error = String(reason);
      loaded = true;
      return;
    }
    if (!container) return;
    // CRLF and LF read the same: only what the lines say counts.
    const [a, b] = [(before ?? "").replaceAll("\r\n", "\n"), (after ?? "").replaceAll("\r\n", "\n")];
    if (shown && shown.before === a && shown.after === b) return;
    shown = { before: a, after: b };
    error = null;
    same = a === b;
    stats = countLines(a, b);
    loaded = true;

    const scroll = view?.scrollDOM.scrollTop ?? 0;
    view?.destroy();
    view = new EditorView({
      parent: container,
      state: EditorState.create({
        doc: b,
        extensions: [
          basicSetup,
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          // The merge view's own words, in the app's language.
          EditorState.phrases.of({ "$ unchanged lines": t("git.diff.unchanged", { n: "$" }) }),
          themeSlot.of(currentTheme()),
          languageSlot.of([]),
          unifiedMergeView({
            original: a,
            mergeControls: false,
            gutter: true,
            collapseUnchanged: { margin: 3, minSize: 6 },
          }),
        ],
      }),
    });
    view.scrollDOM.scrollTop = scroll;
    jump();

    const description = LanguageDescription.matchFilename(languages, fileName(diff.path));
    if (description) {
      const built = view;
      const support = await description.load();
      if (view === built) built.dispatch({ effects: languageSlot.reconfigure(support) });
    }
  }

  onMount(() => {
    void load();
    return () => {
      view?.destroy();
      view = null;
    };
  });

  // What's on disk or in the index moves: a save, a stage, a commit.
  $effect(() => {
    void fileTabs.savedTick;
    void gitStore.snapshot;
    if (live && loaded) void load();
  });

  // Asked for a line while already open.
  $effect(() => {
    void diffJump.target;
    if (loaded) jump();
  });

  // Follows theme and font changes made in Preferences.
  $effect(() => {
    const extension = currentTheme();
    view?.dispatch({ effects: themeSlot.reconfigure(extension) });
  });

  $effect(() => {
    if (active) queueMicrotask(() => view?.focus());
  });

  function openFile() {
    const tileId = workspaceStore.focusedTileId;
    if (tileId) workspaceStore.openFile(tileId, absPath);
  }
</script>

<div class="diff-view">
  <div class="diff-head">
    <span class="diff-path" title={absPath}>{diff.path}</span>
    <span class="diff-sides">{fromLabel} → {toLabel}</span>
    {#if loaded && !error}
      <span class="diff-stats"><span class="plus">+{stats.added}</span> <span class="minus">−{stats.removed}</span></span>
    {/if}
    {#if diff.to === null}
      <button class="diff-button" onclick={openFile}>{t("git.diff.openFile")}</button>
    {/if}
  </div>
  {#if error}
    <div class="diff-message"><p>{error}</p></div>
  {:else if loaded && same}
    <div class="diff-message"><p>{t("git.diff.same")}</p></div>
  {/if}
  <div class="diff-host" class:hidden={!!error || same} bind:this={container}></div>
</div>

<style>
  .diff-view {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg, transparent);
    color: var(--titlebar-fg, #d4d4d4);
  }

  .diff-head {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    min-height: 26px;
    padding: 0 10px;
    border-bottom: 1px solid var(--tabbar-border, #2a2a2a);
    font-size: 11px;
  }

  .diff-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
  }

  .diff-sides {
    flex-shrink: 0;
    opacity: 0.65;
  }

  .diff-stats {
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .plus {
    color: #3fa04a;
  }

  .minus {
    color: #e05a5a;
  }

  .diff-button {
    flex-shrink: 0;
    padding: 2px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .diff-button:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .diff-host {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .diff-host.hidden {
    display: none;
  }

  .diff-host :global(.cm-editor) {
    height: 100%;
  }

  .diff-host :global(.cm-editor.cm-focused) {
    outline: none;
  }

  .diff-message {
    padding: 24px;
    font-size: 13px;
    opacity: 0.75;
  }
</style>
