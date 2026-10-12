/** The diff a line is asked for in: the same four things a diff tab is told
 * apart by (see workspace-store's openDiff). */
interface JumpDiff {
  root: string;
  path: string;
  from: string | null;
  to: string | null;
}

/**
 * A line to show in a file's diff tab: set right before the tab is opened or
 * brought forward (a review's finding, see git/GitReviewSection.svelte), and
 * taken by that tab once it has the file on screen. Taken, it's gone: a tab
 * opened later for the same file starts where any other does.
 */
class DiffJump {
  target = $state<(JumpDiff & { line: number }) | null>(null);

  to(diff: JumpDiff, line: number): void {
    this.target = line > 0 ? { root: diff.root, path: diff.path, from: diff.from, to: diff.to, line } : null;
  }

  /** The line asked for in `diff`, once; null when none was, or it was for
   * another diff. */
  take(diff: JumpDiff): number | null {
    const target = this.target;
    if (!target || target.root !== diff.root || target.path !== diff.path || target.from !== diff.from || target.to !== diff.to) return null;
    this.target = null;
    return target.line;
  }
}

export const diffJump = new DiffJump();
