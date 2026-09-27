import { describe, expect, it } from "vitest";
import { folderHasChanges, visibleRows, type TreeEntry } from "./explorer-tree";

const dir = (path: string): TreeEntry => ({ name: path.split("/").pop()!, path, isDir: true });
const file = (path: string): TreeEntry => ({ name: path.split("/").pop()!, path, isDir: false });

describe("visibleRows", () => {
  const children = new Map<string, TreeEntry[]>([
    ["/r", [dir("/r/src"), file("/r/README.md")]],
    ["/r/src", [dir("/r/src/lib"), file("/r/src/main.ts")]],
    ["/r/src/lib", [file("/r/src/lib/a.ts")]],
  ]);

  it("shows only the root's entries while nothing is open", () => {
    expect(visibleRows("/r", children, new Set()).map((row) => row.path)).toEqual(["/r/src", "/r/README.md"]);
  });

  it("nests an open folder's entries right under it, one level deeper", () => {
    const rows = visibleRows("/r", children, new Set(["/r/src", "/r/src/lib"]));
    expect(rows.map((row) => [row.path, row.depth])).toEqual([
      ["/r/src", 0],
      ["/r/src/lib", 1],
      ["/r/src/lib/a.ts", 2],
      ["/r/src/main.ts", 1],
      ["/r/README.md", 0],
    ]);
  });

  it("skips an open folder whose contents haven't loaded yet", () => {
    const rows = visibleRows("/r", new Map([["/r", [dir("/r/src")]]]), new Set(["/r/src"]));
    expect(rows.map((row) => row.path)).toEqual(["/r/src"]);
  });
});

describe("folderHasChanges", () => {
  it("finds changes anywhere below the folder", () => {
    expect(folderHasChanges("E:\\repo\\src", ["E:\\repo\\src\\lib\\a.ts"])).toBe(true);
    expect(folderHasChanges("/repo/src", ["/repo/src/a.ts"])).toBe(true);
  });

  it("doesn't mistake a sibling sharing the prefix for a child", () => {
    expect(folderHasChanges("/repo/src", ["/repo/src-tauri/main.rs"])).toBe(false);
    expect(folderHasChanges("/repo/src", ["/repo/README.md"])).toBe(false);
  });
});
