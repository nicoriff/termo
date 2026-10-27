import {
  gitBranches,
  gitBusy,
  gitCancel,
  gitPr,
  gitPrCreate,
  gitPrUpdate,
  gitRun,
  gitSnapshot,
  onGitOp,
  type Branch,
  type Forge,
  type GitOp,
  type GitOpKind,
  type NewPr,
  type PrPatch,
  type PrState,
  type Remote,
  type RepoSnapshot,
} from "$lib/core/git-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { fileTabs } from "$lib/editor/file-tabs.svelte";
import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
import { tabTitleStore } from "$lib/terminal/tab-title-store.svelte";
import { agentsStore } from "$lib/agents/agents-store.svelte";
import { explorerStore } from "$lib/explorer/explorer-store.svelte";
import { tabGroupsStore } from "$lib/tab-groups/tab-groups-store.svelte";
import { harnessStore } from "$lib/harness/harness-store.svelte";
import { countChanges } from "./git-summary";

/** How often the repository is looked at again while something shows it:
 * what's done from the console (or by an agent) turns up without a refresh. */
const POLL_MS = 4000;
/** A request barely moves, and asking for it goes over the network. */
const PR_POLL_MS = 60_000;
/** How much of what an action prints is kept to show. */
const MAX_LINES = 200;

/** The action under way on the repository in view. */
export interface RunningOp {
  root: string;
  op: GitOpKind;
  lines: string[];
}

/** The remote the branch's work goes to: its upstream's, else `origin`, else
 * the first there is (the backend's `main_remote`). */
export function mainRemote(snapshot: RepoSnapshot | null): Remote | null {
  if (!snapshot) return null;
  const tracked = snapshot.remotes
    .filter((remote) => snapshot.upstream?.startsWith(`${remote.name}/`))
    .sort((a, b) => b.name.length - a.name.length)[0];
  return tracked ?? snapshot.remotes.find((remote) => remote.name === "origin") ?? snapshot.remotes[0] ?? null;
}

/**
 * The repository of the tab in focus, for the Git panel, the status bar's
 * item and the button's badge: what it looks like (kept fresh on a short
 * poll while any of them shows it), its pull request, and the actions run
 * on it (see src-tauri/src/git).
 *
 * The panel docks at the window's left edge like the explorer and the agents
 * panel, so opening one closes the others.
 */
class GitStore {
  open = $state(false);
  snapshot = $state<RepoSnapshot | null>(null);
  /** Whether the folder in focus has been looked at yet. */
  loaded = $state(false);
  branches = $state<Branch[]>([]);
  pr = $state<PrState | null>(null);
  /** A request being read, changed or opened. */
  prBusy = $state(false);
  prError = $state<string | null>(null);
  running = $state<RunningOp | null>(null);
  /** Why the last action failed, until the next one starts or it's closed. */
  error = $state<string | null>(null);
  /** What the last action that worked printed last. */
  lastOutput = $state<string | null>(null);
  /** The commit message being written, by repository: closing the panel or
   * moving to another tab doesn't lose it. */
  drafts = $state<Record<string, string>>({});
  /** Bumped to ask the panel to put the keyboard in the commit message. */
  commitFocusNonce = $state(0);

  /** The folder of the tab in focus. */
  private folder = $derived.by(() => {
    const session = workspaceStore.focusedSession;
    return session ? (sessionFolder(session) ?? null) : null;
  });
  /** Whether anything on screen shows the repository. */
  private watching = $derived(appConfigStore.gitEnabled && (this.open || appConfigStore.gitStatusBar));
  private wantsPr = $derived(
    appConfigStore.gitEnabled &&
      appConfigStore.gitPrEnabled &&
      (this.open || (appConfigStore.gitStatusBar && appConfigStore.gitStatusParts.pr)),
  );

  root = $derived(this.snapshot?.root ?? null);
  branch = $derived(this.snapshot?.branch ?? null);
  counts = $derived(countChanges(this.snapshot?.files ?? []));
  remote = $derived(mainRemote(this.snapshot));
  forge = $derived<Forge | null>(this.remote?.forge ?? null);
  /** Changes whenever the history may have: what the graph and the branch
   * list are read again for. */
  historyKey = $derived(
    this.snapshot
      ? [
          this.snapshot.root,
          this.snapshot.branch,
          this.snapshot.head?.sha,
          this.snapshot.upstream,
          this.snapshot.ahead,
          this.snapshot.behind,
          this.snapshot.lastFetch,
          this.snapshot.stashCount,
        ].join("|")
      : "",
  );
  /** macOS and Linux: the console in focus is running git itself (Windows
   * consoles don't report their foreground process). */
  consoleRunsGit = $derived.by(() => {
    const session = workspaceStore.focusedSession;
    return session ? tabTitleStore.byTab[session.id]?.processName?.toLowerCase() === "git" : false;
  });

  /** The folder last asked about: a tab that's in none (an agent's runs)
   * leaves the repository in view as it was. */
  private target: string | null = null;
  private looking = false;
  private lookAgain = false;
  private lastJson = "";
  /** The repository and branch `pr` is of. */
  private prOf = "";
  /** The repository already checked for an action another window started. */
  private adopted = "";
  /** Whether the action under way was started from this window. */
  private ownRun = false;
  /** Asked before anything is pushed from here; false keeps it from going.
   * The reviews set it (see git-review-store.svelte.ts). */
  beforePush: (() => Promise<boolean>) | null = null;
  /** Gives up on what `beforePush` is waiting for. */
  cancelBeforePush: (() => void) | null = null;
  /** A push is waiting on `beforePush`. */
  private holding = false;
  /** Told when a pull request was opened for the branch. */
  afterPrCreate: (() => void) | null = null;

  /** Starts following the tab in focus. The main window calls it once;
   * returns what stops it. */
  start(): () => void {
    let unlisten: (() => void) | null = null;
    let stopped = false;
    void onGitOp((event) => this.onOp(event.root, event.op, event.stage, event.line)).then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    });

    const stopEffects = $effect.root(() => {
      $effect(() => {
        if (!this.watching) return;
        const folder = this.folder;
        // A save changes what git sees right away.
        void fileTabs.savedTick;
        if (folder && folder !== this.target) {
          this.target = folder;
          this.loaded = false;
        }
        void this.refresh();
        const timer = setInterval(() => void this.refresh(), POLL_MS);
        return () => clearInterval(timer);
      });

      // The request follows the branch, and is asked again now and then
      // while the panel is open.
      $effect(() => {
        const root = this.root;
        void this.branch;
        void this.snapshot?.upstream;
        if (!root || !this.wantsPr) {
          this.pr = null;
          this.prOf = "";
          return;
        }
        // Another branch's request says nothing of this one; the same
        // branch's stays up while it's asked again.
        const of = `${root}|${this.branch}`;
        if (of !== this.prOf) {
          this.prOf = of;
          this.pr = null;
          this.prError = null;
        }
        void this.loadPr(false);
        if (!this.open) return;
        const timer = setInterval(() => void this.loadPr(false), PR_POLL_MS);
        return () => clearInterval(timer);
      });

      $effect(() => {
        void this.historyKey;
        const root = this.root;
        if (!root || !this.open) return;
        let cancelled = false;
        void gitBranches(root).then((list) => {
          if (!cancelled) this.branches = list;
        });
        return () => {
          cancelled = true;
        };
      });

      // Fetching on its own, for whoever asked for it in Preferences.
      $effect(() => {
        const minutes = appConfigStore.gitAutoFetchMinutes;
        const root = this.root;
        if (!this.watching || !root || minutes <= 0) return;
        const timer = setInterval(() => {
          if (!this.running && !document.hidden && this.snapshot?.remotes.length) void this.run({ kind: "fetch" }, true);
        }, minutes * 60_000);
        return () => clearInterval(timer);
      });

      // Turned off in Preferences: whatever of it is open closes.
      $effect(() => {
        if (appConfigStore.loaded && !appConfigStore.gitEnabled) this.open = false;
      });
    });

    return () => {
      stopped = true;
      unlisten?.();
      stopEffects();
    };
  }

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    if (open && !appConfigStore.gitEnabled) return;
    // One side panel at a time: they share the window's left edge.
    if (open) {
      agentsStore.setOpen(false);
      explorerStore.open = false;
      harnessStore.setOpen(false);
      tabGroupsStore.open = false;
    }
    this.open = open;
  }

  /** Opens the panel with the keyboard in the commit message. */
  focusCommit(): void {
    this.setOpen(true);
    this.commitFocusNonce++;
  }

  /** Looks at the repository again. Calls made while one is under way are
   * folded into a single look right after it. */
  async refresh(): Promise<void> {
    if (this.looking) {
      this.lookAgain = true;
      return;
    }
    this.looking = true;
    try {
      do {
        this.lookAgain = false;
        const path = this.target;
        if (!path) break;
        const snapshot = await gitSnapshot(path).catch(() => null);
        // The tab changed while git answered: that answer is of another folder.
        if (path !== this.target) {
          this.lookAgain = true;
          continue;
        }
        // Most looks find nothing new: only a real change reaches the screen.
        const json = JSON.stringify(snapshot);
        if (json !== this.lastJson) {
          this.lastJson = json;
          this.snapshot = snapshot;
        }
        this.loaded = true;
      } while (this.lookAgain);
    } finally {
      this.looking = false;
    }
    if (this.root && this.root !== this.adopted) void this.adoptRunning();
  }

  /** An action another window started before this one looked. */
  private async adoptRunning(): Promise<void> {
    const root = this.root;
    if (!root) return;
    this.adopted = root;
    const busy = await gitBusy().catch((): string[] => []);
    if (busy.includes(root) && this.running === null && this.root === root) {
      this.running = { root, op: "fetch", lines: [] };
    }
  }

  private onOp(root: string, op: GitOpKind, stage: "started" | "line" | "finished", line: string | null): void {
    if (root !== this.root) return;
    if (stage === "started") {
      this.running = { root, op, lines: [] };
    } else if (stage === "line" && line && this.running) {
      const lines = [...this.running.lines, line];
      this.running = { ...this.running, op, lines: lines.slice(-MAX_LINES) };
    } else if (stage === "finished" && !this.ownRun) {
      // Another window's action: `run` isn't here to tidy up after it.
      this.running = null;
      void this.refresh();
    }
  }

  /** Runs an action on the repository in view; true when it worked. `quiet`
   * keeps a failure to itself (a fetch nobody asked for). */
  async run(op: GitOp, quiet = false): Promise<boolean> {
    const root = this.root;
    if (!root) return false;
    if (!quiet) this.error = null;
    const pushes = op.kind === "push" || op.kind === "sync";
    if (pushes && !quiet && this.beforePush) {
      if (this.running) return false;
      // The wait can be long (a review, then a question): the repository is
      // busy for as long as it lasts, so nothing else is started on it here.
      const [branch, head] = [this.branch, this.snapshot?.head?.sha ?? null];
      this.ownRun = true;
      this.holding = true;
      this.running = { root, op: op.kind, lines: [] };
      let go = false;
      try {
        go = await this.beforePush();
        await this.refresh();
      } finally {
        this.holding = false;
      }
      // What was let through is that branch as it was. Moved on since (from
      // a console), or with another repository in view, it isn't pushed.
      const same = this.root === root && this.branch === branch && (this.snapshot?.head?.sha ?? null) === head;
      if (!go || !same) {
        this.ownRun = false;
        this.running = null;
        return false;
      }
    }
    this.ownRun = true;
    this.running = { root, op: op.kind, lines: [] };
    try {
      const output = await gitRun(root, op);
      if (!quiet) this.lastOutput = output || null;
      return true;
    } catch (reason) {
      if (!quiet) this.error = String(reason);
      return false;
    } finally {
      this.ownRun = false;
      this.running = null;
      await this.refresh();
    }
  }

  cancel(): void {
    if (this.holding) this.cancelBeforePush?.();
    else if (this.running) void gitCancel(this.running.root);
  }

  /** Commits, and pushes right after when asked to. */
  async commit(message: string, options: { amend: boolean; all: boolean; push: boolean }): Promise<boolean> {
    const root = this.root;
    const done = await this.run({ kind: "commit", message, amend: options.amend, all: options.all });
    if (!done) return false;
    if (root) this.setDraft(root, "");
    return options.push ? this.run({ kind: "push" }) : true;
  }

  draft(root: string | null): string {
    return root ? (this.drafts[root] ?? "") : "";
  }

  setDraft(root: string, message: string): void {
    this.drafts = { ...this.drafts, [root]: message };
  }

  async loadPr(refresh: boolean): Promise<void> {
    const root = this.root;
    if (!root || !this.wantsPr) return;
    if (refresh) this.prBusy = true;
    try {
      const state = await gitPr(root, refresh);
      if (root === this.root) this.pr = state;
    } catch {
      // Left as it was: the next look asks again.
    } finally {
      if (refresh) this.prBusy = false;
    }
  }

  async updatePr(patch: PrPatch): Promise<boolean> {
    return this.changePr((root) => gitPrUpdate(root, patch));
  }

  async createPr(pr: NewPr): Promise<boolean> {
    const created = await this.changePr((root) => gitPrCreate(root, pr));
    if (created) this.afterPrCreate?.();
    return created;
  }

  private async changePr(change: (root: string) => Promise<PrState>): Promise<boolean> {
    const root = this.root;
    if (!root) return false;
    this.prBusy = true;
    this.prError = null;
    try {
      const state = await change(root);
      if (root === this.root) this.pr = state;
      return true;
    } catch (reason) {
      this.prError = String(reason);
      return false;
    } finally {
      this.prBusy = false;
    }
  }
}

export const gitStore = new GitStore();
