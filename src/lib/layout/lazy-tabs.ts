import { lazy } from "$lib/core/lazy";

/** The tabs that aren't consoles, loaded when one opens. One restored from
 * the last run loads before the window shows (see app/startup.ts). */
export const loadGitGraphView = lazy(() => import("$lib/git/GitGraphView.svelte"));
export const loadAgentRunView = lazy(() => import("$lib/agents/AgentRunView.svelte"));
export const loadProjectView = lazy(() => import("$lib/agents/ProjectView.svelte"));
export const loadOrchestrationView = lazy(() => import("$lib/agents/OrchestrationView.svelte"));
export const loadOrchestrationConsole = lazy(() => import("$lib/agents/OrchestrationConsole.svelte"));
