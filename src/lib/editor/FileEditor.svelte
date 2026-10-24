<script lang="ts">
  import { onMount } from "svelte";
  import { basicSetup } from "codemirror";
  import { Compartment, EditorState, Prec, type Text } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { indentWithTab, redo, redoDepth, undo, undoDepth } from "@codemirror/commands";
  import { openSearchPanel, search, searchPanelOpen } from "@codemirror/search";
  import { readTextFile, writeTextFile } from "$lib/core/explorer-client";
  import { gitBlame, gitFileInfo, type BlameCommit, type FileGitInfo } from "$lib/core/git-client";
  import { appConfigStore } from "$lib/core/app-config.svelte";
  import { ago } from "$lib/agents/agent-spec";
  import { gitStore } from "$lib/git/git-store.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { fileName, type DiffTarget } from "$lib/layout/split-tree";
  import { workspaceStore } from "$lib/layout/workspace-store.svelte";
  import { editorTheme } from "./editor-theme";
  import { languageFor } from "./editor-languages";
  import { EDITOR_PHRASES } from "./editor-phrases";
  import { blameState, blameView, setBlame } from "./editor-blame";
  import { diffJump } from "./diff-jump.svelte";
  import { fileTabs } from "./file-tabs.svelte";
  import EditorToolbar from "./EditorToolbar.svelte";
  import EditorSearch from "./EditorSearch.svelte";

  let { sessionId, filePath, active = false }: { sessionId: string; filePath: string; active?: boolean } = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | null = $state(null);
  let error = $state<string | null>(null);
  let saveError = $state<string | null>(null);
  let languageName = $state<string | null>(null);
  /** The document as last read or saved, to tell whether there's anything
   * unsaved — undoing back to it clears the mark again. */
  let savedDoc: Text | null = null;
  /** Written back the way the file had them, so saving a CRLF file doesn't
   * turn every line into a change. */
  let lineEnding = $state("\n");
  let canUndo = $state(false);
  let canRedo = $state(false);
  /** Find and replace: CodeMirror's search is open (its panel is an empty
   * stand-in for EditorSearch), with the replace field or without. */
  let searchOpen = $state(false);
  let searchReplacing = $state(false);
  /** Bumped to put the keyboard back in the search. */
  let searchNonce = $state(0);
  /** Bumped by every change to the text or the selection. */
  let revision = $state(0);
  /** The file in its repository: null outside one, undefined until known. */
  let info = $state<FileGitInfo | null | undefined>(undefined);
  /** Bumped by every blame asked for, so an older answer is dropped. */
  let blameAsk = 0;

  const themeSlot = new Compartment();
  const languageSlot = new Compartment();
  const phrasesSlot = new Compartment();
  const blameSlot = new Compartment();

  const dirty = $derived(!!fileTabs.dirty[sessionId]);
  const gitOn = $derived(appConfigStore.gitEnabled);
  const blameMode = $derived(gitOn ? appConfigStore.gitEditorBlame : "off");
  /** Changes when the repository moves on in a way that touches this file:
   * a commit, a checkout, a pull, the file staged. A string, so the panel's
   * poll handing over a new snapshot of the same thing changes nothing. */
  const gitKey = $derived.by(() => {
    const same = (path: string) => path.replace(/\\/g, "/").toLowerCase() === filePath.replace(/\\/g, "/").toLowerCase();
    const entry = gitStore.snapshot?.files.find((file) => same(file.absPath)) ?? null;
    return `${gitStore.historyKey}|${JSON.stringify(entry)}`;
  });

  function fontFamily(family: string): string {
    return family.includes(",") ? family : `"${family}", monospace`;
  }

  function currentTheme() {
    return editorTheme(themeStore.current, {
      family: fontFamily(themeStore.effectiveFontFamily),
      size: themeStore.effectiveFontSize,
    });
  }

  function currentPhrases() {
    void i18n.locale;
    return EditorState.phrases.of(Object.fromEntries(Object.entries(EDITOR_PHRASES).map(([phrase, key]) => [phrase, t(key)])));
  }

  function when(seconds: number): string {
    const age = ago(seconds * 1000, Date.now());
    return t(age.key, age.params);
  }

  function blameLabel(commit: BlameCommit): string {
    if (commit.uncommitted) return t("editor.blame.uncommitted");
    const text = `${commit.author}, ${when(commit.time)}`;
    return appConfigStore.gitEditorBlameMessage ? `${text} · ${commit.summary}` : text;
  }

  function blameHint(commit: BlameCommit): string {
    if (commit.uncommitted) return "";
    const date = new Date(commit.time * 1000).toLocaleString(i18n.locale, { dateStyle: "medium", timeStyle: "short" });
    return `${commit.sha.slice(0, 8)} · ${commit.author} · ${date}\n${commit.summary}\n\n${t("editor.blame.openHint")}`;
  }

  /** What a commit changed in this file, at the line asked for. */
  function openCommit(root: string, commit: { sha: string; path: string; previous: string | null; previousPath: string | null }, line = 0) {
    const diff: DiffTarget = {
      root,
      path: commit.path,
      from: commit.previous,
      to: commit.sha,
      ...(commit.previousPath && commit.previousPath !== commit.path ? { fromPath: commit.previousPath } : {}),
    };
    diffJump.to(diff, line);
    workspaceStore.openDiff(diff);
  }

  let blameRoot = "";

  function currentBlameView() {
    if (blameMode === "off") return [];
    void i18n.locale;
    void appConfigStore.gitEditorBlameMessage;
    return blameView({
      mode: blameMode,
      label: blameLabel,
      hint: blameHint,
      edited: t("editor.blame.edited"),
      onpick: (commit, line) => openCommit(blameRoot, commit, line),
    });
  }

  async function save(): Promise<boolean> {
    if (!view) return false;
    const doc = view.state.doc;
    try {
      await writeTextFile(filePath, doc.sliceString(0, doc.length, lineEnding));
      savedDoc = doc;
      saveError = null;
      fileTabs.setDirty(sessionId, false);
      fileTabs.noteSaved();
      void refreshGit();
      return true;
    } catch (err) {
      saveError = String(err);
      return false;
    }
  }

  /** Asks again who wrote each line, of the text as the editor has it. */
  async function refreshBlame() {
    const ask = ++blameAsk;
    if (!view || blameMode === "off") {
      view?.dispatch({ effects: setBlame.of(null) });
      return;
    }
    const doc = view.state.doc;
    const blame = await gitBlame(filePath, doc.sliceString(0, doc.length, lineEnding)).catch(() => null);
    if (ask !== blameAsk || !view) return;
    // Typed into while git looked: what came back is for another text.
    if (view.state.doc !== doc) {
      void refreshBlame();
      return;
    }
    blameRoot = blame?.root ?? "";
    view.dispatch({ effects: setBlame.of(blame) });
  }

  async function refreshInfo() {
    if (!gitOn) {
      info = undefined;
      return;
    }
    const found = await gitFileInfo(filePath).catch(() => null);
    info = found;
  }

  function refreshGit() {
    return Promise.all([refreshInfo(), refreshBlame()]);
  }

  function find(replace: boolean): boolean {
    if (!view) return false;
    searchReplacing = replace;
    if (!searchPanelOpen(view.state)) openSearchPanel(view);
    searchNonce++;
    return true;
  }

  /** CodeMirror's search panel, empty and hidden: opening it (Ctrl+F, F3 with
   * nothing to look for) shows EditorSearch instead, and its matches are
   * highlighted only while a panel is open. */
  const searchStandIn = search({
    top: true,
    createPanel: () => {
      const dom = document.createElement("div");
      dom.className = "cm-search-stand-in";
      searchOpen = true;
      return { dom, top: true, destroy: () => (searchOpen = false) };
    },
  });

  function run(command: (target: EditorView) => boolean) {
    if (!view) return;
    command(view);
    view.focus();
  }

  function viewChanges() {
    if (!info) return;
    workspaceStore.openDiff({ root: info.root, path: info.path, from: info.tracked ? "HEAD" : null, to: null });
  }

  function viewLastCommit() {
    const last = info?.last;
    if (!info || !last) return;
    openCommit(info.root, { sha: last.sha, path: info.path, previous: last.parent, previousPath: null });
  }

  /** The toolbar's switch: off, or back to the way it was shown. */
  let lastBlameMode: "line" | "gutter" = appConfigStore.gitEditorBlame === "gutter" ? "gutter" : "line";
  function toggleBlame() {
    if (appConfigStore.gitEditorBlame === "off") {
      appConfigStore.setGitEditorBlame(lastBlameMode);
    } else {
      lastBlameMode = appConfigStore.gitEditorBlame;
      appConfigStore.setGitEditorBlame("off");
    }
  }

  onMount(() => {
    let disposed = false;
    fileTabs.register(sessionId, save);

    void (async () => {
      let content: string;
      try {
        content = await readTextFile(filePath);
      } catch (err) {
        error = String(err);
        return;
      }
      if (disposed || !container) return;
      lineEnding = content.includes("\r\n") ? "\r\n" : "\n";

      view = new EditorView({
        parent: container,
        state: EditorState.create({
          doc: content,
          extensions: [
            basicSetup,
            searchStandIn,
            Prec.high(
              keymap.of([
                { key: "Mod-f", preventDefault: true, run: () => find(false) },
                { key: "Mod-h", mac: "Mod-Alt-f", preventDefault: true, run: () => find(true) },
              ]),
            ),
            keymap.of([
              indentWithTab,
              {
                key: "Mod-s",
                preventDefault: true,
                run: () => {
                  void save();
                  return true;
                },
              },
            ]),
            themeSlot.of(currentTheme()),
            languageSlot.of([]),
            phrasesSlot.of(currentPhrases()),
            blameState,
            blameSlot.of(currentBlameView()),
            EditorView.updateListener.of((update) => {
              if (update.docChanged && savedDoc) {
                const back = update.state.doc.eq(savedDoc);
                fileTabs.setDirty(sessionId, !back);
                // Undone back to the file: its lines are their commits' again.
                if (back) void refreshBlame();
              }
              if (update.docChanged || update.selectionSet) revision++;
              if (update.docChanged || update.transactions.length) {
                canUndo = undoDepth(update.state) > 0;
                canRedo = redoDepth(update.state) > 0;
              }
            }),
          ],
        }),
      });
      savedDoc = view.state.doc;
      if (active) view.focus();

      // The language, by file name, loads on its own: most never get opened.
      const description = languageFor(fileName(filePath));
      if (description) {
        const support = await description.load();
        if (disposed || !view) return;
        languageName = description.name;
        view.dispatch({ effects: languageSlot.reconfigure(support) });
      }
    })();

    return () => {
      disposed = true;
      fileTabs.unregister(sessionId);
      view?.destroy();
      view = null;
    };
  });

  // Follows theme and font changes made in Preferences.
  $effect(() => {
    const extension = currentTheme();
    view?.dispatch({ effects: themeSlot.reconfigure(extension) });
  });

  // CodeMirror's own texts follow the app's language.
  $effect(() => {
    const extension = currentPhrases();
    view?.dispatch({ effects: phrasesSlot.reconfigure(extension) });
  });

  // The blame's look follows Preferences (and the toolbar's switch).
  $effect(() => {
    const extension = currentBlameView();
    view?.dispatch({ effects: blameSlot.reconfigure(extension) });
  });

  // Asked once the file is open, again when the blame is turned on, and
  // when the repository moves on.
  $effect(() => {
    void blameMode;
    void gitOn;
    void gitKey;
    if (view) void refreshGit();
  });

  // Takes the keyboard when its tab becomes the one in front.
  $effect(() => {
    if (active) queueMicrotask(() => view?.focus());
  });
</script>

<div class="file-editor">
  {#if error}
    <div class="editor-message">
      <p>{error}</p>
      <p class="editor-path">{filePath}</p>
    </div>
  {:else}
    <div class="editor-host" bind:this={container}></div>
    {#if view && searchOpen}
      <EditorSearch {view} bind:replacing={searchReplacing} focusNonce={searchNonce} {revision} />
    {/if}
    {#if view}
      <EditorToolbar
        {dirty}
        {canUndo}
        {canRedo}
        git={gitOn}
        {info}
        blameOn={blameMode !== "off"}
        pinned={appConfigStore.editorToolbarPinned}
        onsave={() => void save()}
        onundo={() => run(undo)}
        onredo={() => run(redo)}
        onfind={() => find(false)}
        onreplace={() => find(true)}
        onblame={toggleBlame}
        onpin={() => appConfigStore.setEditorToolbarPinned(!appConfigStore.editorToolbarPinned)}
        onviewchanges={viewChanges}
        onviewcommit={viewLastCommit}
      />
    {/if}
    <div class="editor-status">
      <span class="editor-path" title={filePath}>{filePath}</span>
      {#if saveError}
        <span class="editor-error">{saveError}</span>
      {:else if dirty}
        <span>{t("editor.unsaved")}</span>
      {/if}
      <span class="editor-language">{languageName ?? t("editor.plainText")}</span>
      <span>{lineEnding === "\r\n" ? "CRLF" : "LF"}</span>
    </div>
  {/if}
</div>

<style>
  .file-editor {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--terminal-bg, transparent);
  }

  .editor-host {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .editor-host :global(.cm-editor) {
    height: 100%;
  }

  .editor-host :global(.cm-editor.cm-focused) {
    outline: none;
  }

  .editor-host :global(.cm-panels:has(> .cm-search-stand-in:only-child)) {
    display: none;
  }

  .editor-status {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 3px 10px;
    border-top: 1px solid var(--tabbar-border, #2a2a2a);
    font-size: 11px;
    color: var(--titlebar-fg, #d4d4d4);
    opacity: 0.75;
  }

  .editor-path {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .editor-error {
    color: #ff6b6b;
  }

  .editor-language {
    text-transform: capitalize;
  }

  .editor-message {
    padding: 24px;
    font-size: 13px;
    color: var(--titlebar-fg, #d4d4d4);
  }

  .editor-message .editor-path {
    opacity: 0.6;
    font-size: 11px;
  }
</style>
