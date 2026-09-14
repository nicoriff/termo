import { Terminal, type IDisposable } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import { classify, type ClassifyState } from "../ai-mode/classifier";
import { appConfigStore } from "../core/app-config.svelte";
import { killPty, resizePty, spawnPty, writePty } from "../core/pty-client";
import { eventMatchesKeybinding } from "../core/shortcuts";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { toXtermTheme } from "../theme/theme-loader";
import { themeStore } from "../theme/theme-store.svelte";
import type { Theme } from "../theme/theme-types";
import { OutputHighlighter } from "./output-highlighter";
import { registerPty, unregisterPty } from "./pty-registry";
import { tabTitleStore } from "./tab-title-store.svelte";

// How often the AI-mode classifier re-reads the terminal's rendered screen
// buffer, in ms. xterm.js's onRender can fire many times a second during a
// busy TUI redraw; this keeps that cheap without missing state changes a
// human would notice.
const CLASSIFY_THROTTLE_MS = 250;
// How many of the most recently rendered rows to feed the classifier —
// enough for a typical status line/prompt, without scanning full scrollback.
const CLASSIFY_ROW_COUNT = 20;

function withMonospaceFallback(family: string): string {
  return family.includes(",") ? family : `"${family}", monospace`;
}

export interface ThemeApplyOptions {
  fontSizeOverride: number | null;
  fontFamilyOverride: string | null;
  /** 0 (fully transparent) to 100 (opaque). */
  backgroundOpacity: number;
}

export class TerminalInstance {
  readonly term: Terminal;
  private readonly fit: FitAddon;
  private sessionId: string | null = null;
  private disposed = false;
  // Streaming so a multi-byte UTF-8 character split across two PTY reads
  // decodes correctly instead of producing replacement characters.
  private readonly decoder = new TextDecoder();
  // Streaming for the same reason — an ANSI escape sequence split across
  // two PTY reads shouldn't have its fragments run through the highlight
  // rules in isolation. See OutputHighlighter's docstring.
  private readonly highlighter = new OutputHighlighter();
  onExit: ((code: number) => void) | null = null;

  // Modo IA: keyed by the SessionTab id (what TileChrome/AiModePanel read
  // by), which is known upfront — unlike `sessionId` above (the PTY's own
  // id), which only exists once spawnPty() resolves inside mount().
  private readonly tabId: string;
  private renderSub: IDisposable | null = null;
  private lastClassifyAt = 0;
  private readonly classifyState: ClassifyState = { detectedTool: null };
  // Unix only — stays null forever on Windows (see PtyEvent::ForegroundProcess).
  private lastForegroundProcessPath: string | null = null;

  constructor(tabId: string) {
    this.tabId = tabId;
    const theme = themeStore.current;
    this.term = new Terminal({
      allowProposedApi: true,
      cursorBlink: true,
      theme: toXtermTheme(theme),
      fontFamily: withMonospaceFallback(themeStore.effectiveFontFamily),
      fontSize: themeStore.effectiveFontSize,
      lineHeight: theme.font.lineHeight,
      scrollback: 5000,
    });
    this.fit = new FitAddon();
    this.term.loadAddon(this.fit);
    this.term.loadAddon(new WebLinksAddon());

    const unicode11 = new Unicode11Addon();
    this.term.loadAddon(unicode11);
    this.term.unicode.activeVersion = "11";

    this.term.onData((data) => {
      if (this.sessionId) void writePty(this.sessionId, data);
    });

    this.term.onTitleChange((title) => tabTitleStore.setProgramTitle(this.tabId, title));

    // Reserve tile zoom, tab cycling and new-tab ahead of xterm's own key
    // handling — some shells/apps bind these to something else, so this has
    // to intercept before xterm forwards the keystroke to the PTY, not react
    // to it after the fact via a window listener. Every TerminalInstance
    // registers this same handler; it's fine that all of them resolve
    // through the same global focusedTileId, since the terminal actually
    // receiving keystrokes is already (in practice) the one belonging to the
    // focused tile.
    //
    // The chords differ on macOS because two modifiers there already belong
    // to someone else: Ctrl carries the shell's control characters, and
    // Option is its Meta key (Option+arrows is word navigation in zsh), so
    // the platform convention of routing app commands through Cmd isn't just
    // cosmetic — it's what keeps those keys reaching the shell. Ctrl+Tab is
    // the exception kept identical everywhere: Tab isn't a control-character
    // chord, and macOS apps use Ctrl+Tab for this too.
    this.term.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown") return true;

      if (eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("zoomTile"))) {
        event.preventDefault();
        if (event.key === "ArrowUp") {
          workspaceStore.zoomFocused();
        } else {
          workspaceStore.unzoom();
        }
        return false;
      }

      if (eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("cycleTab"), { ignore: ["shift"] })) {
        event.preventDefault();
        workspaceStore.cycleTab(event.shiftKey ? -1 : 1);
        return false;
      }

      // The exclude guard keeps the tile-overview chord (Ctrl+Alt, which
      // stays Ctrl-based on every platform) from also opening a tab, no
      // matter what the user remaps "new tab" to.
      if (eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("newTab"), { exclude: ["alt"] })) {
        event.preventDefault();
        workspaceStore.newTabLikeFocused();
        return false;
      }

      return true;
    });

    // Modo IA: re-classify this session's state from whatever is actually
    // on screen after each repaint, rather than scanning raw incoming PTY
    // bytes — Claude Code/Codex redraw in place (a spinner rewrites the same
    // row), so the relevant line may have been drawn several chunks ago and
    // not touched since. xterm has already done the cursor/clear-line
    // interpretation for us; reading its buffer back gives the true current
    // screen. Subscribed unconditionally (cheap: one flag check per render
    // when the mode is off) rather than attached/detached on toggle.
    this.renderSub = this.term.onRender(() => {
      if (!appConfigStore.aiModeEnabled || !this.sessionId) return;
      const now = performance.now();
      if (now - this.lastClassifyAt < CLASSIFY_THROTTLE_MS) return;
      this.lastClassifyAt = now;

      const buf = this.term.buffer.active;
      const rowCount = Math.min(CLASSIFY_ROW_COUNT, buf.length);
      // baseY (not buf.length - rowCount) is the top of the *live* screen —
      // using it keeps classification pinned to the current state even
      // while the user has scrolled back through history.
      const startY = Math.max(0, buf.baseY + this.term.rows - rowCount);
      const lines: string[] = [];
      for (let y = startY; y < startY + rowCount; y++) {
        lines.push(buf.getLine(y)?.translateToString(true) ?? "");
      }
      aiStatusStore.set(this.tabId, classify(lines, this.classifyState, this.lastForegroundProcessPath));
    });
  }

  async mount(container: HTMLElement, shellPath?: string, shellArgs?: string[]): Promise<void> {
    this.term.open(container);
    this.fit.fit();

    this.sessionId = await spawnPty({
      shellPath,
      shellArgs,
      cols: this.term.cols,
      rows: this.term.rows,
      onForegroundProcess: (path) => {
        this.lastForegroundProcessPath = path;
        tabTitleStore.setForegroundProcess(this.tabId, path, shellPath);
      },
      onData: (bytes) => {
        const text = this.decoder.decode(bytes, { stream: true });
        this.term.write(themeStore.outputHighlightingEnabled ? this.highlighter.feed(text) : text);
      },
      onExit: (code) => this.onExit?.(code),
    });
    registerPty(this.tabId, this.sessionId);

    // resize() no-ops while sessionId is still null, so a ResizeObserver
    // tick landing during the spawnPty() await above (e.g. surrounding
    // chrome/layout still settling) gets its correction silently dropped —
    // xterm.js ends up at the container's final size while the child
    // process stays spawned at whatever cols/rows were true at the start
    // of mount(). That mismatch is invisible for plain line-output shells
    // but shows up as misplaced/overlapping redraws in full-screen TUIs
    // (e.g. claude) that position output based on their believed terminal
    // size. Re-sync once a session exists to correct any such drift.
    this.resize();
  }

  applyTheme(theme: Theme, options: ThemeApplyOptions): void {
    if (this.disposed) return;
    this.term.options.theme = toXtermTheme(theme);
    this.term.options.fontFamily = withMonospaceFallback(options.fontFamilyOverride ?? theme.font.family);
    this.term.options.fontSize = options.fontSizeOverride ?? theme.font.size;
    this.term.options.lineHeight = theme.font.lineHeight;
    this.resize();
  }

  resize(): void {
    if (this.disposed) return;
    this.fit.fit();
    if (this.sessionId) void resizePty(this.sessionId, this.term.cols, this.term.rows);
  }

  focus(): void {
    this.term.focus();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    this.renderSub?.dispose();
    this.renderSub = null;
    // Also covers teardown paths that don't go through workspaceStore's
    // closeTab (e.g. closing the whole window/tile) — remove() is an
    // idempotent no-op if that already fired.
    aiStatusStore.remove(this.tabId);
    unregisterPty(this.tabId);
    tabTitleStore.remove(this.tabId);
    if (this.sessionId) void killPty(this.sessionId);
    this.term.dispose();
  }
}
