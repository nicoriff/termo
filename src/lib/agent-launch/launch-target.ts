import type { ToolId } from "$lib/ai-mode/pattern-table";

/** What the where-to-run dialog opens once a folder is picked: an agent,
 * started by typing its command into the default shell, or a shell of its
 * own (PowerShell, CMD…) opened right in that folder. */
export type LaunchTarget =
  | { kind: "agent"; tool: ToolId }
  | {
      kind: "shell";
      path: string;
      args?: string[];
      /** Its display name, which also titles the new tab. */
      name: string;
      /** The detected shell id, for its icon (see ConsoleIcon). */
      iconId: string;
    };
