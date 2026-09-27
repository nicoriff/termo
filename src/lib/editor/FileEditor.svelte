<script lang="ts">
  import { onMount } from "svelte";
  import { basicSetup } from "codemirror";
  import { Compartment, EditorState, type Text } from "@codemirror/state";
  import { EditorView, keymap } from "@codemirror/view";
  import { indentWithTab } from "@codemirror/commands";
  import { LanguageDescription } from "@codemirror/language";
  import { languages } from "@codemirror/language-data";
  import { readTextFile, writeTextFile } from "$lib/core/explorer-client";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { t } from "$lib/i18n/i18n.svelte";
  import { fileName } from "$lib/layout/split-tree";
  import { editorTheme } from "./editor-theme";
  import { fileTabs } from "./file-tabs.svelte";

  let { sessionId, filePath, active = false }: { sessionId: string; filePath: string; active?: boolean } = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  let error = $state<string | null>(null);
  let saveError = $state<string | null>(null);
  let languageName = $state<string | null>(null);
  /** The document as last read or saved, to tell whether there's anything
   * unsaved — undoing back to it clears the mark again. */
  let savedDoc: Text | null = null;
  /** Written back the way the file had them, so saving a CRLF file doesn't
   * turn every line into a change. */
  let lineEnding = $state("\n");

  const themeSlot = new Compartment();
  const languageSlot = new Compartment();

  function fontFamily(family: string): string {
    return family.includes(",") ? family : `"${family}", monospace`;
  }

  function currentTheme() {
    return editorTheme(themeStore.current, {
      family: fontFamily(themeStore.effectiveFontFamily),
      size: themeStore.effectiveFontSize,
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
      return true;
    } catch (err) {
      saveError = String(err);
      return false;
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
            EditorView.updateListener.of((update) => {
              if (update.docChanged && savedDoc) fileTabs.setDirty(sessionId, !update.state.doc.eq(savedDoc));
            }),
          ],
        }),
      });
      savedDoc = view.state.doc;
      if (active) view.focus();

      // The language, by file name, loads on its own: most never get opened.
      const description = LanguageDescription.matchFilename(languages, fileName(filePath));
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
    <div class="editor-status">
      <span class="editor-path" title={filePath}>{filePath}</span>
      {#if saveError}
        <span class="editor-error">{saveError}</span>
      {:else if fileTabs.dirty[sessionId]}
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
