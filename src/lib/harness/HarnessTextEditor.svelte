<script lang="ts">
  import { onMount } from "svelte";
  import { basicSetup } from "codemirror";
  import { indentWithTab } from "@codemirror/commands";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView, keymap, placeholder as placeholderText } from "@codemirror/view";
  import { LanguageDescription } from "@codemirror/language";
  import { languages } from "@codemirror/language-data";
  import { editorTheme } from "$lib/editor/editor-theme";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  /** A small Markdown editor inside the Harness panel: what the agents are
   * told. `onchange` gets every edit; a new `value` from outside (a discard,
   * another repository) replaces the text. */
  let {
    value,
    onchange,
    placeholder = "",
    label,
  }: { value: string; onchange: (value: string) => void; placeholder?: string; label: string } = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  const themeSlot = new Compartment();
  const placeholderSlot = new Compartment();
  const languageSlot = new Compartment();

  function fontFamily(family: string): string {
    return family.includes(",") ? family : `"${family}", monospace`;
  }

  function currentTheme() {
    return editorTheme(themeStore.current, {
      family: fontFamily(themeStore.effectiveFontFamily),
      size: Math.max(10, themeStore.effectiveFontSize - 1),
    });
  }

  onMount(() => {
    if (!container) return;
    view = new EditorView({
      parent: container,
      state: EditorState.create({
        doc: value,
        extensions: [
          basicSetup,
          keymap.of([indentWithTab]),
          EditorView.lineWrapping,
          EditorView.contentAttributes.of({ "aria-label": label }),
          themeSlot.of(currentTheme()),
          placeholderSlot.of(placeholderText(placeholder)),
          languageSlot.of([]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) onchange(update.state.doc.toString());
          }),
        ],
      }),
    });
    const built = view;
    const markdown = LanguageDescription.matchFilename(languages, "AGENTS.md");
    void markdown?.load().then((support) => {
      if (view === built) built.dispatch({ effects: languageSlot.reconfigure(support) });
    });
    return () => {
      view?.destroy();
      view = null;
    };
  });

  // Replaced from outside: only when it isn't what's already there, so
  // typing doesn't move the cursor.
  $effect(() => {
    const next = value;
    if (view && view.state.doc.toString() !== next) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
    }
  });

  // Follows theme and font changes made in Preferences.
  $effect(() => {
    const extension = currentTheme();
    view?.dispatch({ effects: themeSlot.reconfigure(extension) });
  });

  $effect(() => {
    const text = placeholder;
    view?.dispatch({ effects: placeholderSlot.reconfigure(placeholderText(text)) });
  });
</script>

<div class="harness-text" bind:this={container}></div>

<style>
  .harness-text {
    min-height: 120px;
    max-height: 420px;
    overflow: auto;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    resize: vertical;
  }

  .harness-text:focus-within {
    border-color: var(--pane-active-border, #3a6df0);
  }

  .harness-text :global(.cm-editor) {
    min-height: 120px;
  }

  .harness-text :global(.cm-editor.cm-focused) {
    outline: none;
  }
</style>
