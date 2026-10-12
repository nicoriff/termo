/**
 * What a code review comes down to, for the Git panel's section, the marks
 * on changed files and the question before a push. Pure — no runes, no
 * Tauri — so it can be unit-tested in plain Node; anything shown comes back
 * as a catalog key for the component to translate.
 */

import type { MessageKey } from "../i18n/translate";

export type Severity = "serious" | "warning" | "suggestion";
/** Most serious first: the order findings are read in. */
export const SEVERITIES: Severity[] = ["serious", "warning", "suggestion"];

export type Scope = { kind: "changes" } | { kind: "branch" } | { kind: "commit"; sha: string };

export interface Finding {
  id: string;
  path: string;
  line: number;
  endLine: number;
  severity: Severity;
  title: string;
  body: string;
  suggestion: string;
  dismissed: boolean;
  stale: boolean;
}

export interface ReviewLike<F extends Finding = Finding> {
  id: string;
  branch: string;
  scope: Scope;
  head: string;
  createdAt: number;
  findings: F[];
}

export const SEVERITY_LABEL: Record<Severity, MessageKey> = {
  serious: "git.review.severity.serious",
  warning: "git.review.severity.warning",
  suggestion: "git.review.severity.suggestion",
};

export const SCOPE_LABEL: Record<Scope["kind"], MessageKey> = {
  changes: "git.review.scope.changes",
  branch: "git.review.scope.branch",
  commit: "git.review.scope.commit",
};

/** What tells one scope's review from another's on the same branch. */
export function scopeKey(scope: Scope): string {
  return scope.kind === "commit" ? `commit:${scope.sha}` : scope.kind;
}

/** Whether a finding is as serious as `least` or more. */
export function atLeast(severity: Severity, least: Severity): boolean {
  return SEVERITIES.indexOf(severity) <= SEVERITIES.indexOf(least);
}

/** The findings to show: as serious as asked for, and the dismissed ones
 * only when wanted. */
export function shownFindings<F extends Finding>(findings: readonly F[], least: Severity, withDismissed: boolean): F[] {
  return findings.filter((finding) => atLeast(finding.severity, least) && (withDismissed || !finding.dismissed));
}

export interface ReviewCounts {
  serious: number;
  warning: number;
  suggestion: number;
  /** Not dismissed, whatever their severity. */
  open: number;
  dismissed: number;
  /** Open, on a file that changed since. */
  stale: number;
}

export function countFindings(findings: readonly Finding[]): ReviewCounts {
  const counts: ReviewCounts = { serious: 0, warning: 0, suggestion: 0, open: 0, dismissed: 0, stale: 0 };
  for (const finding of findings) {
    if (finding.dismissed) {
      counts.dismissed++;
      continue;
    }
    counts.open++;
    counts[finding.severity]++;
    if (finding.stale) counts.stale++;
  }
  return counts;
}

/** The findings that hold a push up: serious, still open, and about the
 * file as it is. */
export function blockingFindings<F extends Finding>(findings: readonly F[]): F[] {
  return findings.filter((finding) => finding.severity === "serious" && !finding.dismissed && !finding.stale);
}

export interface FileFindings<F extends Finding> {
  path: string;
  findings: F[];
}

/** By file, each file's by line; the files with the most serious first. */
export function groupByFile<F extends Finding>(findings: readonly F[]): FileFindings<F>[] {
  const byPath = new Map<string, F[]>();
  for (const finding of findings) {
    const list = byPath.get(finding.path);
    if (list) list.push(finding);
    else byPath.set(finding.path, [finding]);
  }
  const worst = (list: F[]) => Math.min(...list.map((finding) => SEVERITIES.indexOf(finding.severity)));
  return [...byPath.entries()]
    .map(([path, list]) => ({ path, findings: [...list].sort((a, b) => a.line - b.line) }))
    .sort((a, b) => worst(a.findings) - worst(b.findings) || a.path.localeCompare(b.path));
}

/** "src/a.rs:12", "src/a.rs:12-18", or the file alone for one about all of it. */
export function findingPlace(finding: Pick<Finding, "path" | "line" | "endLine">): string {
  if (finding.line <= 0) return finding.path;
  return finding.endLine > finding.line ? `${finding.path}:${finding.line}-${finding.endLine}` : `${finding.path}:${finding.line}`;
}

/** The reviews made on `branch`, newest first. */
export function reviewsOf<R extends ReviewLike>(reviews: readonly R[], branch: string | null): R[] {
  return reviews.filter((review) => review.branch === (branch ?? "")).sort((a, b) => b.createdAt - a.createdAt);
}

/** The branch's review of `scope`, when there's one. */
export function reviewOf<R extends ReviewLike>(reviews: readonly R[], branch: string | null, scope: Scope): R | null {
  return reviewsOf(reviews, branch).find((review) => scopeKey(review.scope) === scopeKey(scope)) ?? null;
}

/** The files of the working tree with a finding still open: from the
 * branch's reviews of its changes and of the branch itself, not of a commit. */
export function filesWithFindings(reviews: readonly ReviewLike[], branch: string | null): Map<string, Severity> {
  const marks = new Map<string, Severity>();
  for (const review of reviewsOf(reviews, branch)) {
    if (review.scope.kind === "commit") continue;
    for (const finding of review.findings) {
      if (finding.dismissed || finding.stale) continue;
      const before = marks.get(finding.path);
      if (!before || atLeast(finding.severity, before)) marks.set(finding.path, finding.severity);
    }
  }
  return marks;
}

/** How many findings are still open, and about the files as they are,
 * across `reviews`. */
export function openFindings(reviews: readonly ReviewLike[]): number {
  return reviews.reduce((sum, review) => sum + review.findings.filter((finding) => !finding.dismissed && !finding.stale).length, 0);
}

/** A stretch of an agent's text: prose, or what was inside a code fence. */
export interface TextPart {
  code: boolean;
  text: string;
}

/** An agent's text with its fenced code set apart from the prose around
 * it, the fences themselves (and the language named on them) taken out. A
 * fence left open runs to the end. */
export function splitFences(text: string): TextPart[] {
  const parts: TextPart[] = [];
  let lines: string[] = [];
  let code = false;
  const flush = () => {
    const body = lines.join("\n");
    if (code ? body !== "" : body.trim() !== "") parts.push({ code, text: code ? body : body.trim() });
    lines = [];
  };
  for (const line of text.replaceAll("\r\n", "\n").split("\n")) {
    if (/^\s*```/.test(line)) {
      flush();
      code = !code;
    } else lines.push(line);
  }
  flush();
  return parts;
}

/** What goes to an agent in a console to fix a finding: `lead` (the
 * request, in the user's language) and the finding under it. */
export function fixRequest(finding: Pick<Finding, "path" | "line" | "endLine" | "title" | "body" | "suggestion">, lead: string): string {
  const parts = [lead, "", `${findingPlace(finding)} — ${finding.title}`, "", finding.body.trim()];
  if (finding.suggestion.trim()) parts.push("", finding.suggestion.trim());
  return parts.join("\n").trim();
}

/** `text` as a paste into a console: the program in it takes it whole, new
 * lines and all, and nothing is sent until the user presses Enter.
 *
 * The text is an agent's, written from a diff nobody vouches for, so no
 * control character of it goes through but the new line and the tab: an
 * escape could end the paste early (taking the end mark out isn't enough —
 * what's left around it can spell another), and what came after would be
 * typed into the console as keys, Enter included. */
export function asPaste(text: string): string {
  const plain = text.replace(/\r\n?/g, "\n").replace(/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g, "");
  return `\x1b[200~${plain}\x1b[201~`;
}
