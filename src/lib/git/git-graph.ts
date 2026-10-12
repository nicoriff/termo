/**
 * Lays a history out as the graph draws it: which column each commit's dot
 * sits in, and the lines of its row. Pure — no runes, no Tauri — so it can be
 * unit-tested in plain Node.
 *
 * Commits come newest first, every one before its parents (git's
 * `--date-order`). A column ("lane") is a line of history on its way down to
 * the commit it's waiting for.
 */

export interface GraphInput {
  sha: string;
  parents: string[];
}

/** A line that ends or starts at the row's dot, from/to another column. */
export interface GraphLink {
  lane: number;
  color: number;
}

export interface GraphRow<T extends GraphInput = GraphInput> {
  commit: T;
  /** The column of the commit's dot. */
  lane: number;
  color: number;
  /** Lines coming from above into the dot: its children's. */
  top: GraphLink[];
  /** Lines going down from the dot: one per parent. */
  bottom: GraphLink[];
  /** Other histories passing by, top to bottom, in their own column. */
  through: GraphLink[];
  /** How many columns the row uses. */
  width: number;
}

/** What's still open below the last row laid out, to go on from: each
 * column's awaited commit (null for a free one) and its color. */
export interface GraphState {
  lanes: (string | null)[];
  colors: number[];
  nextColor: number;
}

export function emptyGraphState(): GraphState {
  return { lanes: [], colors: [], nextColor: 0 };
}

function freeLane(state: GraphState): number {
  const free = state.lanes.indexOf(null);
  if (free !== -1) return free;
  state.lanes.push(null);
  state.colors.push(0);
  return state.lanes.length - 1;
}

/** The rows of `commits`, continuing from `from` (the state a previous page
 * ended in). Returns the state to pass along with the next page. */
export function layoutGraph<T extends GraphInput>(
  commits: readonly T[],
  from: GraphState = emptyGraphState(),
): { rows: GraphRow<T>[]; state: GraphState } {
  const state: GraphState = { lanes: [...from.lanes], colors: [...from.colors], nextColor: from.nextColor };
  const rows: GraphRow<T>[] = [];

  for (const commit of commits) {
    // Every column waiting for this commit ends at its dot.
    const waiting: number[] = [];
    state.lanes.forEach((sha, index) => {
      if (sha === commit.sha) waiting.push(index);
    });
    let lane: number;
    const top: GraphLink[] = [];
    if (waiting.length === 0) {
      // Nothing above leads here: a branch's tip.
      lane = freeLane(state);
      state.colors[lane] = state.nextColor++;
    } else {
      lane = waiting[0];
      for (const index of waiting) top.push({ lane: index, color: state.colors[index] });
    }
    const color = state.colors[lane];
    for (const index of waiting) state.lanes[index] = null;

    const through: GraphLink[] = [];
    state.lanes.forEach((sha, index) => {
      if (sha !== null) through.push({ lane: index, color: state.colors[index] });
    });

    const bottom: GraphLink[] = [];
    commit.parents.forEach((parent, position) => {
      if (position === 0) {
        // The first parent carries the line on, in the same column and color.
        state.lanes[lane] = parent;
        state.colors[lane] = color;
        bottom.push({ lane, color });
        return;
      }
      // A merged-in parent: the column already heading there, or a new one.
      let target = state.lanes.findIndex((sha, index) => sha === parent && index !== lane);
      if (target === -1) {
        target = freeLane(state);
        state.lanes[target] = parent;
        state.colors[target] = state.nextColor++;
      }
      bottom.push({ lane: target, color: state.colors[target] });
    });

    const used = [lane, ...top.map((link) => link.lane), ...bottom.map((link) => link.lane), ...through.map((link) => link.lane)];
    rows.push({ commit, lane, color, top, bottom, through, width: Math.max(...used) + 1 });

    // Columns freed at the right edge go away; ones in the middle stay, so
    // the lines beside them don't jump sideways.
    while (state.lanes.length > 0 && state.lanes[state.lanes.length - 1] === null) {
      state.lanes.pop();
      state.colors.pop();
    }
  }
  return { rows, state };
}

/** Where a commit stands between the branch in hand and what it tracks. */
export type CommitSide =
  /** Only here: not pushed yet. */
  | "ahead"
  /** Only on the upstream: not pulled yet. */
  | "behind"
  /** In both, or in neither (another branch's). */
  | "shared";

/** Every commit reachable from `tip`, among the ones given. */
function ancestors(tip: string | null, parentsOf: ReadonlyMap<string, readonly string[]>): Set<string> {
  const seen = new Set<string>();
  const pending = tip ? [tip] : [];
  while (pending.length > 0) {
    const sha = pending.pop()!;
    if (seen.has(sha) || !parentsOf.has(sha)) continue;
    seen.add(sha);
    pending.push(...parentsOf.get(sha)!);
  }
  return seen;
}

/** Which of `commits` are still to push and which still to pull, given
 * where HEAD and its upstream are. Without an upstream nothing is marked:
 * there's nothing to be ahead of. */
export function commitSides(commits: readonly GraphInput[], head: string | null, upstream: string | null): Map<string, CommitSide> {
  const sides = new Map<string, CommitSide>();
  if (!head || !upstream) return sides;
  const parentsOf = new Map(commits.map((commit) => [commit.sha, commit.parents]));
  const ours = ancestors(head, parentsOf);
  const theirs = ancestors(upstream, parentsOf);
  for (const commit of commits) {
    const [mine, yours] = [ours.has(commit.sha), theirs.has(commit.sha)];
    sides.set(commit.sha, mine && !yours ? "ahead" : yours && !mine ? "behind" : "shared");
  }
  return sides;
}
