<script lang="ts">
  import { onMount } from "svelte";
  import { basicSetup } from "codemirror";
  import { Compartment, EditorState } from "@codemirror/state";
  import { EditorView } from "@codemirror/view";
  import { unifiedMergeView } from "@codemirror/merge";
  import { editorTheme } from "$lib/editor/editor-theme";
  import { languageFor } from "$lib/editor/editor-languages";
  import { t } from "$lib/i18n/i18n.svelte";
  import { fileName } from "$lib/layout/split-tree";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  /** One file's change on the Harness panel's review: the file as it would
   * be, with what goes shown in place, read only. Like DiffView, but from
   * two texts instead of from git. */
  let { path, before, after }: { path: string; before: string | null; after: string | null } = $props();

  let container: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  const themeSlot = new Compartment();
  const languageSlot = new Compartment();

  function fontFamily(family: string): string {
    return family.includes(",") ? family : `"${family}", monospace`;
  }

  function currentTheme() {
    const c = themeStore.current.colors;
    return [
      editorTheme(themeStore.current, {
        family: fontFamily(themeStore.effectiveFontFamily),
        size: Math.max(10, themeStore.effectiveFontSize - 2),
      }),
      EditorView.theme({
        ".cm-changedLine": { backgroundColor: `${c.green}22` },
        ".cm-changedText": { background: `${c.green}44` },
        ".cm-deletedChunk": { backgroundColor: `${c.red}22` },
        ".cm-deletedChunk .cm-deletedText, .cm-deletedText": { background: `${c.red}44` },
        ".cm-changeGutter": { width: "3px" },
        ".cm-changedLineGutter": { background: c.green },
        ".cm-deletedLineGutter": { background: c.red },
        ".cm-collapsedLines": { color: c.brightBlack, background: "transparent" },
      }),
    ];
  }

  onMount(() => {
    if (!container) return;
    // CRLF and LF read the same: only what the lines say counts.
    const [a, b] = [(before ?? "").replaceAll("\r\n", "\n"), (after ?? "").replaceAll("\r\n", "\n")];
    view = new EditorView({
      parent: container,
      state: EditorState.create({
        doc: b,
        extensions: [
          basicSetup,
          EditorState.readOnly.of(true),
          EditorView.editable.of(false),
          EditorView.lineWrapping,
          EditorState.phrases.of({ "$ unchanged lines": t("git.diff.unchanged", { n: "$" }) }),
          themeSlot.of(currentTheme()),
          languageSlot.of([]),
          unifiedMergeView({ original: a, mergeControls: false, gutter: true, collapseUnchanged: { margin: 2, minSize: 5 } }),
        ],
      }),
    });
    const built = view;
    const description = languageFor(fileName(path));
    if (description) {
      void description.load().then((support) => {
        if (view === built) built.dispatch({ effects: languageSlot.reconfigure(support) });
      });
    }
    return () => {
      view?.destroy();
      view = null;
    };
  });

  $effect(() => {
    const extension = currentTheme();
    view?.dispatch({ effects: themeSlot.reconfigure(extension) });
  });
</script>

<div class="harness-diff" bind:this={container}></div>

<style>
  .harness-diff {
    max-height: 360px;
    overflow: auto;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
  }

  .harness-diff :global(.cm-editor) {
    height: auto;
  }
</style>
