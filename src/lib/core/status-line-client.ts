import { invoke } from "@tauri-apps/api/core";

/** Whether Termo is chained onto Claude Code's `statusLine` (see
 * src-tauri/src/pty/status_line.rs). */
export function isStatusLineInstalled(): Promise<boolean> {
  return invoke("status_line_installed");
}

export function installStatusLine(): Promise<void> {
  return invoke("install_status_line");
}

export function uninstallStatusLine(): Promise<void> {
  return invoke("uninstall_status_line");
}

/** The same for Copilot CLI's (see src-tauri/src/pty/copilot_status.rs). */
export function isCopilotStatusLineInstalled(): Promise<boolean> {
  return invoke("copilot_status_line_installed");
}

export function installCopilotStatusLine(): Promise<void> {
  return invoke("install_copilot_status_line");
}

export function uninstallCopilotStatusLine(): Promise<void> {
  return invoke("uninstall_copilot_status_line");
}
