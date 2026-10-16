<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { t } from "$lib/i18n/i18n.svelte";
  import type { HarnessSpec } from "$lib/core/harness-client";
  import { TOOL_DISPLAY_NAMES } from "$lib/ai-mode/tool-names";
  import SettingDropdown from "$lib/preferences/SettingDropdown.svelte";
  import HarnessInfo from "./HarnessInfo.svelte";
  import {
    ROW_KINDS,
    addRow,
    isPathKind,
    relativeToRoot,
    removeRow,
    rowProblem,
    rowSupport,
    ruleRows,
    type RowKind,
    type RowSide,
    type Support,
  } from "./harness-model";
  import { harnessStore } from "./harness-store.svelte";

  /** One of the Harness panel's two lists: what the agents do ("do"), or
   * what they don't ("dont"). Each rule is one plain line, with what it
   * means and which agents hold it behind its info button. */
  let {
    side,
    spec,
    root,
    footer,
  }: {
    side: RowSide;
    spec: HarnessSpec;
    root: string;
    /** Below the list: quick ways to add (packs, suggested checks). */
    footer?: Snippet;
  } = $props();

  const KIND_TEXT = {
    check: "harness.rows.text.check",
    allow: "harness.rows.text.allow",
    noRead: "harness.rows.text.noRead",
    noEdit: "harness.rows.text.noEdit",
    deny: "harness.rows.text.deny",
    ask: "harness.rows.text.ask",
  } as const;
  const KIND_NAME = {
    check: "harness.rows.kinds.check",
    allow: "harness.rows.kinds.allow",
    instruction: "harness.rows.kinds.instruction",
    noRead: "harness.rows.kinds.noRead",
    noEdit: "harness.rows.kinds.noEdit",
    deny: "harness.rows.kinds.deny",
    ask: "harness.rows.kinds.ask",
  } as const;
  const KIND_ABOUT = {
    check: "harness.rows.about.check",
    allow: "harness.rows.about.allow",
    instruction: "harness.rows.about.instruction",
    noRead: "harness.rows.about.noRead",
    noEdit: "harness.rows.about.noEdit",
    deny: "harness.rows.about.deny",
    ask: "harness.rows.about.ask",
  } as const;
  const PLACEHOLDER = {
    check: "harness.rows.placeholders.check",
    allow: "harness.rows.placeholders.allow",
    instruction: "harness.rows.placeholders.instruction",
    noRead: "harness.rows.placeholders.path",
    noEdit: "harness.rows.placeholders.path",
    deny: "harness.rows.placeholders.deny",
    ask: "harness.rows.placeholders.ask",
  } as const;
  const SUPPORT_LABEL = {
    enforced: "harness.support.enforced",
    partial: "harness.support.partial",
    text: "harness.support.text",
    none: "harness.support.none",
  } as const;
  const SUPPORT_MARK: Record<Support, string> = { enforced: "✓", partial: "◐", text: "Aa", none: "–" };

  const rows = $derived(ruleRows(spec)[side]);
  const kinds = $derived(ROW_KINDS[side]);

  let adding = $state(false);
  // The first kind of its side to start with; picked from there on.
  let kind = $state<RowKind>(untrack(() => ROW_KINDS[side][0]));
  let subject = $state("");
  let note = $state<string | null>(null);
  const problem = $derived(subject.trim() ? rowProblem(spec, kind, subject) : null);
  let input: HTMLInputElement | undefined = $state();

  /** A rule's sentence, its subject apart so it can be set as code. */
  function sentence(rowKind: Exclude<RowKind, "instruction">): [string, string] {
    const marker = "\u0000";
    const [before, after = ""] = t(KIND_TEXT[rowKind], { subject: marker }).split(marker);
    return [before, after];
  }

  function startAdding() {
    adding = true;
    note = null;
    queueMicrotask(() => input?.focus());
  }

  function cancel() {
    adding = false;
    subject = "";
    note = null;
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!subject.trim() || problem) return;
    const [addKind, text] = [kind, subject];
    harnessStore.edit((draft) => addRow(draft, addKind, text));
    subject = "";
    note = null;
    input?.focus();
  }

  async function browse() {
    const picked = await openDialog({ defaultPath: root, multiple: false, directory: false });
    if (typeof picked !== "string") return;
    const relative = relativeToRoot(root, picked);
    if (relative === null) {
      note = t("harness.paths.outside");
      return;
    }
    note = null;
    subject = relative;
  }

  function onFormKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.stopPropagation();
      cancel();
    }
  }
</script>

<section class="rule-section {side}">
  <header class="rule-head">
    <span class="rule-icon" aria-hidden="true">
      {#if side === "do"}
        <svg viewBox="0 0 20 20">
          <circle cx="10" cy="10" r="8.2" fill="currentColor" opacity="0.18" />
          <circle cx="10" cy="10" r="8.2" fill="none" stroke="currentColor" stroke-width="1.5" />
          <path d="M6.2 10.3l2.6 2.6 5-5.3" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      {:else}
        <svg viewBox="0 0 20 20">
          <circle cx="10" cy="10" r="8.2" fill="currentColor" opacity="0.18" />
          <circle cx="10" cy="10" r="8.2" fill="none" stroke="currentColor" stroke-width="1.5" />
          <path d="M4.4 15.6L15.6 4.4" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
        </svg>
      {/if}
    </span>
    <div class="rule-titles">
      <h3>{t(side === "do" ? "harness.rows.do.title" : "harness.rows.dont.title")}</h3>
      <p>{t(side === "do" ? "harness.rows.do.hint" : "harness.rows.dont.hint")}</p>
    </div>
    {#if rows.length > 0}
      <span class="rule-count">{rows.length}</span>
    {/if}
  </header>

  {#if rows.length === 0 && !adding}
    <p class="empty">{t(side === "do" ? "harness.rows.do.empty" : "harness.rows.dont.empty")}</p>
  {:else}
    <ul class="rows">
      {#each rows as row (`${row.kind}:${row.index}:${row.subject}`)}
        <li class="row {row.kind}">
          <span class="bullet" aria-hidden="true"></span>
          <span class="row-text">
            {#if row.kind === "instruction"}
              {row.subject}
            {:else}
              {@const [before, after] = sentence(row.kind)}
              {before}<code>{row.subject}</code>{after}
            {/if}
          </span>
          <HarnessInfo label={t("harness.rows.details")}>
            <p class="tip-kind">{t(KIND_NAME[row.kind])}</p>
            <p class="tip-about">{t(KIND_ABOUT[row.kind])}</p>
            {#if spec.agents.length > 0}
              <p class="tip-how">{t("harness.rows.how")}</p>
              <ul class="tip-agents">
                {#each rowSupport(spec, row.kind) as { agent, support } (agent)}
                  <li>
                    <span>{TOOL_DISPLAY_NAMES[agent]}</span>
                    <span class="tip-support {support}"><span aria-hidden="true">{SUPPORT_MARK[support]}</span> {t(SUPPORT_LABEL[support])}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </HarnessInfo>
          <button
            class="row-remove"
            title={t("harness.remove")}
            aria-label={t("harness.remove")}
            onclick={() => harnessStore.edit((draft) => removeRow(draft, row))}
          >
            <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" stroke-width="1.1" /></svg>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if adding}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <form class="add-form" onsubmit={submit} onkeydown={onFormKeydown}>
      <SettingDropdown
        label={t("harness.rows.kind")}
        width="100%"
        options={kinds.map((option) => ({ value: option, label: t(KIND_NAME[option]) }))}
        value={kind}
        onpick={(next) => {
          kind = next as RowKind;
          note = null;
        }}
      />
      <div class="add-line">
        <input
          bind:this={input}
          type="text"
          class:mono={kind !== "instruction"}
          placeholder={t(PLACEHOLDER[kind])}
          aria-label={t(PLACEHOLDER[kind])}
          bind:value={subject}
          oninput={() => (note = null)}
        />
        {#if isPathKind(kind)}
          <button type="button" class="small" title={t("harness.paths.browse")} aria-label={t("harness.paths.browse")} onclick={() => void browse()}>…</button>
        {/if}
      </div>
      {#if note || problem}
        <p class="field-error">{note ?? (problem ? t(problem) : "")}</p>
      {/if}
      <div class="add-actions">
        <button type="button" class="small" onclick={cancel}>{t("harness.rows.done")}</button>
        <button type="submit" class="small primary" disabled={!subject.trim() || !!problem}>{t("harness.add")}</button>
      </div>
    </form>
  {:else}
    <button class="add-button" onclick={startAdding}>
      <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M6 2v8M2 6h8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" /></svg>
      {t("harness.rows.add")}
    </button>
  {/if}

  {#if footer}
    {@render footer()}
  {/if}
</section>

<style>
  .rule-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 10px;
    padding: 10px 10px 8px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 8px;
    --accent: #4caf72;
  }

  .rule-section.dont {
    --accent: #e05a5a;
  }

  .rule-head {
    display: flex;
    align-items: flex-start;
    gap: 9px;
  }

  .rule-icon {
    flex-shrink: 0;
    color: var(--accent);
  }

  .rule-icon svg {
    display: block;
    width: 26px;
    height: 26px;
  }

  .rule-titles {
    flex: 1;
    min-width: 0;
  }

  h3 {
    margin: 1px 0 0;
    font-size: 13.5px;
    font-weight: 700;
  }

  .rule-titles p {
    margin: 2px 0 0;
    font-size: 11px;
    opacity: 0.6;
  }

  .rule-count {
    min-width: 18px;
    padding: 0 5px;
    box-sizing: border-box;
    border-radius: 9px;
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    color: var(--accent);
    font-size: 10.5px;
    font-weight: 700;
    line-height: 17px;
    text-align: center;
  }

  .empty {
    margin: 2px 0 0 35px;
    font-size: 11px;
    opacity: 0.5;
  }

  .rows {
    display: flex;
    flex-direction: column;
    margin: 2px 0 0;
    padding: 0;
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 7px;
    min-height: 26px;
    padding: 2px 2px 2px 8px;
    border-radius: 5px;
  }

  .row:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
  }

  .bullet {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--accent);
    opacity: 0.8;
  }

  /* Asking first is between doing and not: amber. */
  .row.ask .bullet {
    background: #d7a72a;
  }

  .row.instruction .bullet {
    background: transparent;
    border: 1.5px solid var(--accent);
    box-sizing: border-box;
  }

  .row-text {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .row-text code {
    padding: 0 3px;
    border-radius: 3px;
    background: rgba(128, 128, 128, 0.18);
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11px;
  }

  .row-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
    padding: 0;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: inherit;
    opacity: 0;
    cursor: pointer;
  }

  .row:hover .row-remove,
  .row-remove:focus-visible {
    opacity: 0.6;
  }

  .row-remove:hover {
    opacity: 1 !important;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .row-remove svg {
    width: 9px;
    height: 9px;
  }

  .add-button {
    display: flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    margin-left: 4px;
    padding: 3px 6px;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--accent);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .add-button:hover {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .add-button svg {
    width: 11px;
    height: 11px;
  }

  .add-form {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 7px;
    border-radius: 6px;
    background: rgba(128, 128, 128, 0.1);
  }

  .add-line {
    display: flex;
    gap: 4px;
  }

  .add-line input {
    flex: 1;
    min-width: 0;
  }

  .add-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }

  input[type="text"] {
    box-sizing: border-box;
    padding: 4px 6px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 4px;
    background: var(--terminal-bg, rgba(0, 0, 0, 0.2));
    color: inherit;
    font: inherit;
    font-size: 11.5px;
  }

  input.mono {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
  }

  input[type="text"]:focus {
    outline: none;
    border-color: var(--pane-active-border, #3a6df0);
  }

  .field-error {
    margin: 0;
    font-size: 11px;
    color: #e05a5a;
  }

  .small {
    flex-shrink: 0;
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

  .tip-kind {
    margin: 0 0 3px;
    font-weight: 700;
  }

  .tip-about {
    margin: 0;
    opacity: 0.85;
  }

  .tip-how {
    margin: 8px 0 3px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0.6;
  }

  .tip-agents {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .tip-agents li {
    display: flex;
    justify-content: space-between;
    gap: 10px;
  }

  .tip-support {
    font-weight: 600;
  }

  .tip-support.enforced {
    color: #4caf72;
  }

  .tip-support.partial {
    color: #d7a72a;
  }

  .tip-support.text {
    color: var(--pane-active-border, #3a6df0);
  }

  .tip-support.none {
    opacity: 0.5;
  }
</style>
