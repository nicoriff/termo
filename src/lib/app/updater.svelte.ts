import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { runningAgents } from "$lib/core/agents-client";
import { runningWork } from "./quit";

/**
 * "Check for updates": asks the release server (plugins.updater in
 * tauri.conf.json) for a newer version and, when there is one, downloads it,
 * installs it and restarts — all in one go. Only signed builds are accepted:
 * the updater checks each against the public key in the config before
 * installing anything.
 *
 * The one pause: installing closes every console and stops the agents, so
 * when something is running it waits for the user to say go.
 */
export type UpdateStatus =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "upToDate" }
  /** Found one, but installing would cut these short: waiting for a yes. */
  | { kind: "waiting"; consoles: number; agents: number }
  /** `total` is null when the server doesn't say how big the file is. */
  | { kind: "downloading"; received: number; total: number | null }
  | { kind: "installing" }
  | { kind: "error"; message: string };

class UpdaterStore {
  status = $state<UpdateStatus>({ kind: "idle" });
  /** The version a check found, with its release notes (both languages;
   * see release-notes.ts `notesFor`). */
  found = $state<{ version: string; body: string } | null>(null);
  private update: Update | null = null;

  /** Checking, downloading or installing: nothing to start meanwhile. */
  get working(): boolean {
    const { kind } = this.status;
    return kind === "checking" || kind === "downloading" || kind === "installing";
  }

  async checkAndInstall(): Promise<void> {
    if (this.working) return;
    this.status = { kind: "checking" };
    this.found = null;
    try {
      const update = await check();
      if (!update) {
        this.status = { kind: "upToDate" };
        return;
      }
      this.update = update;
      this.found = { version: update.version, body: update.body ?? "" };
      const { consoles, agents } = await runningWork();
      const background = (await runningAgents().catch(() => [])).length;
      if (consoles + agents + background > 0) {
        this.status = { kind: "waiting", consoles, agents: agents + background };
        return;
      }
      await this.install();
    } catch (error) {
      this.status = { kind: "error", message: String(error) };
    }
  }

  /** Downloads and installs what the last check found, then restarts. */
  async install(): Promise<void> {
    const update = this.update;
    if (!update || this.working) return;
    let received = 0;
    let total: number | null = null;
    this.status = { kind: "downloading", received, total };
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? null;
          this.status = { kind: "downloading", received, total };
        } else if (event.event === "Progress") {
          received += event.data.chunkLength;
          this.status = { kind: "downloading", received, total };
        } else {
          this.status = { kind: "installing" };
        }
      });
      // On Windows the installer has already closed Termo and opens the new
      // one itself; elsewhere the new version is in place, waiting for this.
      await relaunch();
    } catch (error) {
      this.status = { kind: "error", message: String(error) };
    }
  }

  /** Back to the start once the dialog closes — unless it's mid-way, which
   * carries on (and restarts Termo) whether the dialog is open or not. */
  reset(): void {
    if (this.working) return;
    this.status = { kind: "idle" };
    this.found = null;
    this.update = null;
  }
}

export const updater = new UpdaterStore();
