import { invoke } from "@tauri-apps/api/core";

/** Whether Termini is chained onto Claude Code's `statusLine` (see
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
