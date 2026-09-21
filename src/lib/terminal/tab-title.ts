/**
 * Automatic tab titles, from two signals:
 * - the title a program sets itself (OSC 0/2, xterm's onTitleChange) — works
 *   on every OS, e.g. zsh/oh-my-posh prompts, claude, vim, ssh;
 * - the PTY's foreground process (macOS/Linux only), for programs that
 *   never set a title.
 * A program-set title wins, then the foreground process name, then the
 * tab's original shell title.
 */

export interface TabTitleState {
  programTitle: string | null;
  programTitleAt: number;
  processName: string | null;
}

export const EMPTY_TAB_TITLE: TabTitleState = { programTitle: null, programTitleAt: 0, processName: null };

const MAX_TITLE_LENGTH = 40;

// The foreground process is polled about once a second, so a program's own
// title usually arrives up to a second BEFORE its process change is noticed
// (and a shell prompt's title just before the return to the shell is).
// Titles younger than this survive a process change; older ones are stale.
export const TITLE_GRACE_MS = 1500;

const SHELL_NAMES = new Set(["sh", "bash", "zsh", "fish", "dash", "ksh", "tcsh", "csh", "nu", "pwsh", "powershell", "cmd"]);

function baseName(path: string): string {
  return (path.split(/[\\/]/).pop() ?? "").replace(/\.exe$/i, "");
}

/** Readable name for a foreground executable path, or null when it's just
 * the shell itself (the tab's own title already says that). */
export function processDisplayName(path: string | null, shellPath?: string): string | null {
  if (!path) return null;
  const parts = path.split(/[\\/]/).filter(Boolean);
  let name = baseName(path);
  // Version-pinned installs name the binary after its version, e.g.
  // ~/.local/share/claude/versions/2.1.269 — the tool is the folder above.
  if (/^v?\d+(\.\d+)+$/.test(name)) {
    const versionsIndex = parts.lastIndexOf("versions");
    name = versionsIndex > 0 ? parts[versionsIndex - 1] : (parts[parts.length - 2] ?? name);
  }
  const lower = name.toLowerCase();
  if (SHELL_NAMES.has(lower) || (shellPath && lower === baseName(shellPath).toLowerCase())) return null;
  return name;
}

export function cleanProgramTitle(raw: string): string | null {
  const title = raw.replace(/\s+/g, " ").trim();
  if (!title) return null;
  return title.length > MAX_TITLE_LENGTH ? `${title.slice(0, MAX_TITLE_LENGTH - 1)}…` : title;
}

export function applyProgramTitle(state: TabTitleState, raw: string, now: number): TabTitleState {
  const programTitle = cleanProgramTitle(raw);
  if (programTitle === state.programTitle) return state;
  return { ...state, programTitle, programTitleAt: now };
}

export function applyProcessChange(state: TabTitleState, processName: string | null, now: number): TabTitleState {
  if (processName === state.processName) return state;
  const titleIsFresh = now - state.programTitleAt <= TITLE_GRACE_MS;
  return {
    processName,
    programTitle: titleIsFresh ? state.programTitle : null,
    programTitleAt: titleIsFresh ? state.programTitleAt : 0,
  };
}

/** `consoleLabel` is what the brackets say: the tab's console title (e.g.
 * "zsh (default)", "PowerShell 7"), or the agent's name while one is running
 * in it (see tab-title-store). A program or process title is prefixed with
 * it, e.g. "[zsh] claude"; with neither, the label stands alone.
 *
 * `redundantTitles` are program titles the label already says — a Codex
 * console would otherwise read "[Codex] codex". */
export function resolveTabTitle(
  state: TabTitleState | undefined,
  consoleLabel: string,
  redundantTitles: readonly string[] = [],
): string {
  const label = consoleLabel.replace(/\s*\(default\)$/, "");
  const title = state?.programTitle ?? state?.processName;
  if (!title) return consoleLabel;
  if (redundantTitles.some((name) => name.toLowerCase() === title.toLowerCase())) return label;
  return `[${label}] ${title}`;
}
