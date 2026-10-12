import {
  gitReviewBusy,
  gitReviewCancel,
  gitReviewClear,
  gitReviewComment,
  gitReviewDismiss,
  gitReviewFix,
  gitReviewLoad,
  gitReviewPublish,
  gitReviewSetNote,
  gitReviewStart,
  onGitReviewStep,
  type Review,
  type ReviewFinding,
  type ReviewScope,
  type StoredReviews,
} from "$lib/core/git-client";
import type { PlanStep } from "$lib/core/orchestration-client";
import { appConfigStore } from "$lib/core/app-config.svelte";
import { writePty } from "$lib/core/pty-client";
import { uiState } from "$lib/core/ui-state.svelte";
import { aiEngine } from "$lib/agents/ai-engine.svelte";
import { agentsStore } from "$lib/agents/agents-store.svelte";
import { aiStatusStore } from "$lib/ai-mode/ai-status-store.svelte";
import { isConsoleTab } from "$lib/layout/split-tree";
import { workspaceStore } from "$lib/layout/workspace-store.svelte";
import { getPtyId } from "$lib/terminal/pty-registry";
import type { MessageKey } from "$lib/i18n/translate";
import { gitStore } from "./git-store.svelte";
import { asPaste, blockingFindings, filesWithFindings, fixRequest, openFindings, reviewOf, reviewsOf } from "./git-review";

/** How many of a review's steps are kept to show. */
const MAX_STEPS = 300;

/** The review under way on the repository in view. */
export interface RunningReview {
  root: string;
  /** Null for one another window started: only that it's there is known. */
  scope: ReviewScope | null;
  startedAt: number;
  steps: PlanStep[];
}

/** Something to say after an action, as a catalog key. */
export interface ReviewNotice {
  key: MessageKey;
  params?: Record<string, string>;
}

/** A push held up for the user to decide on: what a review of the branch
 * found, or why there's no review to go by. */
export type PushGate = ({ kind: "findings"; count: number } | { kind: "failed"; message: string }) & {
  /** The repository and branch whose push it is. */
  root: string;
  branch: string;
};

/**
 * The code reviews of the repository of the tab in focus (see
 * src-tauri/src/git/review.rs): the ones kept, the one under way, and what's
 * done with a finding — dismissed, handed to an agent, sent to a console,
 * left on the pull request.
 *
 * A review is one question to an agent CLI that may only look. It isn't a
 * run of an agent: nothing of it shows in the agents' panel. Fixing is.
 */
class GitReviewStore {
  stored = $state<StoredReviews>({ note: "", reviews: [] });
  running = $state<RunningReview | null>(null);
  /** Why the last review or action failed, in words ready to show. */
  error = $state<string | null>(null);
  notice = $state<ReviewNotice | null>(null);
  gate = $state<PushGate | null>(null);
  /** A finding being handed over or published. */
  acting = $state(false);

  /** Reviews are there to ask for: Git, the reviews, and Modo IA are on. */
  enabled = $derived(appConfigStore.gitEnabled && appConfigStore.gitReviewEnabled);
  available = $derived(this.enabled && appConfigStore.aiModeEnabled);
  /** The ones made on the branch in view, newest first. */
  reviews = $derived(reviewsOf(this.stored.reviews, gitStore.branch));
  /** The files of the working tree with a finding still open. */
  marks = $derived(filesWithFindings(this.stored.reviews, gitStore.branch));
  /** How many findings of the branch's reviews are still open. */
  open = $derived(openFindings(this.reviews));
  busy = $derived(this.running !== null && this.running.root === gitStore.root);

  /** The agents of the user's that fix: the ones of their own (not an
   * orchestration's team) made for it, which have to be able to change
   * files. */
  fixers = $derived(agentsStore.localAgents.filter((agent) => agent.gitRole === "fixer" && agent.autonomy !== "read"));
  /** The one picked in Preferences, while it's still there and still can. */
  fixer = $derived(this.fixers.find((agent) => agent.id === appConfigStore.gitReviewFixAgentId) ?? null);
  /** The console in focus, when an agent is working in it: where a finding
   * can be sent for that agent to fix. */
  console = $derived.by(() => {
    const session = workspaceStore.focusedSession;
    if (!session || !isConsoleTab(session)) return null;
    const tool = aiStatusStore.byId[session.id]?.tool ?? null;
    return tool ? { tabId: session.id, tool } : null;
  });

  private loadedOf = "";
  private gateAnswer: ((go: boolean) => void) | null = null;
  /** The push being held was given up on. */
  private dropped = false;

  /** Starts following the repository in view. The main window calls it
   * once; returns what stops it. */
  start(): () => void {
    let unlisten: (() => void) | null = null;
    let stopped = false;
    void onGitReviewStep((event) => {
      if (this.running?.root !== event.root) return;
      this.running = { ...this.running, steps: [...this.running.steps, event.step].slice(-MAX_STEPS) };
    }).then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    });
    gitStore.beforePush = () => this.beforePush();
    gitStore.cancelBeforePush = () => {
      this.dropped = true;
      this.cancel();
      this.answerGate(false);
    };
    gitStore.afterPrCreate = () => {
      if (this.available && appConfigStore.gitReviewOnPrCreate) void this.review({ kind: "branch" });
    };

    const stopEffects = $effect.root(() => {
      // Read again whenever the repository changes: another branch has other
      // reviews, and a changed file makes its findings stale.
      $effect(() => {
        const root = gitStore.root;
        void gitStore.snapshot;
        if (!this.enabled || !root) {
          this.stored = { note: "", reviews: [] };
          this.loadedOf = "";
          return;
        }
        if (!gitStore.open && this.loadedOf === root) return;
        void this.load();
      });

      // A held push is of one branch of one repository: with another in
      // view, "push anyway" would be about something else. It isn't pushed.
      $effect(() => {
        const gate = this.gate;
        if (gate && (gitStore.root !== gate.root || gitStore.branch !== gate.branch)) this.answerGate(false);
      });
    });

    return () => {
      stopped = true;
      unlisten?.();
      stopEffects();
      gitStore.beforePush = null;
      gitStore.cancelBeforePush = null;
      gitStore.afterPrCreate = null;
    };
  }

  async load(): Promise<void> {
    const root = gitStore.root;
    if (!root) return;
    const [stored, busy] = await Promise.all([
      gitReviewLoad(root).catch(() => null),
      gitReviewBusy().catch((): string[] => []),
    ]);
    if (root !== gitStore.root) return;
    if (stored) this.stored = stored;
    this.loadedOf = root;
    // One another window started, or one of its that has ended since.
    if (busy.includes(root) && !this.running) this.running = { root, scope: null, startedAt: Date.now(), steps: [] };
    else if (!busy.includes(root) && this.running?.root === root && this.running.scope === null) this.running = null;
  }

  /** Reviews `scope` of the repository in view, to the end. The review, or
   * null when it couldn't be made (`error` says why). */
  async review(scope: ReviewScope): Promise<Review | null> {
    const root = gitStore.root;
    if (!root || !this.available || this.running) return null;
    // One of the user's own agents made to review, while it's still there
    // and still one: otherwise the built-in reviewer.
    const picked = appConfigStore.gitReviewAgentId.trim();
    const agentId = agentsStore.localAgents.some((agent) => agent.id === picked && agent.gitRole === "reviewer") ? picked : null;
    const engine = aiEngine.value;
    this.error = null;
    this.notice = null;
    if (!engine && !agentId) {
      this.notice = { key: "git.review.noEngine" };
      return null;
    }
    this.running = { root, scope, startedAt: Date.now(), steps: [] };
    try {
      const pr = gitStore.pr?.kind === "ok" ? gitStore.pr.pr : null;
      return await gitReviewStart(root, {
        scope,
        engine: engine ?? "claude",
        agentId,
        base: pr?.base || null,
        instructions: appConfigStore.gitReviewInstructions,
        timeoutMin: appConfigStore.gitReviewTimeoutMin,
      });
    } catch (reason) {
      this.error = String(reason);
      return null;
    } finally {
      this.running = null;
      await this.load();
    }
  }

  /** A review asked for away from the panel (the status bar's menu, the
   * palette, a commit in the graph): the panel opens to show it. */
  ask(scope: ReviewScope): void {
    gitStore.setOpen(true);
    void this.review(scope);
  }

  cancel(): void {
    if (this.running) void gitReviewCancel(this.running.root);
  }

  private async change(action: (root: string) => Promise<StoredReviews>): Promise<void> {
    const root = gitStore.root;
    if (!root) return;
    try {
      const stored = await action(root);
      if (root === gitStore.root) this.stored = stored;
    } catch (reason) {
      this.error = String(reason);
    }
  }

  dismiss(review: Review, finding: ReviewFinding, dismissed: boolean): Promise<void> {
    return this.change((root) => gitReviewDismiss(root, review.id, finding.id, dismissed));
  }

  clear(review: Review): Promise<void> {
    return this.change((root) => gitReviewClear(root, review.id));
  }

  setNote(note: string): Promise<void> {
    return this.change((root) => gitReviewSetNote(root, note));
  }

  /** Hands a finding to one of the user's agents: a run of that agent, in
   * the repository, that leaves its change uncommitted. */
  async fix(review: Review, finding: ReviewFinding, agentId: string): Promise<boolean> {
    const root = gitStore.root;
    if (!root || this.acting) return false;
    this.acting = true;
    this.error = null;
    try {
      await gitReviewFix(root, review.id, finding.id, agentId);
      const name = agentsStore.agents.find((agent) => agent.id === agentId)?.name ?? "";
      this.notice = { key: "git.review.fixStarted", params: { agent: name } };
      return true;
    } catch (reason) {
      this.error = String(reason);
      return false;
    } finally {
      this.acting = false;
    }
  }

  /** Puts a finding in the prompt of the agent working in the console in
   * focus, for the user to send. `lead` is the request, in their language. */
  sendToConsole(finding: ReviewFinding, lead: string): boolean {
    const ptyId = this.console ? getPtyId(this.console.tabId) : undefined;
    if (!ptyId) return false;
    void writePty(ptyId, asPaste(fixRequest(finding, lead)));
    uiState.refocusTerminals();
    return true;
  }

  /** The comment publishing would leave, to show before it does. */
  comment(review: Review, findingIds: string[]): Promise<string> {
    return gitReviewComment(review.root, review.id, findingIds);
  }

  /** Leaves the findings picked as a comment on the branch's pull request. */
  async publish(review: Review, findingIds: string[]): Promise<boolean> {
    if (this.acting || findingIds.length === 0) return false;
    this.acting = true;
    this.error = null;
    try {
      await gitReviewPublish(review.root, review.id, findingIds);
      this.notice = { key: "git.review.published" };
      void gitStore.loadPr(true);
      return true;
    } catch (reason) {
      this.error = String(reason);
      return false;
    } finally {
      this.acting = false;
    }
  }

  /** The user's answer to a push that was held up. */
  answerGate(go: boolean): void {
    this.gate = null;
    this.gateAnswer?.(go);
    this.gateAnswer = null;
  }

  private holdPush(gate: PushGate): Promise<boolean> {
    // The question is asked in the panel.
    gitStore.setOpen(true);
    this.gateAnswer?.(false);
    this.gate = gate;
    return new Promise((resolve) => (this.gateAnswer = resolve));
  }

  /** Before a push from Termo, for whoever asked for it in Preferences: the
   * branch is reviewed unless it already was as it is, and with anything
   * serious found the user decides. True lets the push go. */
  private async beforePush(): Promise<boolean> {
    if (!this.available || appConfigStore.gitReviewBeforePush !== "ask") return true;
    const snapshot = gitStore.snapshot;
    // Nothing of its own to review: a detached HEAD, or level with its base.
    if (!snapshot?.branch || !snapshot.head || !snapshot.base || snapshot.base.ahead === 0) return true;
    const of = { root: snapshot.root, branch: snapshot.branch };
    this.dropped = false;
    if (this.running) return this.holdPush({ kind: "failed", message: "", ...of });
    await this.load();
    let review = reviewOf(this.stored.reviews, snapshot.branch, { kind: "branch" });
    if (!review || review.head !== snapshot.head.sha) {
      gitStore.setOpen(true);
      review = await this.review({ kind: "branch" });
      if (this.dropped) return false;
      if (!review) return this.holdPush({ kind: "failed", message: this.error ?? "", ...of });
    }
    const held = blockingFindings(review.findings);
    return held.length === 0 ? true : this.holdPush({ kind: "findings", count: held.length, ...of });
  }
}

export const gitReviewStore = new GitReviewStore();
