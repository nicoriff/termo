import { describe, expect, it } from "vitest";
import { resumeCommand } from "./resume-command";

const UUID = "3f2b8c1e-1111-4222-8333-444455556666";

describe("resumeCommand", () => {
  it("uses each tool's documented resume form", () => {
    expect(resumeCommand("claude", UUID)).toBe(`claude --resume ${UUID}`);
    expect(resumeCommand("codex", UUID)).toBe(`codex resume ${UUID}`);
  });

  it("accepts Codex session names", () => {
    expect(resumeCommand("codex", "auth-refactor")).toBe("codex resume auth-refactor");
  });

  it("refuses ids that would need shell quoting", () => {
    expect(resumeCommand("claude", "id with spaces")).toBeNull();
    expect(resumeCommand("claude", "id;rm -rf /")).toBeNull();
    expect(resumeCommand("claude", "$(whoami)")).toBeNull();
    expect(resumeCommand("claude", "")).toBeNull();
  });
});
