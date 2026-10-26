import { invoke } from "@tauri-apps/api/core";
import type { PromptEngine } from "./types";

/** A theme's primary prompt as the tool prints it (ANSI), for `cwd` at
 * `width` columns. `theme` null: Starship with the user's own starship.toml.
 * Downloads an Oh My Posh theme that isn't yet. */
export function promptPreview(
  tool: Exclude<PromptEngine, "none">,
  theme: string | null,
  cwd: string | null,
  width: number,
): Promise<string> {
  return invoke("prompt_preview", { tool, theme, cwd, width });
}
