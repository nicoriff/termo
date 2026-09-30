import { Terminal, type IDisposable } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { aiStatusStore } from "../ai-mode/ai-status-store.svelte";
import { agentStatusStore } from "../ai-mode/agent-status-store.svelte";
import { tabAgentSessions } from "../ai-mode/tab-sessions.svelte";
import { classify, type ClassifyState } from "../ai-mode/classifier";
import { appConfigStore } from "../core/app-config.svelte";
import { isMac, isWindows } from "../core/platform";
import { killPty, promptRedrawPty, resizePty, spawnPty, writePty } from "../core/pty-client";
import { eventMatchesKeybinding, isCopyChord, isPasteChord, isPlainCtrlC, isPlainCtrlV } from "../core/shortcuts";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { toXtermTheme } from "../theme/theme-loader";
import { themeStore } from "../theme/theme-store.svelte";
import type { Theme } from "../theme/theme-types";
import { neutralizePaintedControls } from "./conpty-controls";
import { OutputHighlighter } from "./output-highlighter";
import { parseOsc7 } from "./osc7";
import { tabCwd } from "./tab-cwd.svelte";
import { registerPty, unregisterPty } from "./pty-registry";
import { registerTerminal, unregisterTerminal } from "./terminal-registry";
import { copySelection, pasteClipboard } from "./terminal-clipboard";
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

// How long the container size must stay put before the PTY hears about it.
// Each PTY resize makes the shell (and ConPTY on Windows) reflow and redraw
// its whole screen, so a split drag or the 140ms zoom animation, which
// resize many times in a row, would otherwise scramble the output.
const PTY_RESIZE_SETTLE_MS = 100;

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
  private oscSub: IDisposable | null = null;
  private lastClassifyAt = 0;
  private readonly classifyState: ClassifyState = { detectedTool: null };
  // Unix only — stays null forever on Windows (see PtyEvent::ForegroundProcess).
  private lastForegroundProcessPath: string | null = null;

  /** What xterm wanted to send before the PTY existed — see the onData
   * handler in the constructor. */
  private pendingInput: string[] = [];
  private container: HTMLElement | null = null;
  private lastPtySize: { cols: number; rows: number } | null = null;
  private ptyResizeTimer: ReturnType<typeof setTimeout> | null = null;
  /** Identifies the most recent font whose load we're waiting on, so a
   * quick succession of font changes only re-measures for the last one. */
  private fontToken = 0;

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

    // Buffered until the PTY id arrives rather than dropped. The console's
    // output channel is wired before pty_spawn's reply comes back, so its very
    // first bytes can reach xterm while `sessionId` is still null — and those
    // bytes are ConPTY's cursor-position query (ESC[6n), which blocks the
    // shell's startup until the terminal answers. Dropping that answer left
    // the shell attached to a console it never finished initializing: alive,
    // burning no CPU, printing nothing. It showed up when restoring a layout
    // spawned several consoles at once, which widens that window.
    this.term.onData((data) => {
      if (this.sessionId) void writePty(this.sessionId, data);
      else this.pendingInput.push(data);
    });

    this.term.onTitleChange((title) => tabTitleStore.setProgramTitle(this.tabId, title));

    // The shell announces its directory on every prompt (see
    // src-tauri/src/pty/cwd_report.rs) — the only way to know where a console
    // actually is, since the tab only remembers where it was opened. Returning
    // true marks the sequence handled so xterm doesn't pass it on as output.
    this.oscSub = this.term.parser.registerOscHandler(7, (payload) => {
      const dir = parseOsc7(payload);
      if (dir) tabCwd.set(this.tabId, dir);
      return true;
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

      // macOS: Option is the shell's Meta for these keys — Option+Backspace
      // kills the previous word, Option+arrows move by word. xterm only does
      // that with macOptionIsMeta, which would also swallow Option+letter,
      // i.e. how @, # and accents are typed on a Spanish Mac layout. So only
      // these keys are translated, and Option keeps composing characters.
      if (isMac && event.altKey && !event.ctrlKey && !event.metaKey && this.sessionId) {
        const meta: Record<string, string> = {
          Backspace: "\x1b\x7f",
          Delete: "\x1bd",
          ArrowLeft: "\x1bb",
          ArrowRight: "\x1bf",
        };
        // `code` as well as `key`: with Option held, some layouts report the
        // composed character in `key` rather than the key's own name.
        const sequence = meta[event.key] ?? meta[event.code];
        if (sequence) {
          event.preventDefault();
          void writePty(this.sessionId, sequence);
          return false;
        }
      }

      // The other half of the macOS text-editing convention: where Option is
      // "by word", Cmd is "the whole line" — Cmd+Backspace erases the line
      // being typed (^U), the same as in any native text field. No shortcut
      // uses Cmd+Backspace, so this never takes a key away from the app.
      if (
        isMac &&
        event.metaKey &&
        !event.ctrlKey &&
        !event.altKey &&
        this.sessionId &&
        (event.key === "Backspace" || event.code === "Backspace")
      ) {
        event.preventDefault();
        void writePty(this.sessionId, "\x15");
        return false;
      }

      // Copy only takes the key when there IS a selection: with none, Ctrl+C
      // has to keep reaching the shell as the interrupt. On Windows plain
      // Ctrl+C copies too (see isCopyChord), and lets go of the selection
      // as it does, so pressing it again interrupts, as in Windows Terminal.
      if (
        this.term.hasSelection() &&
        isCopyChord(event, appConfigStore.getEffectiveShortcut("copy"), isWindows)
      ) {
        event.preventDefault();
        void copySelection().then(() => {
          if (isPlainCtrlC(event)) this.term.clearSelection();
        });
        return false;
      }

      // On Windows plain Ctrl+V pastes too, as in Windows Terminal (see
      // isPasteChord): Claude Code there doesn't paste text on it. When the
      // clipboard holds no text (an image), the key still reaches the
      // program, which may take the image itself.
      if (isPasteChord(event, appConfigStore.getEffectiveShortcut("paste"), isWindows)) {
        event.preventDefault();
        const plainCtrlV = isPlainCtrlV(event);
        void pasteClipboard().then((pasted) => {
          if (!pasted && plainCtrlV && this.sessionId) void writePty(this.sessionId, "\x16");
        });
        return false;
      }

      if (eventMatchesKeybinding(event, appConfigStore.getEffectiveShortcut("selectAll"))) {
        event.preventDefault();
        this.term.selectAll();
        return false;
      }

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

      // The exclude guard keeps AltGr chords (reported as Ctrl+Alt on
      // Spanish/Latin keyboard layouts) from also opening a tab, no matter
      // what the user remaps "new tab" to.
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

  async mount(container: HTMLElement, shellPath?: string, shellArgs?: string[], cwd?: string): Promise<void> {
    this.container = container;
    this.term.open(container);
    this.fit.fit();
    void this.remeasureWhenFontLoads(
      `${themeStore.effectiveFontSize}px ${withMonospaceFallback(themeStore.effectiveFontFamily)}`,
    );

    const spawnSize = { cols: this.term.cols, rows: this.term.rows };
    const spawned = await spawnPty({
      shellPath,
      shellArgs,
      cwd,
      ...spawnSize,
      // Every callback below bails once this instance is gone. A console being
      // torn down keeps producing output (and finally an exit) for a moment,
      // and writing that into an already-disposed xterm throws — inside the
      // Tauri channel's message handler, which is the same dispatch every
      // other console's output rides on. Restoring a layout is exactly when
      // that matters: consoles are created while others are being killed.
      onForegroundProcess: (path) => {
        if (this.disposed) return;
        this.lastForegroundProcessPath = path;
        tabTitleStore.setForegroundProcess(this.tabId, path, shellPath);
      },
      onData: (bytes) => {
        if (this.disposed) return;
        const decoded = this.decoder.decode(bytes, { stream: true });
        const text = isWindows ? neutralizePaintedControls(decoded) : decoded;
        this.term.write(themeStore.outputHighlightingEnabled ? this.highlighter.feed(text) : text);
      },
      onExit: (code) => {
        if (this.disposed) return;
        this.onExit?.(code);
      },
    });
    // Disposed while the spawn was in flight (a layout restore replaces tiles
    // faster than a console starts): adopt the session only to kill it, or it
    // would outlive its terminal as an orphan shell.
    if (this.disposed) {
      void killPty(spawned);
      return;
    }
    this.sessionId = spawned;
    // Whatever xterm produced while the spawn was in flight goes out now, in
    // order — ConPTY's cursor-position query arrives that early, and the shell
    // does not finish starting until it is answered.
    for (const data of this.pendingInput) {
      void writePty(this.sessionId, data);
    }
    this.pendingInput = [];
    registerPty(this.tabId, this.sessionId);
    // What the Edit menu and the editing shortcuts act on (terminal-registry).
    registerTerminal(this.tabId, {
      hasSelection: () => this.term.hasSelection(),
      selection: () => this.term.getSelection(),
      // xterm wraps it in the bracketed-paste markers when the program turned
      // them on (Claude Code, PSReadLine do) and hands it to onData — the PTY.
      paste: (text) => this.term.paste(text),
      selectAll: () => this.term.selectAll(),
      clear: () => this.term.clear(),
      focus: () => this.focus(),
    });
    this.lastPtySize = spawnSize;

    // resize() no-ops while sessionId is still null, so a ResizeObserver
    // tick landing during the spawnPty() await above (e.g. surrounding
    // chrome/layout still settling) gets its correction silently dropped —
    // xterm.js ends up at the container's final size while the child
    // process stays spawned at whatever cols/rows were true at the start
    // of mount(). That mismatch is invisible for plain line-output shells
    // but shows up as misplaced/overlapping redraws in full-screen TUIs
    // (e.g. claude) that position output based on their believed terminal
    // size. Re-sync once a session exists to correct any such drift.
    this.resize({ immediate: true });
  }

  applyTheme(theme: Theme, options: ThemeApplyOptions): void {
    if (this.disposed) return;
    const fontFamily = withMonospaceFallback(options.fontFamilyOverride ?? theme.font.family);
    const fontSize = options.fontSizeOverride ?? theme.font.size;
    this.term.options.theme = toXtermTheme(theme);
    this.term.options.fontFamily = fontFamily;
    this.term.options.fontSize = fontSize;
    this.term.options.lineHeight = theme.font.lineHeight;
    this.resize();
    void this.remeasureWhenFontLoads(`${fontSize}px ${fontFamily}`);
  }

  /** xterm measures the character cell as soon as the font options are set and
   * caches that one measurement. A font from the picker arrives through an
   * @font-face with `font-display: swap` (see theme/google-fonts.ts), so the
   * measurement can land on the fallback face and then never be revisited:
   * every cell afterwards is drawn at a width the shell doesn't share, which
   * is what leaves the cursor a few columns off and characters on screen that
   * the shell believes it already erased. */
  private async remeasureWhenFontLoads(fontSpec: string): Promise<void> {
    const token = ++this.fontToken;
    try {
      await document.fonts.load(fontSpec);
      await document.fonts.ready;
    } catch {
      // An unloadable family just keeps the fallback metrics, which are
      // already what's on screen.
      return;
    }
    if (this.disposed || token !== this.fontToken) return;
    // OptionsService drops a write that equals the current value, so the cell
    // is only re-measured if the family actually changes — hence the detour
    // through a face that is never the configured one.
    const family = this.term.options.fontFamily ?? "monospace";
    this.term.options.fontFamily = family === "serif" ? "monospace" : "serif";
    this.term.options.fontFamily = family;
    this.resize({ immediate: true });
  }

  /** Fits xterm to its container and, once the size settles, tells the PTY.
   * Skipped while the pane is hidden: an inactive tab is `display: none`, so
   * its container measures nothing and fitting would shrink the session to a
   * few columns, making the shell reflow its whole screen into garbage by
   * the time the tab is shown again. */
  resize(options?: { immediate?: boolean }): void {
    if (this.disposed) return;
    if (this.container && (this.container.clientWidth === 0 || this.container.clientHeight === 0)) return;
    this.fit.fit();
    if (!this.sessionId) return;
    if (this.ptyResizeTimer) clearTimeout(this.ptyResizeTimer);
    this.ptyResizeTimer = null;
    if (options?.immediate) {
      this.syncPtySize();
      return;
    }
    this.ptyResizeTimer = setTimeout(() => {
      this.ptyResizeTimer = null;
      this.syncPtySize();
    }, PTY_RESIZE_SETTLE_MS);
  }

  /** Only a real change in cols/rows reaches the PTY: re-showing a tab or a
   * repaint at the same size must not make the shell redraw. */
  private syncPtySize(): void {
    if (this.disposed || !this.sessionId) return;
    const { cols, rows } = this.term;
    if (this.lastPtySize?.cols === cols && this.lastPtySize.rows === rows) return;
    this.lastPtySize = { cols, rows };
    // A prompt drawn against the old width doesn't move on its own — the
    // console just reflows the cells it already has, which is how
    // oh-my-posh's right-hand block ends up wrapped onto a second row. The
    // backend decides whether this shell needs (and can take) the nudge.
    const id = this.sessionId;
    void resizePty(id, cols, rows)
      .then(() => promptRedrawPty(id))
      .catch(() => {});
  }

  focus(): void {
    this.term.focus();
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    if (this.ptyResizeTimer) clearTimeout(this.ptyResizeTimer);
    this.renderSub?.dispose();
    this.renderSub = null;
    this.oscSub?.dispose();
    this.oscSub = null;
    tabCwd.remove(this.tabId);
    // Also covers teardown paths that don't go through workspaceStore's
    // closeTab (e.g. closing the whole window/tile) — remove() is an
    // idempotent no-op if that already fired.
    aiStatusStore.remove(this.tabId);
    agentStatusStore.remove(this.tabId);
    tabAgentSessions.remove(this.tabId);
    unregisterPty(this.tabId);
    unregisterTerminal(this.tabId);
    tabTitleStore.remove(this.tabId);
    if (this.sessionId) void killPty(this.sessionId);
    this.term.dispose();
  }
}
