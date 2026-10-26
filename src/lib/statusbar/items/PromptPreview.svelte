<script lang="ts">
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onMount } from "svelte";
  import { promptPreview } from "$lib/core/prompt-preview-client";
  import type { PromptEngine } from "$lib/core/types";
  import { t } from "$lib/i18n/i18n.svelte";
  import { withMonospaceFallback } from "$lib/terminal/TerminalInstance";
  import { toXtermTheme } from "$lib/theme/theme-loader";
  import { themeStore } from "$lib/theme/theme-store.svelte";
  import { activeConsole } from "../active.svelte";

  /** How a prompt theme looks: the tool's own output for the active
   * console's folder, drawn in a small terminal with the user's colors and
   * font, and a command typed after it. */
  let { tool, theme }: { tool: Exclude<PromptEngine, "none">; theme: string | null } = $props();

  const SAMPLE_COMMAND = "git status";

  let host: HTMLDivElement;
  let term: Terminal | null = null;
  let cols = $state(0);
  let phase = $state<"loading" | "ready" | "failed">("loading");

  /** Each theme's output, while the menu is open: going back over one shows
   * it at once. */
  const cache = new Map<string, Promise<string>>();

  onMount(() => {
    const current = themeStore.current;
    const terminal = new Terminal({
      theme: toXtermTheme(current),
      fontFamily: withMonospaceFallback(themeStore.effectiveFontFamily),
      fontSize: Math.min(themeStore.effectiveFontSize, 13),
      lineHeight: current.font.lineHeight,
      cursorBlink: false,
      disableStdin: true,
      scrollback: 0,
      rows: 2,
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(host);
    cols = fit.proposeDimensions()?.cols ?? 60;
    terminal.resize(cols, 2);
    term = terminal;
    return () => {
      terminal.dispose();
      term = null;
    };
  });

  $effect(() => {
    const key = `${tool}:${theme ?? ""}:${cols}`;
    if (!term || cols === 0) return;
    let stale = false;
    phase = "loading";
    let request = cache.get(key);
    if (!request) {
      request = promptPreview(tool, theme, activeConsole.folder, cols);
      cache.set(key, request);
      request.catch(() => cache.delete(key));
    }
    request
      .then((ansi) => {
        if (stale || !term) return;
        show(term, ansi);
        phase = "ready";
      })
      .catch(() => {
        if (!stale) phase = "failed";
      });
    return () => {
      stale = true;
    };
  });

  /** The prompt as a console would have it: without the blank line some
   * themes start with, and with a command after it. */
  function show(terminal: Terminal, ansi: string) {
    const lines = ansi.replace(/^(\r?\n)+/, "").replace(/\r?\n/g, "\r\n");
    const rows = lines.split("\r\n").length;
    terminal.resize(cols, Math.max(1, Math.min(rows, 6)));
    terminal.reset();
    terminal.write("\x1b[?25l" + lines + SAMPLE_COMMAND);
  }
</script>

<div class="preview" class:dim={phase !== "ready"}>
  <div class="terminal" bind:this={host}></div>
  {#if phase !== "ready"}
    <span class="status">{t(phase === "loading" ? "statusBar.prompt.previewLoading" : "statusBar.prompt.previewFailed")}</span>
  {/if}
</div>

<style>
  .preview {
    position: relative;
    padding: 8px 10px;
    border-radius: 5px;
    background: var(--terminal-bg, #1e1e1e);
  }

  .terminal {
    pointer-events: none;
    transition: opacity 0.1s;
  }

  .preview.dim .terminal {
    opacity: 0.35;
  }

  /* xterm's own viewport scrollbar, of no use for a couple of lines. */
  .terminal :global(.xterm-viewport) {
    overflow: hidden !important;
  }

  .status {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    opacity: 0.75;
  }
</style>
