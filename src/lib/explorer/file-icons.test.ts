import { describe, expect, it } from "vitest";
import { fileBadge } from "./file-icons";

describe("fileBadge", () => {
  it("goes by the extension, ignoring case", () => {
    expect(fileBadge("main.ts")?.label).toBe("TS");
    expect(fileBadge("README.MD")?.label).toBe("MD");
  });

  it("lets a well-known whole name win over its extension", () => {
    expect(fileBadge("package.json")?.label).toBe("NP");
    expect(fileBadge("Cargo.lock")?.label).toBe("LK");
    expect(fileBadge(".env.local")?.label).toBe("EN");
  });

  it("has no badge for unknown or extensionless files", () => {
    expect(fileBadge("notes.xyz")).toBeNull();
    expect(fileBadge("CHANGES")).toBeNull();
    expect(fileBadge(".hidden")).toBeNull();
  });
});
