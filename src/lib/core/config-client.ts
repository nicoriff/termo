import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "./types";

export function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

export function saveConfig(config: AppConfig): Promise<void> {
  return invoke("save_config", { config });
}
