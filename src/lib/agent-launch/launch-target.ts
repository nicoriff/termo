import type { ToolId } from "$lib/ai-mode/pattern-table";

/** What the where-to-run dialog opens once a folder is picked: an agent,
 * started by typing its command into the default shell, or a shell of its
 * own (PowerShell, CMD…) opened right in that folder. Or nothing at all: a
 * form's folder field (see FolderField) just takes the folder picked. */
export type LaunchTarget =
  | { kind: "agent"; tool: ToolId }
  | {
      kind: "folder";
      /** What the field holds now, offered first. */
      current: string | null;
      onPick: (path: string) => void;
    }
  | {
      kind: "shell";
      path: string;
      args?: string[];
      /** Its display name, which also titles the new tab. */
      name: string;
      /** The detected shell id, for its icon (see ConsoleIcon). */
      iconId: string;
    };
