import { lazy } from "$lib/core/lazy";

/** The file and diff tabs, with CodeMirror under them: loaded when one opens. */
export const loadFileEditor = lazy(() => import("./FileEditor.svelte"));
export const loadDiffView = lazy(() => import("./DiffView.svelte"));
