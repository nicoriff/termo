<script lang="ts">
  import { onDestroy } from "svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import type { FileGitInfo } from "$lib/core/git-client";
  import type { GitMark } from "$lib/core/explorer-client";
  import { isMac } from "$lib/core/platform";
  import { ago } from "$lib/agents/agent-spec";
  import { markColor } from "$lib/explorer/git-marks";
  import { i18n, t } from "$lib/i18n/i18n.svelte";
  import { themeStore } from "$lib/theme/theme-store.svelte";

  /** The file editor's tools: a round button floating at the top middle of
   * the file that opens out to both sides while the mouse is on it (or the
   * keyboard is in it), and stays open when pinned. Changing the file opens
   * it for a moment, saving a click away. */
  let {
    dirty,
    canUndo,
    canRedo,
    git,
    info,
    blameOn,
    pinned,
    onsave,
    onundo,
    onredo,
    onfind,
    onreplace,
    onblame,
    onpin,
    onviewchanges,
    onviewcommit,
  }: {
    dirty: boolean;
    canUndo: boolean;
    canRedo: boolean;
    /** Whether git is on at all: off, nothing of it shows. */
    git: boolean;
    /** The file in its repository; null outside one, undefined until known. */
    info: FileGitInfo | null | undefined;
    blameOn: boolean;
    pinned: boolean;
    onsave: () => void;
    onundo: () => void;
    onredo: () => void;
    onfind: () => void;
    onreplace: () => void;
    onblame: () => void;
    onpin: () => void;
    onviewchanges: () => void;
    onviewcommit: () => void;
  } = $props();

  /** How long it stays open after the mouse leaves: crossing a gap on the
   * way to a button doesn't fold it. */
  const LEAVE_MS = 350;
  /** How long changing the file shows it. */
  const NUDGE_MS = 2500;

  let hovering = $state(false);
  let focused = $state(false);
  let nudged = $state(false);
  let gitOpen = $state(false);
  let copied = $state(false);
  let leaveTimer: ReturnType<typeof setTimeout> | undefined;
  let nudgeTimer: ReturnType<typeof setTimeout> | undefined;
  let wasDirty = false;

  const open = $derived(pinned || hovering || focused || nudged || gitOpen);
  const showGit = $derived(git && !!info);

  const mod = isMac ? "⌘" : "Ctrl+";
  const keys = {
    save: `${mod}S`,
    undo: `${mod}Z`,
    redo: isMac ? "⌘⇧Z" : "Ctrl+Y",
    find: `${mod}F`,
    replace: isMac ? "⌘⌥F" : "Ctrl+H",
  };

  /** The one mark that says most about the file, for its dot. */
  const mark = $derived.by((): GitMark | null => {
    if (!info || info.changesUnavailable) return null;
    if (info.unstaged === "conflict") return "conflict";
    if (!info.tracked) return "untracked";
    return info.unstaged ?? info.staged ?? null;
  });

  const stateText = $derived.by(() => {
    if (!info) return "";
    if (info.changesUnavailable) return t("editor.git.changesUnavailable");
    if (mark === "conflict") return t("editor.git.conflict");
    if (!info.tracked) return t("editor.git.untracked");
    if (info.staged && info.unstaged) return t("editor.git.stagedAndModified");
    if (info.staged) return t("editor.git.staged");
    if (info.unstaged) return t("editor.git.modified");
    return t("editor.git.clean");
  });

  const changed = $derived(!!info && !info.changesUnavailable && (info.added ?? 0) + (info.removed ?? 0) > 0);

  function when(seconds: number): string {
    const age = ago(seconds * 1000, Date.now());
    return t(age.key, age.params);
  }

  function exactly(seconds: number): string {
    return new Date(seconds * 1000).toLocaleString(i18n.locale, { dateStyle: "medium", timeStyle: "short" });
  }

  async function copySha(sha: string) {
    await writeText(sha).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1200);
  }

  function enter() {
    clearTimeout(leaveTimer);
    hovering = true;
  }

  function leave() {
    clearTimeout(leaveTimer);
    leaveTimer = setTimeout(() => {
      hovering = false;
      gitOpen = false;
    }, LEAVE_MS);
  }

  function onFocusOut(event: FocusEvent) {
    const into = event.relatedTarget as Node | null;
    if (!into || !(event.currentTarget as HTMLElement).contains(into)) focused = false;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    if (gitOpen) {
      gitOpen = false;
      return;
    }
    (document.activeElement as HTMLElement | null)?.blur();
    hovering = false;
  }

  // Changing the file opens it for a moment, unless the mouse is already on it.
  $effect(() => {
    if (dirty && !wasDirty) {
      nudged = true;
      clearTimeout(nudgeTimer);
      nudgeTimer = setTimeout(() => (nudged = false), NUDGE_MS);
    }
    wasDirty = dirty;
  });

  onDestroy(() => {
    clearTimeout(leaveTimer);
    clearTimeout(nudgeTimer);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="editor-toolbar"
  class:open
  style="--added:{themeStore.current.colors.green};--removed:{themeStore.current.colors.red}"
  onmouseenter={enter}
  onmouseleave={leave}
  onfocusin={(event) => (focused = (event.target as HTMLElement).matches(":focus-visible"))}
  onfocusout={onFocusOut}
  onkeydown={onKeydown}
>
  <div class="dock" role="toolbar" aria-label={t("editor.toolbar.label")}>
    <div class="wing left" inert={!open}>
      <div class="wing-inner">
        <button class="tool" style="--i:4" disabled={!dirty} title={`${t("editor.toolbar.save")} (${keys.save})`} aria-label={t("editor.toolbar.save")} onclick={onsave}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M3 2.5h8l2.5 2.5v8.5h-11zM5 2.5v3.5h5.5V2.5M5 13.5V9.5h6v4" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
          </svg>
        </button>
        <button class="tool" style="--i:3" disabled={!canUndo} title={`${t("editor.toolbar.undo")} (${keys.undo})`} aria-label={t("editor.toolbar.undo")} onclick={onundo}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M5.5 3 2.5 6l3 3M2.5 6h7a4 4 0 0 1 0 8H7" fill="none" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
        <button class="tool" style="--i:2" disabled={!canRedo} title={`${t("editor.toolbar.redo")} (${keys.redo})`} aria-label={t("editor.toolbar.redo")} onclick={onredo}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M10.5 3l3 3-3 3M13.5 6h-7a4 4 0 0 0 0 8H9" fill="none" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
        <span class="sep" aria-hidden="true"></span>
        <button class="tool" style="--i:1" title={`${t("editor.toolbar.find")} (${keys.find})`} aria-label={t("editor.toolbar.find")} onclick={onfind}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <circle cx="7" cy="7" r="4.25" fill="none" stroke="currentColor" stroke-width="1.2" />
            <path d="M10.2 10.2 13.5 13.5" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
        <button class="tool" style="--i:0" title={`${t("editor.toolbar.replace")} (${keys.replace})`} aria-label={t("editor.toolbar.replace")} onclick={onreplace}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M2.5 5h8.5M8.5 2.5 11 5 8.5 7.5M13.5 11H5M7.5 8.5 5 11l2.5 2.5" fill="none" stroke="currentColor" stroke-width="1.2" />
          </svg>
        </button>
      </div>
    </div>

    <button class="hub" class:dirty title={t("editor.toolbar.open")} aria-label={t("editor.toolbar.label")} aria-expanded={open} onclick={enter}>
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M10.5 2.5 13.5 5.5 6 13H3v-3zM9 4l3 3" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" />
      </svg>
      {#if dirty}<span class="dirty-dot" aria-hidden="true"></span>{/if}
    </button>

    <div class="wing right" inert={!open}>
      <div class="wing-inner">
        {#if showGit && info}
          <button
            class="tool chip"
            style="--i:0"
            class:active={gitOpen}
            title={t("editor.toolbar.gitInfo")}
            aria-label={t("editor.toolbar.gitInfo")}
            aria-expanded={gitOpen}
            onclick={() => (gitOpen = !gitOpen)}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <circle cx="4.5" cy="3.5" r="1.5" fill="none" stroke="currentColor" stroke-width="1.2" />
              <circle cx="4.5" cy="12.5" r="1.5" fill="none" stroke="currentColor" stroke-width="1.2" />
              <circle cx="11.5" cy="5.5" r="1.5" fill="none" stroke="currentColor" stroke-width="1.2" />
              <path d="M4.5 5v6M11.5 7c0 3-7 2-7 4" fill="none" stroke="currentColor" stroke-width="1.2" />
            </svg>
            <span class="chip-branch">{info.branch ?? t("editor.git.detached")}</span>
            {#if mark}<span class="mark-dot" style="background:{markColor(mark, themeStore.current.colors)}" aria-hidden="true"></span>{/if}
            {#if changed}
              <span class="count added">+{info.added ?? 0}</span>
              <span class="count removed">−{info.removed ?? 0}</span>
            {/if}
          </button>
          <button
            class="tool"
            style="--i:1"
            class:on={blameOn}
            title={blameOn ? t("editor.toolbar.blameOff") : t("editor.toolbar.blameOn")}
            aria-label={blameOn ? t("editor.toolbar.blameOff") : t("editor.toolbar.blameOn")}
            aria-pressed={blameOn}
            onclick={onblame}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <circle cx="5" cy="5" r="2.25" fill="none" stroke="currentColor" stroke-width="1.2" />
              <path d="M1.5 12.5c0-2 1.6-3.5 3.5-3.5s3.5 1.5 3.5 3.5M10 5h4.5M10 8h4.5M11 11h3.5" fill="none" stroke="currentColor" stroke-width="1.2" />
            </svg>
          </button>
          <span class="sep" aria-hidden="true"></span>
        {/if}
        <button
          class="tool"
          style="--i:{showGit ? 2 : 0}"
          class:on={pinned}
          title={pinned ? t("editor.toolbar.unpin") : t("editor.toolbar.pin")}
          aria-label={pinned ? t("editor.toolbar.unpin") : t("editor.toolbar.pin")}
          aria-pressed={pinned}
          onclick={onpin}
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            {#if pinned}
              <path d="M6 2.5h4l-.5 4 2.5 2.5H4L6.5 6.5zM8 9v4.5" fill="currentColor" fill-opacity="0.35" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
            {:else}
              <path d="M6 2.5h4l-.5 4 2.5 2.5H4L6.5 6.5zM8 9v4.5" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
            {/if}
          </svg>
        </button>
      </div>
    </div>
  </div>

  {#if gitOpen && showGit && info}
    <div class="git-card">
      <dl>
        <dt>{t("editor.git.branch")}</dt>
        <dd>{info.branch ?? t("editor.git.detached")}</dd>
        <dt>{t("editor.git.state")}</dt>
        <dd class="state">
          {#if mark}<span class="mark-dot" style="background:{markColor(mark, themeStore.current.colors)}" aria-hidden="true"></span>{/if}
          {stateText}
        </dd>
        {#if changed}
          <dt>{t("editor.git.lines")}</dt>
          <dd><span class="count added">+{info.added ?? 0}</span> <span class="count removed">−{info.removed ?? 0}</span></dd>
        {/if}
        <dt>{t("editor.git.lastCommit")}</dt>
        <dd>
          {#if info.last}
            <span class="subject">{info.last.subject}</span>
            <span class="meta" title={exactly(info.last.time)}>{info.last.author} · {when(info.last.time)}</span>
            <button class="sha" title={t("editor.git.copySha")} onclick={() => copySha(info.last!.sha)}>
              {copied ? t("git.copied") : info.last.sha.slice(0, 8)}
            </button>
          {:else}
            <span class="meta">{t("editor.git.noCommits")}</span>
          {/if}
        </dd>
      </dl>
      <div class="card-actions">
        {#if info.tracked ? changed || !!info.unstaged || !!info.staged : true}
          <button class="action" onclick={onviewchanges}>{t("editor.git.viewChanges")}</button>
        {/if}
        {#if info.last}
          <button class="action" onclick={onviewcommit}>{t("editor.git.viewCommit")}</button>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* Floats at the top middle of the file; only its own parts take the mouse,
     so the text around it stays clickable. */
  .editor-toolbar {
    --ease: cubic-bezier(0.22, 1, 0.36, 1);
    position: absolute;
    top: 10px;
    left: 50%;
    z-index: 5;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    color: var(--titlebar-fg, #d4d4d4);
    font-size: 11.5px;
  }

  /* The round button sits in the middle column; the two sides take equal
     columns, as wide as the wider one, so the button stays put however much
     each side holds. Each side draws its own half of the bar, hugging its
     buttons and tucked under the round one, so the bar has no empty end. */
  .dock {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
  }

  /* Each side grows from nothing to its content: the columns animate, so
     the bar opens out from behind the round button. */
  .wing {
    display: grid;
    grid-template-columns: 0fr;
    transition: grid-template-columns 300ms var(--ease);
  }

  .open .wing {
    grid-template-columns: 1fr;
  }

  .wing.left {
    justify-self: end;
    margin-right: -17px;
  }

  .wing.right {
    justify-self: start;
    margin-left: -17px;
  }

  .wing-inner {
    min-width: 0;
    overflow: hidden;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 1px;
    height: 36px;
    border: 1px solid transparent;
    background: transparent;
    transition:
      padding 300ms var(--ease),
      background-color 220ms var(--ease),
      border-color 220ms var(--ease),
      box-shadow 220ms var(--ease);
  }

  .open .wing-inner {
    border-color: var(--tabbar-border, #2a2a2a);
    background: var(--tabbar-active-bg, #1e1e1e);
    box-shadow: 0 6px 22px rgba(0, 0, 0, 0.35);
  }

  .wing.left .wing-inner {
    justify-content: flex-end;
    border-right: none;
    border-radius: 999px 0 0 999px;
  }

  .wing.right .wing-inner {
    border-left: none;
    border-radius: 0 999px 999px 0;
  }

  /* Half the round button under it, and a little room beside it. */
  .open .wing.left .wing-inner {
    padding: 0 21px 0 3px;
  }

  .open .wing.right .wing-inner {
    padding: 0 3px 0 21px;
  }

  .hub {
    /* Over the halves of the bar tucked under it. */
    position: relative;
    z-index: 1;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    padding: 0;
    border: 1px solid color-mix(in srgb, var(--pane-active-border, #3a6df0) 60%, transparent);
    border-radius: 50%;
    background: color-mix(in srgb, var(--pane-active-border, #3a6df0) 22%, var(--tabbar-active-bg, #1e1e1e));
    color: inherit;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
    cursor: pointer;
    transition:
      transform 300ms var(--ease),
      background-color 220ms var(--ease),
      box-shadow 220ms var(--ease);
  }

  .hub svg {
    width: 15px;
    height: 15px;
    transition: transform 300ms var(--ease);
  }

  .open .hub {
    background: var(--pane-active-border, #3a6df0);
    color: #fff;
    box-shadow: none;
  }

  .open .hub svg {
    transform: rotate(-15deg) scale(0.92);
  }

  .hub:focus-visible {
    outline: 2px solid var(--pane-active-border, #3a6df0);
    outline-offset: 2px;
  }

  .dirty-dot {
    position: absolute;
    top: 1px;
    right: 1px;
    width: 8px;
    height: 8px;
    border: 2px solid var(--tabbar-active-bg, #1e1e1e);
    border-radius: 50%;
    background: #e3b341;
  }

  .tool {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    min-width: 28px;
    height: 28px;
    padding: 0 6px;
    border: none;
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
    /* Folded: small and faded, then each one comes in after the one nearer
       the middle. */
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity 160ms ease,
      transform 160ms ease,
      background-color 120ms ease;
  }

  .open .tool {
    opacity: 0.85;
    transform: none;
    transition:
      opacity 220ms ease calc(60ms + var(--i) * 35ms),
      transform 320ms var(--ease) calc(60ms + var(--i) * 35ms),
      background-color 120ms ease;
  }

  .tool svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .open .tool:hover:not(:disabled),
  .open .tool.active {
    opacity: 1;
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  .open .tool:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .open .tool.on {
    opacity: 1;
    color: var(--pane-active-border, #3a6df0);
  }

  .tool:focus-visible {
    outline: 1px solid var(--pane-active-border, #3a6df0);
    outline-offset: -1px;
  }

  .sep {
    flex-shrink: 0;
    width: 1px;
    height: 14px;
    margin: 0 3px;
    background: var(--tabbar-border, #2a2a2a);
  }

  .chip {
    padding: 0 9px 0 7px;
  }

  .chip-branch {
    max-width: 16ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mark-dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
  }

  .count {
    font-variant-numeric: tabular-nums;
  }

  .count.added {
    color: var(--added);
  }

  .count.removed {
    color: var(--removed);
  }

  .git-card {
    width: 300px;
    padding: 9px 11px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 10px;
    background: var(--tabbar-active-bg, #1e1e1e);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    animation: card-in 220ms var(--ease);
  }

  @keyframes card-in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 5px 10px;
    margin: 0;
  }

  dt {
    opacity: 0.55;
  }

  dd {
    min-width: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-wrap: anywhere;
  }

  dd.state {
    flex-direction: row;
    align-items: center;
    gap: 6px;
  }

  .subject {
    font-weight: 600;
  }

  .meta {
    opacity: 0.65;
  }

  .sha {
    align-self: flex-start;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px;
    opacity: 0.6;
    cursor: pointer;
  }

  .sha:hover {
    opacity: 1;
    text-decoration: underline;
  }

  .card-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 8px;
  }

  .card-actions:empty {
    display: none;
  }

  .action {
    padding: 3px 9px;
    border: 1px solid var(--tabbar-border, #2a2a2a);
    border-radius: 999px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }

  .action:hover {
    background: var(--titlebar-hover-bg, rgba(255, 255, 255, 0.08));
  }

  @media (prefers-reduced-motion: reduce) {
    .wing,
    .dock,
    .hub,
    .hub svg,
    .tool,
    .open .tool {
      transition: none;
    }

    .git-card {
      animation: none;
    }
  }
</style>
