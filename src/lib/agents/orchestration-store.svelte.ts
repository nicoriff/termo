import {
  approveOrchestration,
  editOrchestrationPlan,
  reviseOrchestration,
  answerOrchestration,
  clarifyOrchestration,
  writeOrchestrationReport,
  deleteOrchestration,
  listOrchestrations,
  onOrchestration,
  onOrchestrationProgress,
  orchestrationCatalog,
  planOrchestration,
  resolveRevision,
  resumeOrchestration,
  stopOrchestration,
  type Answered,
  type Catalog,
  type Clarifying,
  type PlanStep,
  type Orchestration,
  type Plan,
} from "$lib/core/orchestration-client";
import type { AgentEngine } from "$lib/core/agents-client";
import { agentsStore } from "./agents-store.svelte";
import { announceOrchestration } from "./orchestration-attention";
import { pendingQuestions } from "./orchestration-spec";

/** The orchestrations and what each is doing, kept by the backend
 * (src-tauri/src/agents/orchestration). Every change arrives through the
 * `orchestration` event, so the state here follows one that's running. */
class OrchestrationStore {
  /** Newest first. */
  all = $state<Orchestration[]>([]);
  /** What a team can be made of on this computer; loaded on first use. */
  catalog = $state<Catalog>([]);
  /** A plan is being drafted (it takes a minute or two). */
  planning = $state(false);
  /** While it is: which part, since when, and the steps the orchestrator
   * has taken (see PlanningProgress). */
  planningPhase = $state<"clarify" | "plan" | null>(null);
  planningStartedAt = $state(0);
  planningSteps = $state<PlanStep[]>([]);
  /** The steps of a revision being asked for, by orchestration. */
  revisingSteps = $state<Record<string, PlanStep[]>>({});
  private started = false;
  private catalogLoaded = false;

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    await onOrchestration((orchestration) => this.take(orchestration));
    await onOrchestrationProgress((progress) => {
      if (progress.id) {
        const steps = this.revisingSteps[progress.id] ?? [];
        this.revisingSteps = { ...this.revisingSteps, [progress.id]: [...steps, progress.step].slice(-40) };
      } else if (this.planningPhase === progress.phase) {
        this.planningSteps = [...this.planningSteps, progress.step].slice(-80);
      }
    });
    this.all = await listOrchestrations().catch(() => []);
  }

  async loadCatalog(): Promise<void> {
    if (this.catalogLoaded) return;
    this.catalogLoaded = true;
    this.catalog = await orchestrationCatalog().catch(() => {
      this.catalogLoaded = false;
      return [];
    });
  }

  get(id: string): Orchestration | undefined {
    return this.all.find((orchestration) => orchestration.id === id);
  }

  private take(orchestration: Orchestration): void {
    const before = this.get(orchestration.id);
    const known = before !== undefined;
    void announceOrchestration(before, orchestration);
    this.all = known
      ? this.all.map((other) => (other.id === orchestration.id ? orchestration : other))
      : [orchestration, ...this.all];
    // Its team is saved by the backend: agents this window hasn't seen yet
    // (a plan just approved, an agent the orchestrator added).
    const ids = orchestration.plan.agents.map((agent) => agent.agentId).filter((id) => id !== "");
    if (ids.some((id) => !agentsStore.agents.some((agent) => agent.id === id))) void agentsStore.reload();
  }

  /** What the planner would ask first; planning shows as going on meanwhile. */
  async clarify(prompt: string, workDir: string, engine: AgentEngine, model: string): Promise<Clarifying> {
    this.planning = true;
    this.startPhase("clarify");
    try {
      return await clarifyOrchestration(prompt, workDir, engine, model);
    } finally {
      this.planning = false;
      this.planningPhase = null;
    }
  }

  private startPhase(phase: "clarify" | "plan") {
    this.planningPhase = phase;
    this.planningSteps = [];
    this.planningStartedAt = Date.now();
  }

  async plan(
    prompt: string,
    workDir: string,
    engine: AgentEngine,
    model: string,
    answers: Answered[] = [],
    overheadUsd = 0,
  ): Promise<Orchestration> {
    this.planning = true;
    this.startPhase("plan");
    try {
      const orchestration = await planOrchestration(prompt, workDir, engine, model, answers, overheadUsd);
      this.take(orchestration);
      return orchestration;
    } finally {
      this.planning = false;
      this.planningPhase = null;
    }
  }

  async approve(id: string, chosen: string, plan: Plan, budgetUsd: number): Promise<void> {
    this.take(await approveOrchestration(id, chosen, $state.snapshot(plan), budgetUsd));
  }

  async editPlan(id: string, plan: Plan): Promise<void> {
    this.take(await editOrchestrationPlan(id, $state.snapshot(plan)));
  }

  /** The plan with the user's changes worked in by the orchestrator, and its
   * note on them; nothing of it is saved yet. */
  async revise(id: string, plan: Plan, instructions: string): Promise<{ plan: Plan; note: string }> {
    this.revisingSteps = { ...this.revisingSteps, [id]: [] };
    const revised = await reviseOrchestration(id, $state.snapshot(plan), instructions).finally(() => {
      const { [id]: _, ...rest } = this.revisingSteps;
      this.revisingSteps = rest;
    });
    this.take(revised.orchestration);
    return { plan: revised.plan, note: revised.note };
  }

  stop(id: string): Promise<void> {
    return stopOrchestration(id);
  }

  async resume(id: string, budgetUsd: number | null): Promise<void> {
    this.take(await resumeOrchestration(id, budgetUsd));
  }

  async answer(id: string, questionId: string, answer: string): Promise<void> {
    this.take(await answerOrchestration(id, questionId, answer));
  }

  /** Questions waiting for the user, across every orchestration. */
  get waitingQuestions(): number {
    return this.all.reduce((count, item) => count + pendingQuestions(item).length, 0);
  }

  async report(id: string): Promise<void> {
    this.take(await writeOrchestrationReport(id));
  }

  async resolve(id: string, revisionId: string, approve: boolean): Promise<void> {
    this.take(await resolveRevision(id, revisionId, approve));
  }

  async remove(id: string): Promise<void> {
    this.all = await deleteOrchestration(id);
    await agentsStore.reload();
  }
}

export const orchestrationStore = new OrchestrationStore();
