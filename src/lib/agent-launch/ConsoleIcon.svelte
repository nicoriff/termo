<script lang="ts">
  /**
   * A small glyph per kind of console, for the new-tab menu: the shells by
   * their detected id (see src-tauri/src/pty/shell_detect.rs) and the
   * agents. Drawn here rather than taken from each vendor's logo, in each
   * one's usual color so they're told apart at a glance.
   */
  let { kind, size = 14 }: { kind: string; size?: number } = $props();

  const family = $derived(
    kind === "claude" || kind === "codex" || kind === "gemini"
      ? kind
      : kind === "pwsh" || kind === "powershell"
        ? "powershell"
        : kind === "cmd"
          ? "cmd"
          : kind === "git-bash"
            ? "git"
            : kind.startsWith("wsl-")
              ? "wsl"
              : "posix",
  );
</script>

<svg class="console-icon" width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
  {#if family === "claude"}
    <!-- A spark: rays around a center, in Claude's clay orange. -->
    <g stroke="#d97757" stroke-width="1.8" stroke-linecap="round">
      <line x1="8" y1="1.5" x2="8" y2="14.5" />
      <line x1="1.5" y1="8" x2="14.5" y2="8" />
      <line x1="3.4" y1="3.4" x2="12.6" y2="12.6" />
      <line x1="12.6" y1="3.4" x2="3.4" y2="12.6" />
    </g>
  {:else if family === "codex"}
    <!-- A hexagon holding a prompt, in OpenAI's green. -->
    <path d="M8 1.2l5.9 3.4v6.8L8 14.8l-5.9-3.4V4.6z" fill="none" stroke="#10a37f" stroke-width="1.4" />
    <path d="M5.6 6.2l2 1.8-2 1.8M8.8 10h1.8" fill="none" stroke="#10a37f" stroke-width="1.3" stroke-linecap="round" />
  {:else if family === "gemini"}
    <!-- A four-point sparkle with curved sides, in Gemini's blue. -->
    <path d="M8 1c.6 4.2 2.8 6.4 7 7-4.2.6-6.4 2.8-7 7-.6-4.2-2.8-6.4-7-7 4.2-.6 6.4-2.8 7-7z" fill="#4796e3" />
  {:else if family === "powershell"}
    <rect x="1" y="2" width="14" height="12" rx="2" fill="#2671be" />
    <path d="M4.2 5.2l3 2.8-3 2.8" fill="none" stroke="#fff" stroke-width="1.4" stroke-linecap="round" />
    <line x1="8.4" y1="10.8" x2="11.8" y2="10.8" stroke="#fff" stroke-width="1.4" stroke-linecap="round" />
  {:else if family === "cmd"}
    <rect x="1" y="2" width="14" height="12" rx="2" fill="#2b2b2b" stroke="#8a8a8a" stroke-width="1" />
    <path d="M4 5.5l2.4 2.5L4 10.5" fill="none" stroke="#e0e0e0" stroke-width="1.3" stroke-linecap="round" />
    <line x1="7.8" y1="10.5" x2="11.5" y2="10.5" stroke="#e0e0e0" stroke-width="1.3" stroke-linecap="round" />
  {:else if family === "git"}
    <!-- Git's diamond with a branch. -->
    <rect x="3" y="3" width="10" height="10" rx="1.6" transform="rotate(45 8 8)" fill="#f05032" />
    <g stroke="#fff" stroke-width="1.1" stroke-linecap="round" fill="#fff">
      <line x1="6.6" y1="5.4" x2="6.6" y2="10.6" />
      <line x1="6.6" y1="6.6" x2="9.4" y2="8.6" />
      <circle cx="6.6" cy="10.6" r="0.9" />
      <circle cx="9.4" cy="8.6" r="0.9" />
    </g>
  {:else if family === "wsl"}
    <!-- Linux on Windows: a prompt on a warm Linux-orange tile. -->
    <rect x="1" y="2" width="14" height="12" rx="2" fill="#e95420" />
    <path d="M4.2 5.2l3 2.8-3 2.8" fill="none" stroke="#fff" stroke-width="1.4" stroke-linecap="round" />
    <line x1="8.4" y1="10.8" x2="11.8" y2="10.8" stroke="#fff" stroke-width="1.4" stroke-linecap="round" />
  {:else}
    <rect x="1" y="2" width="14" height="12" rx="2" fill="none" stroke="currentColor" stroke-width="1.1" />
    <path d="M4.2 5.2l3 2.8-3 2.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
    <line x1="8.4" y1="10.8" x2="11.8" y2="10.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
  {/if}
</svg>

<style>
  .console-icon {
    flex-shrink: 0;
    display: block;
  }
</style>
