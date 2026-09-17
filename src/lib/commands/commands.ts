/**
 * Command palette matching. Pure — no runes, no Tauri, no i18n store — so it
 * can be unit-tested in plain Node, same approach as `core/shortcuts.ts`.
 * Labels arrive already translated; `command-registry.svelte.ts` builds them.
 */

export type CommandGroup = "navigation" | "tabs" | "layouts" | "appearance" | "aiMode";

/** Display order of the groups in the palette. */
export const COMMAND_GROUPS: CommandGroup[] = ["navigation", "tabs", "layouts", "appearance", "aiMode"];

export interface Command {
  id: string;
  group: CommandGroup;
  label: string;
  run: () => void;
}

/** Lowercased and accent-free, so "seleccion" also matches "Selección". */
export function normalize(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

const STARTS_WITH = 0;
const WORD_START = 1;
const CONTAINS = 2;
const SUBSEQUENCE = 3;

/** Lower is a better match; `null` means the query doesn't match at all.
 * Subsequence is what makes initials work: "gtp" finds "Go to Preferences". */
export function scoreMatch(label: string, query: string): number | null {
  const text = normalize(label);
  const needle = normalize(query).trim();
  if (needle === "") return STARTS_WITH;

  if (text.startsWith(needle)) return STARTS_WITH;
  const at = text.indexOf(needle);
  if (at > 0) return text[at - 1] === " " ? WORD_START : CONTAINS;

  let cursor = 0;
  for (const char of needle) {
    cursor = text.indexOf(char, cursor);
    if (cursor === -1) return null;
    cursor++;
  }
  return SUBSEQUENCE;
}

/** Matching commands, best first; ties keep group order and then the order
 * the registry produced. An empty query returns everything. */
export function matchCommands<T extends { group: CommandGroup; label: string }>(commands: T[], query: string): T[] {
  return commands
    .map((command, index) => ({ command, index, score: scoreMatch(command.label, query) }))
    .filter((entry): entry is { command: T; index: number; score: number } => entry.score !== null)
    .sort(
      (a, b) =>
        a.score - b.score ||
        COMMAND_GROUPS.indexOf(a.command.group) - COMMAND_GROUPS.indexOf(b.command.group) ||
        a.index - b.index,
    )
    .map((entry) => entry.command);
}
