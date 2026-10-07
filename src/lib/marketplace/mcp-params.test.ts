import { describe, expect, it } from "vitest";
import type { McpDetail } from "$lib/core/marketplace-client";
import { initialParamValues, paramsFor, paramsSatisfied, previewLines } from "./mcp-params";

function remoteDetail(overrides: Partial<McpDetail["remotes"][0]> = {}): McpDetail {
  return {
    kind: "mcp",
    server_name: "ai.smithery/example",
    remotes: [
      {
        type: "streamable-http",
        url: "https://server.smithery.ai/x/mcp",
        headers: [{ name: "Authorization", isRequired: true, isSecret: true, value: "Bearer {api_key}" }],
        ...overrides,
      },
    ],
    packages: [],
    repository: null,
  };
}

function packageDetail(): McpDetail {
  return {
    kind: "mcp",
    server_name: "com.pulsemcp/remote-filesystem",
    remotes: [],
    packages: [
      {
        registryType: "npm",
        identifier: "remote-filesystem-mcp-server",
        version: "0.1.5",
        environmentVariables: [
          { name: "GCS_BUCKET", isRequired: true },
          { name: "GCS_ROOT_PATH", isRequired: false, default: "/" },
        ],
      },
    ],
    repository: null,
  };
}

describe("paramsFor", () => {
  it("reads a remote's headers", () => {
    const specs = paramsFor(remoteDetail());
    expect(specs).toEqual([{ name: "Authorization", description: undefined, required: true, secret: true, default: undefined }]);
  });

  it("never treats a secret's placeholder text as a default", () => {
    const specs = paramsFor(remoteDetail());
    expect(specs[0].default).toBeUndefined();
  });

  it("falls back to the package's environment variables with no remote", () => {
    const specs = paramsFor(packageDetail());
    expect(specs.map((spec) => spec.name)).toEqual(["GCS_BUCKET", "GCS_ROOT_PATH"]);
    expect(specs[1].default).toBe("/");
  });

  it("is empty when there is nothing to configure", () => {
    expect(paramsFor({ kind: "mcp", server_name: "x", remotes: [], packages: [], repository: null })).toEqual([]);
  });
});

describe("initialParamValues", () => {
  it("prefills only the params that have a real default", () => {
    expect(initialParamValues(paramsFor(packageDetail()))).toEqual({ GCS_ROOT_PATH: "/" });
  });
});

describe("paramsSatisfied", () => {
  it("blocks on a required param with nothing entered", () => {
    const specs = paramsFor(remoteDetail());
    expect(paramsSatisfied(specs, {})).toBe(false);
    expect(paramsSatisfied(specs, { Authorization: "  " })).toBe(false);
    expect(paramsSatisfied(specs, { Authorization: "Bearer real-token" })).toBe(true);
  });

  it("never blocks on an optional param", () => {
    expect(paramsSatisfied(paramsFor(packageDetail()).filter((s) => !s.required), {})).toBe(true);
  });
});

describe("previewLines", () => {
  it("masks a secret that was entered and shows a dash when it was not", () => {
    const detail = remoteDetail();
    expect(previewLines(detail, {})[1]).toContain("—");
    expect(previewLines(detail, { Authorization: "Bearer real" })[1]).toContain("••••••");
    expect(previewLines(detail, { Authorization: "Bearer real" })[1]).not.toContain("real");
  });

  it("shows a non-secret value as-is", () => {
    const detail = packageDetail();
    expect(previewLines(detail, { GCS_BUCKET: "my-bucket" })[1]).toContain("my-bucket");
  });
});

describe("a remote's query parameters", () => {
  const detail = {
    kind: "mcp" as const,
    server_name: "acme/files",
    remotes: [
      {
        type: "streamable-http",
        url: "https://files.run.tools",
        headers: [{ name: "ACME_KEY", isRequired: true, isSecret: true }],
        queryParams: [{ name: "projectId", isRequired: true, default: "main" }],
      },
    ],
    packages: [],
    repository: null,
  };

  it("are asked for along with the headers, with their real defaults", () => {
    const specs = paramsFor(detail);
    expect(specs.map((spec) => spec.name)).toEqual(["ACME_KEY", "projectId"]);
    expect(initialParamValues(specs)).toEqual({ projectId: "main" });
    expect(paramsSatisfied(specs, { projectId: "main" })).toBe(false);
    expect(paramsSatisfied(specs, { projectId: "main", ACME_KEY: "k" })).toBe(true);
  });

  it("show in the preview, a secret never in the clear", () => {
    expect(previewLines(detail, { ACME_KEY: "k-123", projectId: "main" })).toEqual([
      "streamable-http https://files.run.tools",
      "ACME_KEY: ••••••",
      "?projectId=main",
    ]);
  });
});
