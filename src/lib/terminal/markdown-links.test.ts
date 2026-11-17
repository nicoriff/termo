import { describe, expect, it } from "vitest";
import { findMarkdownPaths, resolveMarkdownPath } from "./markdown-links";

const paths = (line: string) => findMarkdownPaths(line).map((match) => match.path);

describe("findMarkdownPaths", () => {
  it("finds relative and absolute paths among other text", () => {
    expect(paths("● Write(docs/plan.md)")).toEqual(["docs/plan.md"]);
    expect(paths("Wrote C:\\repo\\NOTES.md and ./README.markdown")).toEqual(["C:\\repo\\NOTES.md", "./README.markdown"]);
    expect(paths("see `/home/me/x/spec.mdx`.")).toEqual(["/home/me/x/spec.mdx"]);
  });

  it("gives the range the path takes in the line", () => {
    const [match] = findMarkdownPaths('saved "docs/a.md"');
    expect(match).toEqual({ start: 7, end: 16, path: "docs/a.md" });
  });

  it("leaves a line number or a list separator out", () => {
    expect(paths("docs/a.md:12")).toEqual(["docs/a.md"]);
    expect(paths("a.md, b.md")).toEqual(["a.md", "b.md"]);
    expect(paths("in:docs/a.md")).toEqual(["docs/a.md"]);
  });

  it("skips web addresses and names that only look like it", () => {
    expect(paths("https://example.com/README.md")).toEqual([]);
    expect(paths("file.mdown file.md5 .md")).toEqual([]);
  });
});

describe("resolveMarkdownPath", () => {
  it("reads a relative path from the console's folder", () => {
    expect(resolveMarkdownPath("docs/plan.md", "C:\\repo", true)).toBe("C:\\repo\\docs\\plan.md");
    expect(resolveMarkdownPath("../x/./a.md", "/home/me/repo/", false)).toBe("/home/me/x/a.md");
  });

  it("keeps an absolute path", () => {
    expect(resolveMarkdownPath("C:/repo/a.md", undefined, true)).toBe("C:\\repo\\a.md");
    expect(resolveMarkdownPath("/tmp/a.md", "/home", false)).toBe("/tmp/a.md");
  });

  it("refuses network paths and what it can't place", () => {
    expect(resolveMarkdownPath("\\\\server\\share\\a.md", "C:\\repo", true)).toBeNull();
    expect(resolveMarkdownPath("//server/a.md", "/home", false)).toBeNull();
    expect(resolveMarkdownPath("a.md", "\\\\server\\share", true)).toBeNull();
    expect(resolveMarkdownPath("a.md", undefined, false)).toBeNull();
    expect(resolveMarkdownPath("~/a.md", "/home", false)).toBeNull();
  });
});
