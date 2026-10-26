import { invoke } from "@tauri-apps/api/core";

export interface StarshipStatus {
  path: string | null;
  managed: boolean;
}

// The presets built into starship (`starship preset --list`).
export const STARSHIP_PRESETS = [
  "nerd-font-symbols",
  "no-nerd-font",
  "plain-text-symbols",
  "bracketed-segments",
  "no-runtime-versions",
  "no-empty-icons",
  "pure-preset",
  "pastel-powerline",
  "tokyo-night",
  "gruvbox-rainbow",
  "catppuccin-powerline",
  "jetpack",
];

export function getStarshipStatus(): Promise<StarshipStatus> {
  return invoke("starship_status");
}

export function installStarship(): Promise<string> {
  return invoke("install_starship");
}

export function installStarshipPreset(preset: string): Promise<void> {
  return invoke("install_starship_preset", { preset });
}
