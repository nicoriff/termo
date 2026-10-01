import { invoke } from "@tauri-apps/api/core";

/** One of the month's limited Copilot quotas (see src-tauri/src/copilot_quota.rs). */
export interface CopilotQuota {
  id: "premium_interactions" | "chat";
  percentUsed: number;
  remaining: number;
  entitlement: number;
}

export type CopilotPlanUsage =
  | { kind: "ok"; plan: string | null; quotas: CopilotQuota[]; resetsOn: string | null }
  /** No GitHub CLI signed in to ask GitHub with. */
  | { kind: "noGh" }
  | { kind: "failed" };

/** Copilot's plan usage, from GitHub; the backend keeps it a few minutes. */
export function getCopilotPlanUsage(): Promise<CopilotPlanUsage> {
  return invoke("copilot_plan_usage");
}
