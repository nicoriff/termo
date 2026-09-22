import {
  installItem,
  installMcpItem,
  invalidateCatalog,
  listCatalog,
  marketplaceStatus,
  searchMcpRegistry,
  uninstallItem,
  type AgentTarget,
  type CatalogItem,
  type CatalogSnapshot,
  type DeclaredCommand,
  type MarketplaceStatus,
  type SourceInfo,
} from "$lib/core/marketplace-client";

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
      const page = await searchMcpRegistry(query, null);
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
      const page = await searchMcpRegistry(this.mcpQuery, this.mcpNextCursor);
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
