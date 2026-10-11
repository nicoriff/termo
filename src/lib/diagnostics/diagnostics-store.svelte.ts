import { getDiagnostics, type Diagnostics } from "../core/pty-client";
import { cpuOfMachine, pushSample } from "./diagnostics";

const POLL_MS = 1000;

/** A console's last minute, by the PTY id the backend reports. */
export interface ConsoleHistory {
  cpu: number[];
  memory: number[];
}

/**
 * The readings the diagnostics panel draws. It asks the backend once a
 * second while the panel is open (`start`) and nothing otherwise: the
 * process table it reads takes tens of ms, so it isn't worth keeping warm.
 * The history starts over each time the panel opens.
 */
class DiagnosticsStore {
  latest = $state<Diagnostics | null>(null);
  failed = $state(false);
  /** Percent of the whole machine, so the three CPU lines share a scale. */
  appCpu = $state<number[]>([]);
  appMemory = $state<number[]>([]);
  systemCpu = $state<number[]>([]);
  systemMemory = $state<number[]>([]);
  byConsole = $state<Record<string, ConsoleHistory>>({});

  private timer: ReturnType<typeof setInterval> | null = null;
  private asking = false;

  start(): void {
    if (this.timer) return;
    this.reset();
    void this.poll();
    this.timer = setInterval(() => void this.poll(), POLL_MS);
  }

  stop(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  private reset(): void {
    this.latest = null;
    this.failed = false;
    this.appCpu = [];
    this.appMemory = [];
    this.systemCpu = [];
    this.systemMemory = [];
    this.byConsole = {};
  }

  private async poll(): Promise<void> {
    // A slow reading mustn't pile up behind itself.
    if (this.asking) return;
    this.asking = true;
    try {
      this.record(await getDiagnostics());
      this.failed = false;
    } catch {
      this.failed = true;
    } finally {
      this.asking = false;
    }
  }

  private record(report: Diagnostics): void {
    // Stopped while the reading was on its way.
    if (!this.timer) return;
    this.latest = report;
    this.appCpu = pushSample(this.appCpu, cpuOfMachine(report.app.cpu, report.cores));
    this.appMemory = pushSample(this.appMemory, report.app.memory);
    this.systemCpu = pushSample(this.systemCpu, report.system.cpu);
    this.systemMemory = pushSample(this.systemMemory, report.system.memoryUsed);
    // Only the consoles still there: a closed tab's history goes with it.
    const next: Record<string, ConsoleHistory> = {};
    for (const usage of report.consoles) {
      const before = this.byConsole[usage.id];
      next[usage.id] = {
        cpu: pushSample(before?.cpu ?? [], cpuOfMachine(usage.cpu, report.cores)),
        memory: pushSample(before?.memory ?? [], usage.memory),
      };
    }
    this.byConsole = next;
  }
}

export const diagnosticsStore = new DiagnosticsStore();
