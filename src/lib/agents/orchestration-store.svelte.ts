import {
  approveOrchestration,
  editOrchestrationPlan,
  reviseOrchestration,
  deleteOrchestration,
  listOrchestrations,
  onOrchestration,
  orchestrationCatalog,
  planOrchestration,
  resolveRevision,
  resumeOrchestration,
  stopOrchestration,
  type Catalog,
  type Orchestration,
  type Plan,
} from "$lib/core/orchestration-client";
import { agentsStore } from "./agents-store.svelte";
import { announceOrchestration } from "./orchestration-attention";

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
  private started = false;
  private catalogLoaded = false;

  async init(): Promise<void> {
    if (this.started) return;
    this.started = true;
    await onOrchestration((orchestration) => this.take(orchestration));
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

  async plan(prompt: string, workDir: string, model: string): Promise<Orchestration> {
    this.planning = true;
    try {
      const orchestration = await planOrchestration(prompt, workDir, model);
      this.take(orchestration);
      return orchestration;
    } finally {
      this.planning = false;
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
    const revised = await reviseOrchestration(id, $state.snapshot(plan), instructions);
    this.take(revised.orchestration);
    return { plan: revised.plan, note: revised.note };
  }

  stop(id: string): Promise<void> {
    return stopOrchestration(id);
  }

  async resume(id: string, budgetUsd: number | null): Promise<void> {
    this.take(await resumeOrchestration(id, budgetUsd));
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
