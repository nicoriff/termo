<script lang="ts">
  import { t, type MessageKey } from "$lib/i18n/i18n.svelte";
  import type { AgentTarget } from "$lib/core/marketplace-client";

  interface Props {
    /** Every agent, so the ones that can't take this item are shown as
     * unavailable with a reason rather than quietly left out. */
    all?: AgentTarget[];
    supported: AgentTarget[];
    available: AgentTarget[];
    value: AgentTarget;
    /** Why an agent that isn't `supported` can't take it, when there's a
     * better reason to give than "not supported". */
    unsupported?: MessageKey;
    onpick: (target: AgentTarget) => void;
  }

  const {
    all = ["claude", "codex", "gemini", "copilot", "grok"],
    supported,
    available,
    value,
    unsupported = "marketplace.targetUnsupported",
    onpick,
  }: Props = $props();

  const LABEL_KEYS: Record<AgentTarget, MessageKey> = {
    claude: "marketplace.targets.claude",
    codex: "marketplace.targets.codex",
    gemini: "marketplace.targets.gemini",
    copilot: "marketplace.targets.copilot",
    grok: "marketplace.targets.grok",
  };

  function reasonFor(target: AgentTarget): MessageKey | null {
    if (!supported.includes(target)) return unsupported;
    if (!available.includes(target)) return "marketplace.targetCliMissing";
    return null;
  }
</script>

<div class="targets" role="radiogroup" aria-label={t("marketplace.installsInto")}>
  {#each all as target (target)}
    {@const reason = reasonFor(target)}
    <button
      class="target"
      class:on={value === target && reason === null}
      role="radio"
      aria-checked={value === target}
      disabled={reason !== null}
      title={reason ? t(reason) : undefined}
      onclick={() => onpick(target)}
    >
      {t(LABEL_KEYS[target])}
      {#if reason}<span class="why">{t(reason)}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .targets {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .target {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 4px 10px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 999px;
    background: transparent;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 11px;
    cursor: pointer;
  }

  .target.on {
    border-color: var(--pane-active-border, #3a6df0);
    color: var(--pane-active-border, #3a6df0);
  }

  .target:disabled {
    opacity: 0.45;
    cursor: default;
  }

  .why {
    font-size: 9px;
    opacity: 0.8;
  }
</style>
