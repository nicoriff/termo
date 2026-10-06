<script lang="ts">
  import type { RunFailure } from "$lib/core/agents-client";
  import { t } from "$lib/i18n/i18n.svelte";
  import { FAILURE_KEYS, diagnosisText, failureSource } from "./run-diagnosis";

  /** Why a run failed, said precisely: who answered, with what, what it
   * means and what to do — and everything a support ticket needs. */
  let { failure, at }: { failure: RunFailure; at?: number } = $props();

  let copied = $state(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(diagnosisText(failure, t, at));
      copied = true;
      setTimeout(() => (copied = false), 1600);
    } catch {
      // Nothing to do: the text is on screen.
    }
  }
</script>

<div class="failure-card" role="alert">
  <div class="head">
    <span class="mark" aria-hidden="true">✕</span>
    <div class="titles">
      <span class="title">{t(FAILURE_KEYS[failure.kind].title)}</span>
      <span class="source">{failureSource(failure, t)}</span>
    </div>
    <button class="copy" onclick={() => void copy()}>
      {copied ? t("agents.diagnosis.copied") : t("agents.diagnosis.copy")}
    </button>
  </div>

  {#if failure.message}
    <p class="message">{failure.message}</p>
  {/if}
  <p class="hint">{t(FAILURE_KEYS[failure.kind].hint)}</p>

  <dl class="facts">
    {#if failure.model}
      <dt>{t("agents.diagnosis.model")}</dt>
      <dd>{failure.model}</dd>
    {/if}
    {#if failure.requestId}
      <dt>{t("agents.diagnosis.requestId")}</dt>
      <dd>{failure.requestId}</dd>
    {/if}
    {#if failure.exitCode !== null && failure.exitCode !== undefined}
      <dt>{t("agents.diagnosis.exitCode")}</dt>
      <dd>{failure.exitCode}</dd>
    {/if}
  </dl>

  {#if failure.stderr.length > 0}
    <details class="details">
      <summary>{t("agents.diagnosis.details")}</summary>
      <pre>{failure.stderr.join("\n")}</pre>
    </details>
  {/if}
</div>

<style>
  .failure-card {
    margin: 4px 0;
    padding: 10px 12px;
    border: 1px solid color-mix(in srgb, var(--error-fg, #f48771) 55%, transparent);
    border-left: 4px solid var(--error-fg, #f48771);
    border-radius: 6px;
    background: color-mix(in srgb, var(--error-fg, #f48771) 8%, transparent);
    font-family: inherit;
    white-space: normal;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--error-fg, #f48771);
    color: #1e1e1e;
    font-size: 12px;
    font-weight: 700;
  }

  .titles {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .title {
    font-weight: 600;
  }

  .source {
    font-size: 11.5px;
    opacity: 0.8;
  }

  .copy {
    flex-shrink: 0;
    padding: 3px 10px;
    border: 1px solid var(--tabbar-border, #3a3a3a);
    border-radius: 5px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }

  .copy:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .message {
    margin: 8px 0 4px;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 11.5px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .hint {
    margin: 4px 0;
    line-height: 1.45;
  }

  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 10px;
    margin: 6px 0 0;
    font-size: 11.5px;
  }

  .facts dt {
    opacity: 0.6;
  }

  .facts dd {
    margin: 0;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    overflow-wrap: anywhere;
  }

  .details {
    margin-top: 6px;
    font-size: 11.5px;
  }

  .details summary {
    cursor: pointer;
    opacity: 0.75;
  }

  .details pre {
    margin: 6px 0 0;
    padding: 6px 8px;
    border-radius: 4px;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.05));
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
</style>
