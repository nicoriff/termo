import { invoke } from "@tauri-apps/api/core";

// Mirrors src-tauri/src/marketplace/model.rs and catalog.rs.

export type ComponentClass = "plugin" | "skill" | "mcp";
export type AgentTarget = "claude" | "codex" | "gemini" | "copilot" | "grok";

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
  /** What goes in the address rather than in a header (Smithery's servers
   * say which is which). Same shape as a header's spec. */
  queryParams?: McpEnvVarSpec[];
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

/** Where MCP servers are searched (see marketplace/sources.rs). */
export type SourceId = "official" | "smithery";

export interface ToolInfo {
  name: string;
  description: string;
}

/** What only an MCP server has, and only some sources say. */
export interface McpExtra {
  /** The name it's installed under in every agent. */
  installName: string;
  source: SourceId | "";
  /** Its page on the source's site. */
  listingUrl: string | null;
  verified: boolean | null;
  remote: boolean | null;
  /** Whether the source's own security scan passed, when it ran one. */
  securityScan: boolean | null;
  tools: ToolInfo[];
  /** How to connect isn't in a search result: ask for the details first. */
  needsDetail: boolean;
  /** It only runs here through its source's own runner. */
  localOnly: boolean;
  /** It signs the user in itself, from the agent. */
  signIn: boolean;
}

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
  extra: McpExtra;
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
  geminiCli: string | null;
  copilotCli: string | null;
  grokCli: string | null;
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
  | { status: "installed"; message: string; installName: string; signIn: boolean }
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

/** One page of MCP servers from `source`; `cursor` continues a previous
 * page. Each result says which agents already have it. */
export function searchMcp(source: SourceId, query: string, cursor?: string | null): Promise<McpPage> {
  return invoke("search_mcp", { source, query, cursor: cursor ?? null });
}

/** A server's details, from a source whose results don't carry them. */
export function mcpDetail(source: SourceId, name: string): Promise<CatalogItem> {
  return invoke("mcp_detail", { source, name });
}

export interface SourceStatus {
  id: SourceId;
  /** It can be given an API key (optional: it searches without one). */
  takesKey: boolean;
  hasKey: boolean;
}

export function marketplaceSources(): Promise<SourceStatus[]> {
  return invoke("marketplace_sources");
}

/** Saves the key in the system keychain, once it's seen to work. */
export function setSourceKey(source: SourceId, key: string): Promise<SourceStatus> {
  return invoke("marketplace_set_key", { source, key });
}

export function clearSourceKey(source: SourceId): Promise<SourceStatus> {
  return invoke("marketplace_clear_key", { source });
}

/** Whether the saved key still works. */
export function testSourceKey(source: SourceId): Promise<void> {
  return invoke("marketplace_test_key", { source });
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
