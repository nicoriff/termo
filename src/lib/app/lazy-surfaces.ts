import { lazy } from "$lib/core/lazy";

/** The window's panels and dialogs, loaded the first time each shows rather
 * than with the bundle every window parses before it can start. A panel left
 * open from the last run loads before the window shows (see startup.ts). */
export const loadAgentsSidebar = lazy(() => import("$lib/agents/AgentsSidebar.svelte"));
export const loadExplorerPanel = lazy(() => import("$lib/explorer/ExplorerPanel.svelte"));
export const loadGitPanel = lazy(() => import("$lib/git/GitPanel.svelte"));
export const loadHarnessPanel = lazy(() => import("$lib/harness/HarnessPanel.svelte"));
export const loadTabGroupsPanel = lazy(() => import("$lib/tab-groups/TabGroupsPanel.svelte"));
export const loadDiagnosticsPanel = lazy(() => import("$lib/diagnostics/DiagnosticsPanel.svelte"));
export const loadCompanionPanel = lazy(() => import("$lib/companion/CompanionPanel.svelte"));
export const loadQuickAsk = lazy(() => import("$lib/companion/QuickAsk.svelte"));
export const loadTabGroupEditor = lazy(() => import("$lib/tab-groups/TabGroupEditor.svelte"));
export const loadSaveLayoutDialog = lazy(() => import("$lib/layout/SaveLayoutDialog.svelte"));
export const loadFavoritesManagerDialog = lazy(() => import("$lib/ai-mode/FavoritesManagerDialog.svelte"));
export const loadFavoriteDialog = lazy(() => import("$lib/ai-mode/FavoriteDialog.svelte"));
export const loadLayoutManagerDialog = lazy(() => import("$lib/layout/LayoutManagerDialog.svelte"));
export const loadAboutDialog = lazy(() => import("$lib/app/AboutDialog.svelte"));
export const loadUpdateDialog = lazy(() => import("$lib/app/UpdateDialog.svelte"));
export const loadWhatsNewDialog = lazy(() => import("$lib/app/WhatsNewDialog.svelte"));
export const loadOnboardingWizard = lazy(() => import("$lib/onboarding/OnboardingWizard.svelte"));
export const loadAgentLaunchDialog = lazy(() => import("$lib/agent-launch/AgentLaunchDialog.svelte"));
export const loadCloseFileDialog = lazy(() => import("$lib/editor/CloseFileDialog.svelte"));
export const loadAgentWizard = lazy(() => import("$lib/agents/AgentWizard.svelte"));
export const loadProjectDialog = lazy(() => import("$lib/agents/ProjectDialog.svelte"));
export const loadOrchestrationDialog = lazy(() => import("$lib/agents/OrchestrationDialog.svelte"));
export const loadRestoreLayoutDialog = lazy(() => import("$lib/layout/RestoreLayoutDialog.svelte"));
export const loadRemoteConnectDialog = lazy(() => import("$lib/remote/RemoteConnectDialog.svelte"));
export const loadDeleteLayoutDialog = lazy(() => import("$lib/layout/DeleteLayoutDialog.svelte"));
export const loadMissingAgentsDialog = lazy(() => import("$lib/agent-install/MissingAgentsDialog.svelte"));
export const loadQuitDialog = lazy(() => import("$lib/app/QuitDialog.svelte"));
export const loadMarketplacePanel = lazy(() => import("$lib/marketplace/MarketplacePanel.svelte"));
