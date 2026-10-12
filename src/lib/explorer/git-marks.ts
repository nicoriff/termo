import type { GitMark } from "../core/explorer-client";
import type { ThemeAnsiColors } from "../theme/theme-types";

/** Git's letter for a changed file, as the explorer and the Git panel show
 * it next to the file's name. */
export const MARK_LETTER: Record<GitMark, string> = {
  modified: "M",
  added: "A",
  untracked: "U",
  deleted: "D",
  renamed: "R",
  conflict: "C",
};

/** The color a mark tints its file with, out of the theme's own palette.
 * "folder" is a folder with changes somewhere inside. */
export function markColor(mark: GitMark | "folder", colors: ThemeAnsiColors): string {
  switch (mark) {
    case "modified":
    case "folder":
      return colors.yellow;
    case "added":
    case "untracked":
      return colors.green;
    case "deleted":
      return colors.red;
    case "renamed":
      return colors.blue;
    case "conflict":
      return colors.magenta;
  }
}
