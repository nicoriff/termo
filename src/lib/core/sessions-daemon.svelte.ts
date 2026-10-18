import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Whether the sessions daemon (see src-tauri/src/mux/) answers. When it
 * stops, new consoles open in the app — they won't keep running — and the
 * status bar says so; restarting it ends the consoles it ran.
 */
class SessionsDaemon {
  answering = $state(true);
  restarting = $state(false);
  restartError = $state<string | null>(null);

  start(): () => void {
    void invoke<boolean>("sessions_daemon_answering")
      .then((answering) => (this.answering = answering))
      .catch(() => {});
    const unlisten = listen<boolean>("sessions-daemon", (event) => (this.answering = event.payload));
    return () => void unlisten.then((stop) => stop());
  }

  async restart(): Promise<void> {
    this.restarting = true;
    this.restartError = null;
    try {
      await invoke("sessions_daemon_restart");
      this.answering = true;
    } catch (err) {
      this.restartError = String(err);
    } finally {
      this.restarting = false;
    }
  }
}

export const sessionsDaemon = new SessionsDaemon();
