import type { MessageKey } from "$lib/i18n/translate";

/** CodeMirror's own texts (the search panel, folding, the rest of
 * basicSetup) and the catalog key each is said with, for
 * `EditorState.phrases`. */
export const EDITOR_PHRASES: Record<string, MessageKey> = {
  Find: "editor.cm.find",
  Replace: "editor.cm.replace",
  next: "editor.cm.next",
  previous: "editor.cm.previous",
  all: "editor.cm.all",
  "match case": "editor.cm.matchCase",
  regexp: "editor.cm.regexp",
  "by word": "editor.cm.byWord",
  replace: "editor.cm.replaceOne",
  "replace all": "editor.cm.replaceAll",
  close: "editor.cm.close",
  "Go to line": "editor.cm.goToLine",
  go: "editor.cm.go",
  "current match": "editor.cm.currentMatch",
  "on line": "editor.cm.onLine",
  "replaced match on line $": "editor.cm.replacedOne",
  "replaced $ matches": "editor.cm.replacedMany",
  "folded code": "editor.cm.foldedCode",
  to: "editor.cm.to",
  unfold: "editor.cm.unfold",
  "Fold line": "editor.cm.foldLine",
  "Unfold line": "editor.cm.unfoldLine",
  "Control character": "editor.cm.controlCharacter",
  "Selection deleted": "editor.cm.selectionDeleted",
  Completions: "editor.cm.completions",
  Diagnostics: "editor.cm.diagnostics",
  "No diagnostics": "editor.cm.noDiagnostics",
};
