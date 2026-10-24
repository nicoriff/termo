import { describe, expect, it } from "vitest";
import { EditorState, type TransactionSpec } from "@codemirror/state";
import { mapBlameLines } from "./editor-blame";

/** Lines a..e from commits 0..4, each at its own line there. */
const DOC = "a\nb\nc\nd\ne";
const LINES: [number, number][] = [0, 1, 2, 3, 4].map((commit) => [commit, commit + 1]);

function edit(spec: TransactionSpec) {
  const state = EditorState.create({ doc: DOC });
  const tr = state.update(spec);
  return mapBlameLines(LINES, tr.changes, state.doc, tr.state.doc).map(([commit]) => commit);
}

describe("mapBlameLines", () => {
  it("marks a line typed in as changed and keeps the rest", () => {
    expect(edit({ changes: { from: 2, insert: "x" } })).toEqual([0, -1, 2, 3, 4]);
  });

  it("moves the lines below an inserted line down", () => {
    // Typed at the end of a's line: that line counts as touched too.
    expect(edit({ changes: { from: 1, insert: "\nnew" } })).toEqual([-1, -1, 1, 2, 3, 4]);
  });

  it("drops deleted lines", () => {
    // "b\nc\n" goes: b's line and c's line are gone, d moves up.
    expect(edit({ changes: { from: 2, to: 6 } })).toEqual([0, -1, 4]);
  });

  it("counts a line once when two edits land on it", () => {
    expect(edit({ changes: [{ from: 2, insert: "x" }, { from: 3, insert: "y" }] })).toEqual([0, -1, 2, 3, 4]);
  });

  it("ends with as many lines as the text has", () => {
    const state = EditorState.create({ doc: DOC });
    const tr = state.update({ changes: { from: 0, to: state.doc.length, insert: "one\ntwo" } });
    expect(mapBlameLines(LINES, tr.changes, state.doc, tr.state.doc)).toHaveLength(2);
  });
});
