import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { isMac } from "../core/platform";
import { killPty, resizePty, spawnPty, writePty } from "../core/pty-client";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { toXtermTheme } from "../theme/theme-loader";
import { themeStore } from "../theme/theme-store.svelte";
import type { Theme } from "../theme/theme-types";
import { OutputHighlighter } from "./output-highlighter";

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

  constructor() {
    const theme = themeStore.current;
    this.term = new Terminal({
      allowProposedApi: true,
      cursorBlink: true,
      theme: toXtermTheme(theme, themeStore.backgroundOpacity / 100),
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

      const zoomHeld = isMac ? event.metaKey : event.altKey;
      if (zoomHeld && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
        event.preventDefault();
        if (event.key === "ArrowUp") {
          workspaceStore.zoomFocused();
        } else {
          workspaceStore.unzoom();
        }
        return false;
      }

      if (event.ctrlKey && event.key === "Tab") {
        event.preventDefault();
        workspaceStore.cycleTab(event.shiftKey ? -1 : 1);
        return false;
      }

      // The !altKey guard keeps the tile-overview chord (Ctrl+Alt, which
      // stays Ctrl-based on every platform) from also opening a tab.
      const newTabHeld = isMac ? event.metaKey : event.ctrlKey && !event.altKey;
      if (newTabHeld && event.key.toLowerCase() === "t") {
        event.preventDefault();
        workspaceStore.newTabLikeFocused();
        return false;
      }

      return true;
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
      onData: (bytes) => {
        const text = this.decoder.decode(bytes, { stream: true });
        this.term.write(themeStore.outputHighlightingEnabled ? this.highlighter.feed(text) : text);
      },
      onExit: (code) => this.onExit?.(code),
    });

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
    this.term.options.theme = toXtermTheme(theme, options.backgroundOpacity / 100);
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
    if (this.sessionId) void killPty(this.sessionId);
    this.term.dispose();
  }
}
