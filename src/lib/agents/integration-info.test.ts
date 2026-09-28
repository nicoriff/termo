import { describe, expect, it } from "vitest";
import type { CatalogItem, McpDetail } from "$lib/core/marketplace-client";
import { formatCount, installSource, looksSecret, parseServerJson, publisherOf, splitArgs } from "./integration-info";

function item(serverName: string, author: string | null = null, detail: Partial<McpDetail> = {}): CatalogItem {
  return {
    author,
    detail: { kind: "mcp", server_name: serverName, remotes: [], packages: [], repository: null, ...detail },
  } as unknown as CatalogItem;
}

describe("publisherOf", () => {
  it("prefers the repository's owner", () => {
    expect(publisherOf(item("ai.smithery/fetch", "Hint-Services"))).toBe("Hint-Services");
  });

  it("falls back to the namespace it's published under", () => {
    expect(publisherOf(item("io.github.someone/tool"))).toBe("someone");
    expect(publisherOf(item("ai.smithery/fetch"))).toBe("smithery");
    expect(publisherOf(item("local/thing"))).toBe("local");
  });
});

describe("installSource", () => {
  it("is the remote when there is one, like the install", () => {
    const detail = { remotes: [{ url: "https://x" }], packages: [{ registryType: "npm", identifier: "x" }] };
    expect(installSource(item("a/b", null, detail).detail as McpDetail)).toEqual({ kind: "remote" });
  });

  it("else the first package", () => {
    const detail = { packages: [{ registryType: "pypi", identifier: "mcp-fetch" }] };
    expect(installSource(item("a/b", null, detail).detail as McpDetail)).toEqual({
      kind: "package",
      registryType: "pypi",
      identifier: "mcp-fetch",
    });
    expect(installSource(item("a/b").detail as McpDetail)).toBeNull();
  });
});

describe("formatCount", () => {
  it("abbreviates big numbers", () => {
    expect(formatCount(950, "en")).toBe("950");
    expect(formatCount(12_300, "en")).toBe("12.3K");
    expect(formatCount(1_200_000, "en")).toBe("1.2M");
  });
});

describe("parseServerJson", () => {
  it("reads Claude's mcpServers shape and keeps the name", () => {
    const pasted = parseServerJson(
      JSON.stringify({ mcpServers: { github: { command: "npx", args: ["-y", "@x/gh"], env: { GITHUB_TOKEN: "abc" } } } }),
    );
    expect(pasted).toEqual({
      name: "github",
      kind: "stdio",
      command: "npx",
      args: ["-y", "@x/gh"],
      url: "",
      values: { GITHUB_TOKEN: "abc" },
      skipped: [],
    });
  });

  it("reads VS Code's servers shape, a bare named entry, and the server alone", () => {
    expect(parseServerJson('{"servers": {"a": {"command": "uvx", "args": ["a"]}}}')?.command).toBe("uvx");
    expect(parseServerJson('{"docs": {"command": "node", "args": ["x.js"]}}')?.name).toBe("docs");
    expect(parseServerJson('{"command": "npx", "args": ["-y", "z"]}')).toMatchObject({ name: null, args: ["-y", "z"] });
  });

  it("recognizes a remote server and its headers", () => {
    const pasted = parseServerJson('{"mcpServers": {"r": {"type": "sse", "url": "https://x/mcp", "headers": {"Authorization": "Bearer t"}}}}');
    expect(pasted).toMatchObject({ kind: "sse", url: "https://x/mcp", values: { Authorization: "Bearer t" } });
  });

  it("uses the first of several servers and says which were left out", () => {
    const pasted = parseServerJson('{"mcpServers": {"a": {"command": "x"}, "b": {"command": "y"}}}');
    expect(pasted?.name).toBe("a");
    expect(pasted?.skipped).toEqual(["b"]);
  });

  it("is null for anything else", () => {
    expect(parseServerJson("not json")).toBeNull();
    expect(parseServerJson('{"hello": 1}')).toBeNull();
    expect(parseServerJson("[1, 2]")).toBeNull();
  });
});

describe("looksSecret", () => {
  it("spots the usual secret names", () => {
    for (const name of ["API_KEY", "GITHUB_TOKEN", "DB_PASSWORD", "Authorization", "client_secret"]) {
      expect(looksSecret(name)).toBe(true);
    }
    expect(looksSecret("REGION")).toBe(false);
  });
});

describe("splitArgs", () => {
  it("splits on spaces and keeps quoted parts together", () => {
    expect(splitArgs("-y  mi-server --name 'my server' \"C:\\a b\"")).toEqual([
      "-y",
      "mi-server",
      "--name",
      "my server",
      "C:\\a b",
    ]);
    expect(splitArgs("   ")).toEqual([]);
    expect(splitArgs("--empty ''")).toEqual(["--empty", ""]);
  });
});
