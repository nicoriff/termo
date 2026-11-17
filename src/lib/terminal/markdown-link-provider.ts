import type { IBufferCellPosition, ILink, ILinkProvider, Terminal } from "@xterm/xterm";
import { fileStamps } from "../core/explorer-client";
import { isMac, isWindows } from "../core/platform";
import { t } from "../i18n/i18n.svelte";
import { tabAgentSessions } from "../ai-mode/tab-sessions.svelte";
import { collectSessions } from "../layout/split-tree";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { findMarkdownPaths, resolveMarkdownPath } from "./markdown-links";
import { tabCwd } from "./tab-cwd.svelte";

/**
 * Markdown files a console names — an agent saying it wrote `docs/plan.md`
 * — become links: Ctrl+click (Cmd+click on macOS) shows the file rendered
 * in a tab of its own (see MarkdownView). Only paths that are files on this
 * computer are underlined, read from where the agent (or else the shell) is.
 */
export function markdownLinkProvider(term: Terminal, tabId: string): ILinkProvider {
  return {
    provideLinks(lineNumber, callback) {
      const line = logicalLine(term, lineNumber);
      // Each row of a wrapped line asks; the links are given by the one
      // they start on, so a link isn't given twice.
      const found = line ? findMarkdownPaths(line.text).filter((match) => line.cells[match.start].y === lineNumber) : [];
      const folder = found.length > 0 ? consoleFolder(tabId) : null;
      const resolved = folder === null ? [] : found.map((match) => resolveMarkdownPath(match.path, folder, isWindows));
      const candidates = found.flatMap((match, index) => {
        const path = resolved[index];
        return path ? [{ match, path }] : [];
      });
      if (!line || candidates.length === 0) {
        callback(undefined);
        return;
      }
      void fileStamps(candidates.map((candidate) => candidate.path))
        .catch(() => candidates.map(() => null))
        .then((stamps) => {
          const links: ILink[] = candidates
            .filter((_, index) => stamps[index] !== null)
            .map(({ match, path }) => ({
              range: { start: line.cells[match.start], end: line.cells[match.end - 1] },
              text: match.path,
              decorations: { pointerCursor: true, underline: true },
              activate(event) {
                if (!(isMac ? event.metaKey : event.ctrlKey)) return;
                hideHint();
                workspaceStore.openMarkdown(path, tabId);
              },
              hover(event) {
                showHint(event, t(isMac ? "markdownView.linkHintMac" : "markdownView.linkHint"));
              },
              leave() {
                hideHint();
              },
            }));
          callback(links.length > 0 ? links : undefined);
        });
    },
  };
}

/** The one hint on screen, near the pointer over a link: xterm draws the
 * console on a canvas, so a `title` never shows there. */
let hint: HTMLDivElement | null = null;

function showHint(event: MouseEvent, text: string): void {
  hint ??= document.createElement("div");
  hint.className = "termo-link-hint";
  hint.textContent = text;
  Object.assign(hint.style, {
    position: "fixed",
    zIndex: "10000",
    pointerEvents: "none",
    padding: "3px 8px",
    borderRadius: "4px",
    font: "12px system-ui, sans-serif",
    color: "var(--titlebar-fg, #d4d4d4)",
    background: "var(--tabbar-bg, #252526)",
    border: "1px solid var(--tabbar-border, #3a3a3a)",
    boxShadow: "0 2px 8px rgba(0, 0, 0, 0.35)",
    whiteSpace: "nowrap",
  });
  if (!hint.isConnected) document.body.appendChild(hint);
  // Below and right of the pointer, kept inside the window.
  const width = hint.offsetWidth;
  const height = hint.offsetHeight;
  const left = Math.min(event.clientX + 12, window.innerWidth - width - 4);
  const top = event.clientY + 18 + height > window.innerHeight ? event.clientY - height - 8 : event.clientY + 18;
  hint.style.left = `${Math.max(4, left)}px`;
  hint.style.top = `${Math.max(4, top)}px`;
}

function hideHint(): void {
  hint?.remove();
}

/** Where a relative path the console printed is read from: the agent's
 * folder while one runs in it, else the shell's. Null for another
 * computer's console — its files aren't here. */
function consoleFolder(tabId: string): string | undefined | null {
  const session = workspaceStore.root
    ? collectSessions(workspaceStore.root).find((loc) => loc.session.id === tabId)?.session
    : undefined;
  if (session?.attachHost) return null;
  return tabAgentSessions.byTab[tabId]?.cwd ?? tabCwd.byTab[tabId] ?? session?.cwd;
}

interface LogicalLine {
  text: string;
  /** Where each character of `text` is on screen, 1-based as xterm's ranges. */
  cells: IBufferCellPosition[];
}

/** The whole line `lineNumber` (1-based) is part of, with the rows a long
 * line wrapped onto, so a path broken across them is still found. */
function logicalLine(term: Terminal, lineNumber: number): LogicalLine | null {
  const buffer = term.buffer.active;
  let first = lineNumber - 1;
  while (first > 0 && buffer.getLine(first)?.isWrapped) first--;
  let last = lineNumber - 1;
  while (buffer.getLine(last + 1)?.isWrapped) last++;
  let text = "";
  const cells: IBufferCellPosition[] = [];
  for (let row = first; row <= last; row++) {
    const bufferLine = buffer.getLine(row);
    if (!bufferLine) return null;
    for (let col = 0; col < bufferLine.length; col++) {
      const cell = bufferLine.getCell(col);
      if (!cell || cell.getWidth() === 0) continue;
      const chars = cell.getChars() || " ";
      for (let index = 0; index < chars.length; index++) cells.push({ x: col + 1, y: row + 1 });
      text += chars;
    }
  }
  return { text, cells };
}
