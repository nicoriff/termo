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

export async function pasteClipboard(): Promise<void> {
  const terminal = focusedTerminal();
  if (!terminal) return;
  const text = await readText();
  if (text) terminal.write(text);
  terminal.focus();
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
