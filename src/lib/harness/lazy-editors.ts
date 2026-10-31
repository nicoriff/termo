import { lazy } from "$lib/core/lazy";

/** The Harness panel's diff and file editor, with CodeMirror under them:
 * loaded when one shows. */
export const loadHarnessDiff = lazy(() => import("./HarnessDiff.svelte"));
export const loadHarnessTextEditor = lazy(() => import("./HarnessTextEditor.svelte"));
