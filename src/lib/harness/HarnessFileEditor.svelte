<script lang="ts">
  import { onMount } from "svelte";
  import { readTextFile, writeTextFile } from "$lib/core/explorer-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { loadHarnessTextEditor } from "./lazy-editors";
  import { harnessStore } from "./harness-store.svelte";

  /** One of the repository's instruction files (CLAUDE.md, AGENTS.md…),
   * edited whole inside the Harness panel. Termo's own section in it is
   * written from the harness, so what's changed there doesn't last: the
   * editor says so when the file has one. */
  let { path }: { path: string } = $props();

  const MARKER = "<!-- termo:harness:start -->";

  let original = $state<string | null>(null);
  let text = $state("");
  let error = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);
  /** The file's line endings, kept on save. */
  let crlf = false;

  const dirty = $derived(original !== null && text !== original);
  const hasSection = $derived(text.includes(MARKER));

  onMount(() => {
    const file = harnessStore.absolute(path);
    if (!file) return;
    readTextFile(file)
      .then((content) => {
        crlf = content.includes("\r\n");
        const normalized = content.replaceAll("\r\n", "\n");
        original = normalized;
        text = normalized;
      })
      .catch((reason) => (error = String(reason)));
  });

  async function save() {
    const file = harnessStore.absolute(path);
    if (!file || saving) return;
    saving = true;
    try {
      await writeTextFile(file, crlf ? text.replaceAll("\n", "\r\n") : text);
      original = text;
      saved = true;
      setTimeout(() => (saved = false), 1500);
      error = null;
      void harnessStore.reload();
    } catch (reason) {
      error = String(reason);
    } finally {
      saving = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      void save();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="file-editor" onkeydown={onKeydown}>
  <div class="file-head">
    <button class="link" onclick={() => harnessStore.editFile(null)}>← {t("harness.review.back")}</button>
    <code class="file-name">{path}</code>
    {#if dirty}
      <span class="dirty" title={t("harness.files.unsaved")}>●</span>
    {/if}
  </div>
  {#if hasSection}
    <p class="note">{t("harness.files.sectionNote")}</p>
  {/if}
  {#if error}
    <div class="error-box"><pre>{error}</pre></div>
  {/if}
  {#if original !== null}
    {#await loadHarnessTextEditor() then { default: HarnessTextEditor }}
      <HarnessTextEditor value={text} label={path} onchange={(next) => (text = next)} tall />
    {/await}
    <div class="file-actions">
      {#if dirty}
        <button class="small" disabled={saving} onclick={() => original !== null && (text = original)}>{t("harness.files.revert")}</button>
      {/if}
      <button class="small primary" disabled={!dirty || saving} onclick={() => void save()}>
        {saved ? t("harness.files.saved") : t("harness.files.save")}
      </button>
    </div>
  {:else if !error}
    <p class="note">{t("harness.loading")}</p>
  {/if}
</div>

<style>
  .file-editor {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px 12px;
  }

  .file-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
    font-weight: 600;
  }

  .dirty {
    color: var(--pane-active-border, #3a6df0);
    font-size: 10px;
  }

  .note {
    margin: 0;
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(128, 128, 128, 0.14);
    font-size: 11px;
  }

  .link {
    padding: 0;
    border: none;
    background: none;
    color: var(--pane-active-border, #3a6df0);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .file-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  .small {
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .small:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .small.primary {
    border-color: var(--pane-active-border, #3a6df0);
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
  }

  .error-box {
    padding: 5px 7px;
    border-radius: 4px;
    background: rgba(224, 90, 90, 0.14);
  }

  .error-box pre {
    margin: 0;
    font-size: 10.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
