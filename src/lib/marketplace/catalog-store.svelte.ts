import {
  installItem,
  installMcpItem,
  invalidateCatalog,
  listCatalog,
  marketplaceSources,
  marketplaceStatus,
  mcpDetail,
  searchMcp,
  uninstallItem,
  uninstallMcpItem,
  type AgentTarget,
  type CatalogItem,
  type CatalogSnapshot,
  type DeclaredCommand,
  type MarketplaceStatus,
  type SourceId,
  type SourceInfo,
  type SourceStatus,
} from "$lib/core/marketplace-client";
import { appConfigStore } from "$lib/core/app-config.svelte";

/**
 * The marketplace catalog, loaded once per session and kept here so
 * reopening the panel is instant. The Rust side caches it too (in memory and
 * on disk with a TTL), so a "load" that finds nothing stale costs one IPC
 * round trip and no network at all.
 */
class CatalogStore {
  items = $state<CatalogItem[]>([]);
  sources = $state<SourceInfo[]>([]);
  warnings = $state<string[]>([]);
  fetchedAt = $state(0);
  loading = $state(false);
  error = $state<string | null>(null);
  status = $state<MarketplaceStatus | null>(null);

  /** Whether installing is possible at all: without the CLI the panel is
   * browse-only (see marketplace/agent_cli.rs). */
  get claudeCliAvailable(): boolean {
    return Boolean(this.status?.claudeCli);
  }

  private inFlight: Promise<void> | null = null;

  /** Loads the catalog unless it's already here. `force` re-fetches, which
   * is what the refresh button and any source change do. */
  load(force = false): Promise<void> {
    if (this.inFlight) return this.inFlight;
    if (!force && this.items.length > 0) return Promise.resolve();

    this.loading = true;
    this.error = null;
    this.inFlight = (async () => {
      try {
        const [snapshot, status] = await Promise.all([listCatalog(force), marketplaceStatus()]);
        this.apply(snapshot);
        this.status = status;
      } catch (err) {
        this.error = err instanceof Error ? err.message : String(err);
      } finally {
        this.loading = false;
        this.inFlight = null;
      }
    })();
    return this.inFlight;
  }

  /** Which item is being installed or removed right now, so only its own
   * button goes busy and the rest of the panel stays usable. */
  busyId = $state<string | null>(null);
  installError = $state<string | null>(null);
  /** Set when the marketplace declares a command that has to be approved
   * before the install can go through. Never acted on automatically. */
  pendingApproval = $state<{ item: CatalogItem; commands: DeclaredCommand[] } | null>(null);

  /** Installs, and returns whether it went through — false means an error or
   * a pending approval, both of which the panel reads off this store. */
  async install(item: CatalogItem, target: AgentTarget, acceptCommand?: string): Promise<boolean> {
    if (item.detail.kind !== "plugin") return false;
    this.busyId = item.id;
    this.installError = null;
    try {
      const outcome = await installItem({
        pluginId: item.detail.plugin_id,
        target,
        acceptCommand: acceptCommand ?? null,
      });
      if (outcome.status === "needs-command-approval") {
        this.pendingApproval = { item, commands: outcome.commands };
        return false;
      }
      this.pendingApproval = null;
      await this.refreshAfterChange();
      return true;
    } catch (err) {
      this.installError = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      this.busyId = null;
    }
  }

  async uninstall(item: CatalogItem, target: AgentTarget): Promise<boolean> {
    if (item.detail.kind !== "plugin") return false;
    this.busyId = item.id;
    this.installError = null;
    try {
      await uninstallItem({ pluginId: item.detail.plugin_id, target });
      await this.refreshAfterChange();
      return true;
    } catch (err) {
      this.installError = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      this.busyId = null;
    }
  }

  /** The CLI just changed what's installed, so the cached catalog is stale. */
  private async refreshAfterChange(): Promise<void> {
    await invalidateCatalog();
    await this.load(true);
  }

  // --- MCP registry: unlike the Claude catalog, this one is network-bound
  // and unbounded, so it's searched fresh on every query instead of being
  // downloaded once and filtered locally.

  mcpItems = $state<CatalogItem[]>([]);
  mcpLoading = $state(false);
  mcpError = $state<string | null>(null);
  private mcpNextCursor = $state<string | null>(null);
  private mcpQuery = "";
  /** Where MCP servers are searched now (see marketplace/sources.rs), and
   * what's known about each source. Starts on the user's default. */
  mcpSource = $state<SourceId>(appConfigStore.marketplaceSource);
  mcpSources = $state<SourceStatus[]>([]);
  /** The server whose details are being fetched, and what went wrong. */
  detailLoadingId = $state<string | null>(null);
  detailError = $state<string | null>(null);
  /** The last MCP install: under which name, into which agent, and whether
   * the user still has to sign in from there. */
  lastInstall = $state<{ itemId: string; target: AgentTarget; installName: string; signIn: boolean } | null>(null);

  /** Until the user picks a source here, it's the one set in Preferences —
   * read as the panel opens, since the settings load after this store. */
  private sourcePicked = false;

  useDefaultSource(): void {
    if (!this.sourcePicked) this.mcpSource = appConfigStore.marketplaceSource;
  }

  async loadSources(): Promise<void> {
    this.mcpSources = await marketplaceSources().catch(() => this.mcpSources);
  }

  /** Searches somewhere else: the same query, from the first page. */
  setSource(source: SourceId): void {
    this.sourcePicked = true;
    if (source === this.mcpSource) return;
    this.mcpSource = source;
    appConfigStore.setMarketplaceSource(source);
    void this.searchMcp(this.mcpQuery);
  }

  /** Fetches how to connect to a server whose result didn't say (and what
   * it brings), and puts it in the list in the result's place. */
  async loadDetail(item: CatalogItem): Promise<void> {
    if (!item.extra?.needsDetail || item.extra.source === "" || this.detailLoadingId === item.id) return;
    this.detailLoadingId = item.id;
    this.detailError = null;
    try {
      const full = await mcpDetail(item.extra.source, item.name);
      this.mcpItems = this.mcpItems.map((other) => (other.id === item.id ? full : other));
    } catch (err) {
      this.detailError = err instanceof Error ? err.message : String(err);
    } finally {
      if (this.detailLoadingId === item.id) this.detailLoadingId = null;
    }
  }

  private setInstalled(item: CatalogItem, target: AgentTarget, installed: boolean): void {
    this.mcpItems = this.mcpItems.map((other) => {
      if (other.id !== item.id) return other;
      const rest = other.installedIn.filter((agent) => agent !== target);
      return { ...other, installedIn: installed ? [...rest, target] : rest };
    });
  }
  /** Bumped on every new search so a slow response that lost the race to a
   * newer one gets discarded instead of overwriting fresher results. */
  private mcpToken = 0;

  get mcpHasMore(): boolean {
    return this.mcpNextCursor !== null;
  }

  /** Replaces the MCP results with a fresh first page for `query`. */
  async searchMcp(query: string): Promise<void> {
    const token = ++this.mcpToken;
    this.mcpQuery = query;
    this.mcpLoading = true;
    this.mcpError = null;
    try {
      const page = await searchMcp(this.mcpSource, query, null);
      if (token !== this.mcpToken) return;
      this.mcpItems = page.items;
      this.mcpNextCursor = page.nextCursor;
    } catch (err) {
      if (token !== this.mcpToken) return;
      this.mcpError = err instanceof Error ? err.message : String(err);
      this.mcpItems = [];
      this.mcpNextCursor = null;
    } finally {
      if (token === this.mcpToken) this.mcpLoading = false;
    }
  }

  /** Appends the next page for the current query. */
  async loadMoreMcp(): Promise<void> {
    if (!this.mcpNextCursor || this.mcpLoading) return;
    const token = this.mcpToken;
    this.mcpLoading = true;
    try {
      const page = await searchMcp(this.mcpSource, this.mcpQuery, this.mcpNextCursor);
      if (token !== this.mcpToken) return;
      this.mcpItems = [...this.mcpItems, ...page.items];
      this.mcpNextCursor = page.nextCursor;
    } catch (err) {
      if (token !== this.mcpToken) return;
      this.mcpError = err instanceof Error ? err.message : String(err);
    } finally {
      if (token === this.mcpToken) this.mcpLoading = false;
    }
  }

  /** Installs an MCP server, filling in whatever header/env values the user
   * supplied (see marketplace/mcp-params.ts). There is no equivalent to a
   * marketplace-declared command here, but a needs-approval reply is still
   * handled rather than ignored, for forward safety. */
  async installMcp(item: CatalogItem, target: AgentTarget, params: Record<string, string>): Promise<boolean> {
    if (item.detail.kind !== "mcp") return false;
    this.busyId = item.id;
    this.installError = null;
    try {
      const outcome = await installMcpItem({
        serverName: item.detail.server_name,
        target,
        remotes: item.detail.remotes,
        packages: item.detail.packages,
        params,
      });
      if (outcome.status === "needs-command-approval") {
        this.pendingApproval = { item, commands: outcome.commands };
        return false;
      }
      this.setInstalled(item, target, true);
      this.lastInstall = { itemId: item.id, target, installName: outcome.installName, signIn: outcome.signIn };
      return true;
    } catch (err) {
      this.installError = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      this.busyId = null;
    }
  }

  /** Takes an MCP server out of one agent. */
  async uninstallMcp(item: CatalogItem, target: AgentTarget): Promise<boolean> {
    if (item.detail.kind !== "mcp") return false;
    this.busyId = item.id;
    this.installError = null;
    try {
      await uninstallMcpItem({
        serverName: item.detail.server_name,
        target,
        remotes: item.detail.remotes,
        packages: item.detail.packages,
        params: {},
      });
      this.setInstalled(item, target, false);
      if (this.lastInstall?.itemId === item.id && this.lastInstall.target === target) this.lastInstall = null;
      return true;
    } catch (err) {
      this.installError = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      this.busyId = null;
    }
  }

  private apply(snapshot: CatalogSnapshot): void {
    this.items = snapshot.items;
    this.sources = snapshot.sources;
    this.warnings = snapshot.warnings;
    this.fetchedAt = snapshot.fetchedAt;
  }
}

export const catalogStore = new CatalogStore();
