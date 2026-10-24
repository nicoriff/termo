/**
 * CPU and memory for the status bar's diagnostics items: the machine's,
 * Termo's and each console's. Asked of the backend only while one of those
 * items is in the bar (each takes a hold while mounted), every couple of
 * seconds — the process table behind it takes tens of ms to read.
 */
import { getDiagnostics, type Diagnostics } from "$lib/core/pty-client";

const POLL_MS = 2000;

class UsageSampler {
  latest = $state<Diagnostics | null>(null);

  private holders = 0;
  private timer: ReturnType<typeof setInterval> | null = null;
  private asking = false;

  /** Starts sampling; the returned function lets go of it. */
  hold(): () => void {
    this.holders += 1;
    if (!this.timer) {
      void this.sample();
      this.timer = setInterval(() => void this.sample(), POLL_MS);
    }
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.holders -= 1;
      if (this.holders > 0 || !this.timer) return;
      clearInterval(this.timer);
      this.timer = null;
      this.latest = null;
    };
  }

  private async sample(): Promise<void> {
    if (this.asking) return;
    this.asking = true;
    try {
      const report = await getDiagnostics();
      if (this.timer) this.latest = report;
    } catch {
      // The next one may answer; the items keep what they had.
    } finally {
      this.asking = false;
    }
  }
}

export const usageSampler = new UsageSampler();
