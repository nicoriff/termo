import { invoke } from "@tauri-apps/api/core";

export interface OhMyPoshStatus {
  path: string | null;
  managed: boolean;
}

export const NERD_FONT_FAMILY = "MesloLGS Nerd Font Mono";

// Official theme names, each confirmed to exist in the oh-my-posh repo.
export const OH_MY_POSH_THEMES = [
  "jandedobbeleer",
  "atomic",
  "paradox",
  "powerlevel10k_rainbow",
  "catppuccin_mocha",
  "dracula",
  "night-owl",
  "pure",
  "robbyrussell",
  "agnoster.minimal",
];

export function getOhMyPoshStatus(): Promise<OhMyPoshStatus> {
  return invoke("oh_my_posh_status");
}

export function installOhMyPosh(): Promise<string> {
  return invoke("install_oh_my_posh");
}

export function installOhMyPoshTheme(theme: string): Promise<void> {
  return invoke("install_oh_my_posh_theme", { theme });
}

export function installNerdFont(): Promise<string> {
  return invoke("install_nerd_font");
}
