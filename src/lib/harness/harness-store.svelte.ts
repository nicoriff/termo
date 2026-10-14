import { agentsStore } from "$lib/agents/agents-store.svelte";
import { explorerRoot } from "$lib/core/explorer-client";
import {
  harnessApply,
  harnessLoad,
  harnessPreview,
  type ExternalEntry,
  type HarnessFileChange,
  type HarnessSpec,
  type HarnessState,
} from "$lib/core/harness-client";
import { fileTabs } from "$lib/editor/file-tabs.svelte";
import { explorerStore } from "$lib/explorer/explorer-store.svelte";
import { gitStore } from "$lib/git/git-store.svelte";
import { sessionFolder, workspaceStore } from "$lib/layout/workspace-store.svelte";
import { addRule, copySpec, harnessStatus, sameEntry, sameSpec } from "./harness-model";

/** Wider than the other side panels: rules and instructions are edited in it. */
export const HARNESS_WIDTH = { initial: 380, min: 300, max: 760 };
const WIDTH_KEY = "termo.harness.width";

function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved ? clampWidth(saved) : HARNESS_WIDTH.initial;
  } catch {
    return HARNESS_WIDTH.initial;
  }
}

function clampWidth(width: number): number {
  return Math.round(Math.min(HARNESS_WIDTH.max, Math.max(HARNESS_WIDTH.min, width)));
}

/** The Harness panel: the rules of the repository the focused tab is in,
 * edited as a draft and written, after a look at what changes, into each
 * agent's files (see src-tauri/src/harness). Docked on the window's left
 * edge like the explorer, the Git panel and the agents panel: one at a time. */
class HarnessStore {
  open = $state(false);
  width = $state(loadWidth());
  /** The repository in view; null when the tab's folder isn't in one. */
  root = $state<string | null>(null);
  /** False until the first look at the tab's folder. */
  loaded = $state(false);
  saved = $state<HarnessState | null>(null);
  error = $state<string | null>(null);
  /** The changes on review, before they're written; null while editing. */
  review = $state<HarnessFileChange[] | null>(null);
  busy = $state(false);
  /** The instruction file open in the panel's editor, relative to the root;
   * null while none is. */
  editingFile = $state<string | null>(null);
  /** Unapplied edits, by repository: switching tabs doesn't lose them. */
  private drafts = $state<Record<string, HarnessSpec>>({});
  /** The user's entries adopted in the draft, by repository: taken out of
   * their files when it's applied. */
  private adoptions = $state<Record<string, ExternalEntry[]>>({});

  /** The folder of the tab in focus. */
  private folder = $derived.by(() => {
    const session = workspaceStore.focusedSession;
    return session ? (sessionFolder(session) ?? null) : null;
  });

  draft = $derived<HarnessSpec | null>(
    this.root ? (this.drafts[this.root] ?? (this.saved ? this.saved.spec : null)) : null,
  );
  adopted = $derived<ExternalEntry[]>(this.root ? (this.adoptions[this.root] ?? []) : []);
  /** The repository's rules that aren't Termo's and aren't adopted yet. */
  external = $derived(
    (this.saved?.external ?? []).filter((entry) => !this.adopted.some((other) => sameEntry(other, entry))),
  );
  dirty = $derived(
    !!this.draft && !!this.saved && (!sameSpec(this.draft, this.saved.spec) || this.adopted.length > 0),
  );
  status = $derived(harnessStatus(this.saved?.exists ?? false, this.dirty, this.saved?.pending.length ?? 0));

  /** Starts following the tab in focus while the panel is open. The main
   * window calls it once; returns what stops it. */
  start(): () => void {
    return $effect.root(() => {
      $effect(() => {
        if (!this.open) return;
        const folder = this.folder;
        // A file saved in Termo may be one of the agents' own.
        void fileTabs.savedTick;
        // An agent tab is in no folder: the repository stays as it was.
        if (folder) void this.follow(folder);
        else if (this.root) void this.reload();
        else this.loaded = true;
      });
    });
  }

  toggle(): void {
    this.setOpen(!this.open);
  }

  setOpen(open: boolean): void {
    if (open) {
      agentsStore.setOpen(false);
      explorerStore.open = false;
      gitStore.open = false;
    }
    this.open = open;
    if (!open) this.review = null;
  }

  setWidth(width: number, persist = false): void {
    this.width = clampWidth(width);
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.width));
    } catch {
      // Only a convenience.
    }
  }

  private async follow(folder: string): Promise<void> {
    let root: string | null = null;
    try {
      const found = await explorerRoot(folder);
      root = found.isRepo ? found.path : null;
    } catch {
      root = null;
    }
    if (root !== this.root) {
      this.root = root;
      this.saved = null;
      this.review = null;
      this.editingFile = null;
      this.error = null;
    }
    if (root) await this.reload();
    this.loaded = true;
  }

  /** Reads the repository's harness again. */
  async reload(): Promise<void> {
    const root = this.root;
    if (!root) return;
    try {
      const state = await harnessLoad(root);
      if (root !== this.root) return;
      this.saved = state;
      this.error = null;
    } catch (reason) {
      if (root === this.root) this.error = String(reason);
    }
  }

  /** Changes the draft through `edit`, on a copy. */
  edit(edit: (spec: HarnessSpec) => void): void {
    const root = this.root;
    const base = this.draft;
    if (!root || !base) return;
    const next = copySpec(base);
    edit(next);
    this.drafts = { ...this.drafts, [root]: next };
  }

  discard(): void {
    const root = this.root;
    if (!root) return;
    const rest = { ...this.drafts };
    delete rest[root];
    this.drafts = rest;
    const adoptions = { ...this.adoptions };
    delete adoptions[root];
    this.adoptions = adoptions;
  }

  /** Takes a rule that's in the agents' files into the harness: its entries
   * go from there when the draft is applied, for Termo's to take their place. */
  adopt(entries: ExternalEntry[]): void {
    const root = this.root;
    const rule = entries[0]?.rule;
    if (!root || !rule) return;
    this.edit((spec) => addRule(spec, rule));
    this.adoptions = { ...this.adoptions, [root]: [...this.adopted, ...entries] };
  }

  /** Opens an instruction file in the panel's editor. */
  editFile(path: string | null): void {
    this.editingFile = path;
  }

  /** The absolute path of a file relative to the root. */
  absolute(path: string): string | null {
    const root = this.root;
    if (!root) return null;
    const separator = root.includes("\\") ? "\\" : "/";
    return `${root}${separator}${path.replaceAll("/", separator)}`;
  }

  /** What applying the draft would write, to look at first. The harness on
   * disk out of step with its files reviews the same way, with no edits. */
  async startReview(): Promise<void> {
    const root = this.root;
    const spec = this.draft;
    if (!root || !spec || this.busy) return;
    this.busy = true;
    try {
      const changes = await harnessPreview(root, spec, this.adopted);
      if (root === this.root) {
        this.review = changes;
        this.error = null;
      }
    } catch (reason) {
      if (root === this.root) this.error = String(reason);
    } finally {
      this.busy = false;
    }
  }

  cancelReview(): void {
    this.review = null;
  }

  async apply(): Promise<void> {
    const root = this.root;
    const spec = this.draft;
    if (!root || !spec || this.busy) return;
    this.busy = true;
    try {
      const state = await harnessApply(root, spec, this.adopted);
      if (root !== this.root) return;
      this.saved = state;
      this.review = null;
      this.error = null;
      this.discard();
    } catch (reason) {
      if (root === this.root) this.error = String(reason);
    } finally {
      this.busy = false;
    }
  }
}

export const harnessStore = new HarnessStore();
