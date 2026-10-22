import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { workspaceStore } from "../layout/workspace-store.svelte";
import { getTerminal, type TerminalHandle } from "./terminal-registry";

/** The console the keyboard is on — what every editing action acts upon.
 * Goes through the clipboard plugin rather than navigator.clipboard, which
 * webviews routinely refuse to read from. */
export function focusedTerminal(): TerminalHandle | undefined {
  const tabId = workspaceStore.focusedSessionId;
  return tabId ? getTerminal(tabId) : undefined;
}

export function focusedHasSelection(): boolean {
  return focusedTerminal()?.hasSelection() ?? false;
}

/** Returns false when there was nothing selected, which is what lets Ctrl+C
 * keep meaning "interrupt" in that case. */
export async function copySelection(): Promise<boolean> {
  const terminal = focusedTerminal();
  if (!terminal?.hasSelection()) return false;
  const text = terminal.selection();
  if (!text) return false;
  await writeText(text);
  return true;
}

/** Returns false when the clipboard held no text (an image, say), which is
 * what lets a plain Ctrl+V still reach the program on Windows.
 * The line breaks the copy ends on are dropped, as Windows Terminal does:
 * copying a whole line takes its newline along, and where the program
 * didn't ask for bracketed paste (any shell behind the Windows 10 ConPTY)
 * that newline is an Enter that runs the command. */
export async function pasteClipboard(): Promise<boolean> {
  const terminal = focusedTerminal();
  if (!terminal) return false;
  const text = (await readText().catch(() => "")).replace(/[\r\n]+$/, "");
  if (text) terminal.paste(text);
  terminal.focus();
  return Boolean(text);
}

export function selectAll(): void {
  const terminal = focusedTerminal();
  terminal?.selectAll();
  terminal?.focus();
}

export function clearConsole(): void {
  const terminal = focusedTerminal();
  terminal?.clear();
  terminal?.focus();
}
