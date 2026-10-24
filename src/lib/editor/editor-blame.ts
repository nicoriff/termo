import { Prec, StateEffect, StateField, type ChangeSet, type Extension, type Text } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, ViewPlugin, WidgetType, gutter, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import type { Blame, BlameCommit } from "$lib/core/git-client";

/** Who wrote each line, in the file editor: at the end of the cursor's line,
 * or in a column next to every line. */
export type BlameMode = "line" | "gutter";

/** A line's commit in the blame, and the line it was in that commit; null
 * for a line changed since the blame was asked for. */
export type BlameLine = { commit: BlameCommit; line: number } | null;

export interface BlameView {
  mode: BlameMode;
  /** The short text shown for a commit (author, age, maybe its message). */
  label: (commit: BlameCommit) => string;
  /** The hover text: the commit in full. */
  hint: (commit: BlameCommit) => string;
  /** What a line changed in the editor shows, not in any commit yet. */
  edited: string;
  /** A click on a commit's text. */
  onpick: (commit: BlameCommit, line: number) => void;
}

/** Per line of the text: an index in `commits` and that commit's line, or -1
 * for a line changed in the editor since. */
interface BlameState {
  commits: BlameCommit[];
  lines: [number, number][];
}

export const setBlame = StateEffect.define<Blame | null>();

/**
 * Carries a blame's lines along the edits made after it was asked for:
 * untouched lines keep their commit, and what an edit wrote is marked as
 * changed. Exported to be tested.
 */
export function mapBlameLines(lines: [number, number][], changes: ChangeSet, before: Text, after: Text): [number, number][] {
  const out: [number, number][] = [];
  let next = 1;
  const keep = (upTo: number) => {
    for (; next < upTo; next++) out.push(lines[next - 1] ?? [-1, 0]);
  };
  changes.iterChanges((fromA, toA, fromB, toB) => {
    keep(before.lineAt(fromA).number);
    // Two edits on one line: the line is in already.
    for (let line = Math.max(after.lineAt(fromB).number, out.length + 1); line <= after.lineAt(toB).number; line++) out.push([-1, 0]);
    next = Math.max(next, before.lineAt(toA).number + 1);
  });
  keep(before.lines + 1);
  return out;
}

const blameField = StateField.define<BlameState | null>({
  create: () => null,
  update(value, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setBlame)) return effect.value && { commits: effect.value.commits, lines: effect.value.lines };
    }
    if (!value || !tr.docChanged) return value;
    return { commits: value.commits, lines: mapBlameLines(value.lines, tr.changes, tr.startState.doc, tr.state.doc) };
  },
});

/** Line `number`'s blame (from 1); undefined when there's none to show. */
function lineBlame(state: BlameState | null, number: number): BlameLine | undefined {
  const entry = state?.lines[number - 1];
  if (!entry) return undefined;
  const commit = state!.commits[entry[0]];
  return commit ? { commit, line: entry[1] } : null;
}

class InlineBlame extends WidgetType {
  constructor(
    readonly text: string,
    readonly hint: string,
    readonly pick: (() => void) | null,
  ) {
    super();
  }

  eq(other: InlineBlame): boolean {
    return other.text === this.text && other.hint === this.hint;
  }

  toDOM(): HTMLElement {
    const span = document.createElement("span");
    span.className = "cm-blame-inline";
    span.textContent = this.text;
    if (this.hint) span.title = this.hint;
    if (this.pick) {
      span.classList.add("cm-blame-link");
      span.onmousedown = (event) => event.preventDefault();
      span.onclick = this.pick;
    }
    return span;
  }

  ignoreEvent(): boolean {
    return true;
  }
}

/** The cursor's line, its commit at its end. */
function inlineBlame(config: BlameView): Extension {
  const build = (view: EditorView): DecorationSet => {
    const state = view.state.field(blameField, false) ?? null;
    if (!state) return Decoration.none;
    const line = view.state.doc.lineAt(view.state.selection.main.head);
    const blame = lineBlame(state, line.number);
    if (blame === undefined) return Decoration.none;
    const widget = blame
      ? new InlineBlame(config.label(blame.commit), config.hint(blame.commit), blame.commit.uncommitted ? null : () => config.onpick(blame.commit, blame.line))
      : new InlineBlame(config.edited, "", null);
    return Decoration.set([Decoration.widget({ widget, side: 1 }).range(line.to)]);
  };
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = build(view);
      }
      update(update: ViewUpdate) {
        const blameChanged = update.startState.field(blameField, false) !== update.state.field(blameField, false);
        if (update.docChanged || update.selectionSet || blameChanged) this.decorations = build(update.view);
      }
    },
    { decorations: (plugin) => plugin.decorations },
  );
}

class GutterBlame extends GutterMarker {
  constructor(
    readonly text: string,
    readonly hint: string,
    readonly edited: boolean,
  ) {
    super();
  }

  eq(other: GutterBlame): boolean {
    return other.text === this.text && other.hint === this.hint && other.edited === this.edited;
  }

  toDOM(): Node {
    const span = document.createElement("span");
    span.className = this.edited ? "cm-blame-edited" : "cm-blame-label";
    span.textContent = this.text;
    if (this.hint) span.title = this.hint;
    return span;
  }
}

/** Every line, its commit in a column left of the line numbers: written
 * once at the top of each run of lines from the same commit. */
function gutterBlame(config: BlameView): Extension {
  return Prec.highest(gutter({
    class: "cm-blame-gutter",
    lineMarker(view, block) {
      const state = view.state.field(blameField, false) ?? null;
      const number = view.state.doc.lineAt(block.from).number;
      const blame = lineBlame(state, number);
      if (blame === undefined) return null;
      const above = number > 1 ? lineBlame(state, number - 1) : undefined;
      if (blame === null) return above === null ? null : new GutterBlame(config.edited, "", true);
      if (above && above.commit === blame.commit) return null;
      return new GutterBlame(config.label(blame.commit), config.hint(blame.commit), blame.commit.uncommitted);
    },
    lineMarkerChange: (update) => update.startState.field(blameField, false) !== update.state.field(blameField, false),
    domEventHandlers: {
      click(view, block) {
        const blame = lineBlame(view.state.field(blameField, false) ?? null, view.state.doc.lineAt(block.from).number);
        if (!blame || blame.commit.uncommitted) return false;
        config.onpick(blame.commit, blame.line);
        return true;
      },
    },
  }));
}

const blameTheme = EditorView.baseTheme({
  ".cm-blame-inline": {
    marginLeft: "3em",
    opacity: "0.45",
    fontStyle: "italic",
    whiteSpace: "pre",
  },
  ".cm-blame-link": { cursor: "pointer" },
  ".cm-blame-link:hover": { opacity: "0.8", textDecoration: "underline" },
  ".cm-blame-gutter .cm-gutterElement": {
    maxWidth: "28ch",
    padding: "0 10px 0 6px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    fontSize: "0.85em",
    opacity: "0.55",
    cursor: "pointer",
  },
  ".cm-blame-gutter .cm-blame-edited": { fontStyle: "italic" },
});

/** The blame's state: always there, so a blame survives a change of mode. */
export const blameState: Extension = blameField;

/** How the blame is shown; reconfigured when the mode or the language changes. */
export function blameView(config: BlameView): Extension {
  return [blameTheme, config.mode === "gutter" ? gutterBlame(config) : inlineBlame(config)];
}
