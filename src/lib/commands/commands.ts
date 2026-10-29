/**
 * Command palette matching. Pure — no runes, no Tauri, no i18n store — so it
 * can be unit-tested in plain Node, same approach as `core/shortcuts.ts`.
 * Labels arrive already translated; `command-registry.svelte.ts` builds them.
 */

import { normalize, scoreMatch } from "../core/fuzzy-match";
import type { ShortcutAction } from "../core/shortcuts";

// Where they used to live: callers and tests still import them from here.
export { normalize, scoreMatch };

export type CommandGroup = "navigation" | "tabs" | "layouts" | "appearance" | "git" | "aiMode" | "agents" | "extensions";

/** Display order of the groups in the palette. */
export const COMMAND_GROUPS: CommandGroup[] = [
  "navigation",
  "tabs",
  "layouts",
  "appearance",
  "git",
  "aiMode",
  "agents",
  "extensions",
];

export interface Command {
  id: string;
  group: CommandGroup;
  label: string;
  /** The shortcut that does the same, shown beside the command. */
  shortcut?: ShortcutAction;
  run: () => void;
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
