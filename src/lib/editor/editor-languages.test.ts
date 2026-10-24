import { describe, expect, it } from "vitest";
import { languageFor } from "./editor-languages";

const nameOf = (file: string) => languageFor(file)?.name ?? null;

describe("languageFor", () => {
  it("reads what language-data knows by itself", () => {
    expect(nameOf("main.rs")).toBe("Rust");
    expect(nameOf("App.tsx")).toBe("TSX");
    expect(nameOf("Dockerfile")).toBe("Dockerfile");
    expect(nameOf("build.ps1")).toBe("PowerShell");
  });

  it("gives markdown its own, with code blocks, for .md and .mdx", () => {
    expect(nameOf("README.md")).toBe("Markdown");
    expect(nameOf("page.mdx")).toBe("Markdown");
    expect(languageFor("README.md")).toBe(languageFor("CLAUDE.MD"));
  });

  it("reads the files language-data has nothing for in a close language", () => {
    expect(nameOf("tsconfig.json")).toBe("JSON");
    expect(nameOf("settings.jsonc")).toBe("JSON");
    expect(nameOf("Termo.code-workspace")).toBe("JSON");
    expect(nameOf("Card.svelte")).toBe("HTML");
    expect(nameOf("index.astro")).toBe("HTML");
    expect(nameOf("prompt.zsh")).toBe("Shell");
    expect(nameOf(".zshrc")).toBe("Shell");
    expect(nameOf(".env")).toBe("Properties files");
    expect(nameOf(".env.local")).toBe("Properties files");
    expect(nameOf(".gitignore")).toBe("Properties files");
    expect(nameOf("Cargo.lock")).toBe("TOML");
  });

  it("leaves what it can't tell as plain text", () => {
    expect(nameOf("notes.log")).toBeNull();
    expect(nameOf("LICENSE")).toBeNull();
  });
});
