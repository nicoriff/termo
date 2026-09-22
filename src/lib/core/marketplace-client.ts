import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/marketplace/model.rs and catalog.rs.

export type ComponentClass = "plugin" | "skill" | "mcp";
export type AgentTarget = "claude" | "codex";

export interface PluginDetail {
  kind: "plugin";
  plugin_id: string;
  marketplace: string;
  source: unknown;
  strict: boolean | null;
}

/** One header a remote MCP server needs on every request. `value` is the
 * registry's own text — for a secret header that's a placeholder template
 * ("Bearer {api_key}"), never a working value. */
export interface McpHeaderSpec {
  name: string;
  description?: string;
  isRequired?: boolean;
  isSecret?: boolean;
  value?: string;
}

export interface McpRemote {
  type?: string;
  url: string;
  headers?: McpHeaderSpec[];
}

export interface McpEnvVarSpec {
  name: string;
  description?: string;
  isRequired?: boolean;
  isSecret?: boolean;
  default?: string;
}

export interface McpPackage {
  registryType: string;
  identifier: string;
  version?: string;
  environmentVariables?: McpEnvVarSpec[];
}

export interface McpDetail {
  kind: "mcp";
  server_name: string;
  remotes: McpRemote[];
  packages: McpPackage[];
  repository: unknown | null;
}

export type ItemDetail = PluginDetail | McpDetail;

export interface CatalogItem {
  id: string;
  class: ComponentClass;
  name: string;
  displayName: string;
  description: string;
  sourceId: string;
  sourceLabel: string;
  category: string | null;
  author: string | null;
  homepage: string | null;
  version: string | null;
  tags: string[];
  installCount: number | null;
  /** Derived from where the code is published; null means the UI draws
   * initials instead of fetching an image from somewhere unrelated. */
  iconUrl: string | null;
  /** 0-5, when a source publishes one. Neither Claude's marketplaces nor the
   * MCP registry does today. */
  rating: number | null;
  /** Skills the marketplace entry declares, when it declares any. A facet,
   * not a class: a plugin's real inventory isn't knowable before install. */
  declaresSkills: string[];
  targets: AgentTarget[];
  installedIn: AgentTarget[];
  detail: ItemDetail;
}

export interface SourceInfo {
  name: string;
  kind: string;
  location: string | null;
}

export interface CatalogSnapshot {
  items: CatalogItem[];
  sources: SourceInfo[];
  /** Machine-readable reasons the catalog may be incomplete, translated in
   * the panel: "claude-cli-missing", "claude-cli-failed". */
  warnings: string[];
  fetchedAt: number;
}

export interface MarketplaceStatus {
  claudeCli: string | null;
  codexCli: string | null;
}

export function marketplaceStatus(): Promise<MarketplaceStatus> {
  return invoke("marketplace_status");
}

export function listCatalog(refresh = false): Promise<CatalogSnapshot> {
  return invoke("list_catalog", { refresh });
}

export interface InstallRequest {
  pluginId: string;
  target: AgentTarget;
  /** The checksum of a marketplace-declared command the user approved. */
  acceptCommand?: string | null;
}

export interface DeclaredCommand {
  sha256: string;
  text: string;
}

export type InstallOutcome =
  | { status: "installed"; message: string }
  /** The marketplace declares a command that runs on this machine; it has to
   * be shown and approved before the install is retried. */
  | { status: "needs-command-approval"; commands: DeclaredCommand[] };

export function installItem(request: InstallRequest): Promise<InstallOutcome> {
  return invoke("install_item", { request });
}

export function uninstallItem(request: InstallRequest): Promise<void> {
  return invoke("uninstall_item", { request });
}

export function invalidateCatalog(): Promise<void> {
  return invoke("invalidate_catalog");
}

export interface McpPage {
  items: CatalogItem[];
  nextCursor: string | null;
}

/** One page of the MCP registry. An empty `query` browses everything;
 * `cursor` continues a previous page (from that page's `nextCursor`). */
export function searchMcpRegistry(query: string, cursor?: string | null): Promise<McpPage> {
  return invoke("search_mcp_registry", { query, cursor: cursor ?? null });
}

export interface McpInstallRequest {
  serverName: string;
  target: AgentTarget;
  remotes: McpRemote[];
  packages: McpPackage[];
  /** Header or environment-variable values the user supplied, keyed by
   * name — see marketplace/mcp-params.ts for what's collected and why. */
  params: Record<string, string>;
}

export function installMcpItem(request: McpInstallRequest): Promise<InstallOutcome> {
  return invoke("install_mcp_item", { request });
}

export function uninstallMcpItem(request: McpInstallRequest): Promise<void> {
  return invoke("uninstall_mcp_item", { request });
}
