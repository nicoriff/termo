import { getDiagnostics, type Diagnostics } from "../core/pty-client";
import { cpuOfMachine, pushSample } from "./diagnostics";

const POLL_MS = 1000;

/** The docked panel's width: a default, and how far it can be dragged. */
export const PANEL_WIDTH = { initial: 380, min: 300, max: 640 };
const WIDTH_KEY = "termo.diagnostics.width";

export function clampPanelWidth(width: number): number {
  return Math.round(Math.min(PANEL_WIDTH.max, Math.max(PANEL_WIDTH.min, width)));
}

/** Only a convenience: a browser storage that's missing or throws just means
 * the default width. */
function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved ? clampPanelWidth(saved) : PANEL_WIDTH.initial;
  } catch {
    return PANEL_WIDTH.initial;
  }
}

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
  width = $state(loadWidth());
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

  /** `persist` once a drag is let go, not on every pixel of it. */
  setWidth(width: number, persist = false): void {
    this.width = clampPanelWidth(width);
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.width));
    } catch {
      // Not remembered; it still applies.
    }
  }

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
