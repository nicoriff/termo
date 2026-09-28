import type { CatalogItem, McpDetail } from "$lib/core/marketplace-client";

/** Who publishes a registry entry: the owner of its GitHub repository, else
 * the namespace it's published under ("io.github.someone/x" → someone,
 * "ai.smithery/x" → smithery). */
export function publisherOf(item: CatalogItem): string | null {
  if (item.author) return item.author;
  const detail = item.detail as McpDetail;
  const namespace = detail.server_name?.split("/")[0] ?? "";
  const parts = namespace.split(".").filter(Boolean);
  if (parts[0] === "io" && parts[1] === "github" && parts[2]) return parts[2];
  return parts.length >= 2 ? parts[parts.length - 1] : (parts[0] ?? null);
}

/** What gets installed, mirroring install.rs: a remote server when there's
 * one (nothing to download), else the first package. */
export function installSource(
  detail: McpDetail,
): { kind: "remote" } | { kind: "package"; registryType: string; identifier: string } | null {
  if (detail.remotes.length > 0) return { kind: "remote" };
  const pkg = detail.packages[0];
  return pkg ? { kind: "package", registryType: pkg.registryType, identifier: pkg.identifier } : null;
}

/** "12 mil" / "1,2 M"-style counts, in the user's locale. */
export function formatCount(count: number, locale?: string): string {
  return new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 1 }).format(count);
}

/** An MCP server's config as pasted from its README. */
export interface PastedServer {
  /** Its key in the pasted JSON, when it came under one. */
  name: string | null;
  kind: "stdio" | "http" | "sse";
  command: string;
  args: string[];
  url: string;
  /** Environment variables (a command) or headers (a URL). */
  values: Record<string, string>;
  /** Other servers in the same paste, left out. */
  skipped: string[];
}

const strings = (value: unknown): Record<string, string> =>
  value && typeof value === "object" && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value).map(([key, item]) => [key, String(item ?? "")]))
    : {};

/** Reads the usual shapes of an MCP server config: `{"mcpServers": {name:
 * {...}}}` (Claude), `{"servers": {...}}` (VS Code), `{name: {...}}`, or
 * the server's own object. Null when it isn't one of them. */
export function parseServerJson(text: string): PastedServer | null {
  let data: unknown;
  try {
    data = JSON.parse(text.trim());
  } catch {
    return null;
  }
  if (!data || typeof data !== "object" || Array.isArray(data)) return null;
  const isServer = (value: unknown): boolean =>
    Boolean(value) && typeof value === "object" && ("command" in (value as object) || "url" in (value as object));

  let name: string | null = null;
  let server: Record<string, unknown> | null = null;
  let skipped: string[] = [];
  const root = data as Record<string, unknown>;
  if (isServer(root)) {
    server = root;
  } else {
    const holder = (root.mcpServers ?? root.servers ?? root) as Record<string, unknown>;
    const entries = Object.entries(holder && typeof holder === "object" ? holder : {}).filter(([, value]) => isServer(value));
    if (entries.length === 0) return null;
    [name, server] = entries[0] as [string, Record<string, unknown>];
    skipped = entries.slice(1).map(([key]) => key);
  }
  if (!server) return null;

  const url = typeof server.url === "string" ? server.url : "";
  const type = typeof server.type === "string" ? server.type.toLowerCase() : "";
  const kind = url ? (type === "sse" ? "sse" : "http") : "stdio";
  return {
    name,
    kind,
    command: typeof server.command === "string" ? server.command : "",
    args: Array.isArray(server.args) ? server.args.map((arg) => String(arg)) : [],
    url,
    values: kind === "stdio" ? strings(server.env) : strings(server.headers),
    skipped,
  };
}

/** Whether a variable or header's name says it holds a secret. */
export function looksSecret(name: string): boolean {
  return /key|token|secret|password|passwd|auth|credential|cookie/i.test(name);
}

/** A command line's arguments as typed: split on spaces, keeping quoted
 * parts together ("--name 'my server'" → ["--name", "my server"]). */
export function splitArgs(line: string): string[] {
  const args: string[] = [];
  let current = "";
  let quote: string | null = null;
  let started = false;
  for (const char of line) {
    if (quote) {
      if (char === quote) quote = null;
      else current += char;
    } else if (char === '"' || char === "'") {
      quote = char;
      started = true;
    } else if (/\s/.test(char)) {
      if (started) args.push(current);
      current = "";
      started = false;
    } else {
      current += char;
      started = true;
    }
  }
  if (started) args.push(current);
  return args;
}
