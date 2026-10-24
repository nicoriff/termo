import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { GitMark } from "./explorer-client";
import type { AgentEngine } from "./agents-client";
import type { PlanStep } from "./orchestration-client";

/** See src-tauri/src/git and src-tauri/src/commands/git_commands.rs. */
export type ForgeKind = "github" | "gitlab" | "bitbucket" | "azure" | "gitea" | "generic";

export interface Forge {
  kind: ForgeKind;
  host: string;
  owner: string;
  repo: string;
  webUrl: string | null;
}

export interface FileChange {
  /** Relative to the repository, with forward slashes. */
  path: string;
  absPath: string;
  /** In the index against the last commit. */
  staged: GitMark | null;
  /** On disk against the index. */
  unstaged: GitMark | null;
  conflict: boolean;
  origPath: string | null;
}

export interface HeadCommit {
  sha: string;
  subject: string;
  author: string;
  /** Seconds since the epoch. */
  time: number;
}

export type RepoState = "merge" | "rebase" | "cherryPick" | "revert" | "bisect";

export interface Remote {
  name: string;
  fetchUrl: string;
  pushUrl: string;
  forge: Forge | null;
}

export interface BaseRef {
  name: string;
  ahead: number;
  behind: number;
}

export interface RepoSnapshot {
  root: string;
  branch: string | null;
  detached: boolean;
  upstream: string | null;
  upstreamGone: boolean;
  ahead: number;
  behind: number;
  head: HeadCommit | null;
  base: BaseRef | null;
  files: FileChange[];
  /** The repository's config brings filter programs: its changes aren't read. */
  changesUnavailable: boolean;
  stashCount: number;
  state: RepoState | null;
  /** Some git is at work on the repository right now. */
  locked: boolean;
  /** Ms since the epoch. */
  lastFetch: number | null;
  remotes: Remote[];
}

export type RefKind = "head" | "local" | "remote" | "tag";

export interface RefLabel {
  name: string;
  kind: RefKind;
}

export interface GraphCommit {
  sha: string;
  parents: string[];
  refs: RefLabel[];
  detachedHead: boolean;
  subject: string;
  author: string;
  time: number;
}

export type GraphScope = "current" | "all";

export interface Branch {
  name: string;
  remote: boolean;
  current: boolean;
  sha: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  gone: boolean;
  lastCommitTime: number;
  subject: string;
}

export interface CommitFile {
  path: string;
  mark: GitMark;
  origPath: string | null;
}

export interface CommitDetail {
  sha: string;
  parents: string[];
  author: string;
  email: string;
  time: number;
  message: string;
  files: CommitFile[];
}

export type GitOp =
  | { kind: "fetch" | "pull" | "push" | "sync" | "stageAll" | "stashPush" | "stashPop" }
  | { kind: "stage" | "unstage"; paths: string[] }
  | { kind: "discard"; tracked: string[]; untracked: string[] }
  | { kind: "commit"; message: string; amend: boolean; all: boolean }
  | { kind: "checkout"; branch: string; remote: boolean }
  | { kind: "createBranch"; name: string };

export type GitOpKind = GitOp["kind"];

export interface GitOpEvent {
  root: string;
  op: GitOpKind;
  stage: "started" | "line" | "finished";
  line: string | null;
  ok: boolean | null;
}

export type PrCli = "gh" | "glab";
export type PrStatus = "open" | "draft" | "merged" | "closed";
export type ReviewDecision = "approved" | "changesRequested" | "reviewRequired";

export interface PrInfo {
  number: number;
  title: string;
  body: string;
  state: PrStatus;
  url: string;
  base: string;
  head: string;
  author: string;
  labels: { name: string; color: string | null }[];
  reviewers: string[];
  assignees: string[];
  reviewDecision: ReviewDecision | null;
  mergeable: boolean | null;
  checks: { passed: number; failed: number; pending: number } | null;
  additions: number | null;
  deletions: number | null;
  changedFiles: number | null;
  comments: number;
  updatedAt: string | null;
}

export type PrState =
  | { kind: "ok"; cli: PrCli; pr: PrInfo }
  | { kind: "none"; cli: PrCli }
  | { kind: "noCli"; cli: PrCli }
  | { kind: "notSignedIn"; cli: PrCli }
  | { kind: "unsupported" }
  | { kind: "failed"; message: string };

/** What to change of a request; what's left out stays as it is. */
export interface PrPatch {
  title?: string;
  body?: string;
  base?: string;
  draft?: boolean;
  addLabels?: string[];
  removeLabels?: string[];
  addReviewers?: string[];
  removeReviewers?: string[];
}

export interface NewPr {
  title: string;
  body: string;
  base: string | null;
  draft: boolean;
}

export interface GitTool {
  found: boolean;
  version: string | null;
  signedIn: boolean | null;
}

export interface GitTools {
  git: GitTool;
  gh: GitTool;
  glab: GitTool;
}

/** The repository `path` is in; null outside one. */
export function gitSnapshot(path: string): Promise<RepoSnapshot | null> {
  return invoke("git_snapshot", { path });
}

/** A file as it was at `rev` (a commit, "HEAD", or the index when empty);
 * null when it wasn't there. Rejects with a message ready to show. */
export function gitBlob(root: string, rev: string, path: string): Promise<string | null> {
  return invoke("git_blob", { root, rev, path });
}

/** Where a file stands in its repository, for the editor's toolbar (see
 * src-tauri/src/git/file.rs). */
export interface FileGitInfo {
  root: string;
  /** Relative to the repository, with forward slashes. */
  path: string;
  branch: string | null;
  tracked: boolean;
  staged: GitMark | null;
  unstaged: GitMark | null;
  /** Lines added and removed on disk against HEAD; null when unknown. */
  added: number | null;
  removed: number | null;
  /** The repository's config names programs to compare files with: the
   * file on disk wasn't compared, so the four fields above say nothing. */
  changesUnavailable: boolean;
  last: {
    sha: string;
    parent: string | null;
    author: string;
    /** Seconds since the epoch. */
    time: number;
    subject: string;
  } | null;
}

export interface BlameCommit {
  sha: string;
  author: string;
  /** Seconds since the epoch. */
  time: number;
  summary: string;
  /** Where the file was in this commit. */
  path: string;
  /** Not in any commit yet: changed in the editor or on disk. */
  uncommitted: boolean;
  /** The commit before, and where the file was in it. */
  previous: string | null;
  previousPath: string | null;
}

export interface Blame {
  root: string;
  /** The file's path in the repository now, with forward slashes. */
  path: string;
  commits: BlameCommit[];
  /** For each line, from the first: its commit in `commits` and the line it
   * was in that commit's version of the file (from 1). */
  lines: [number, number][];
}

/** `path` is the file's own, absolute; null outside a repository. */
export function gitFileInfo(path: string): Promise<FileGitInfo | null> {
  return invoke("git_file_info", { path });
}

/** Who wrote each line of `contents`, the editor's text of the file at
 * `path`; null when git can't tell (a file it doesn't follow, say). */
export function gitBlame(path: string, contents: string): Promise<Blame | null> {
  return invoke("git_blame", { path, contents });
}

export function gitBranches(root: string): Promise<Branch[]> {
  return invoke("git_branches", { root });
}

export function gitGraph(root: string, scope: GraphScope, extra: string[], skip: number, limit: number): Promise<GraphCommit[]> {
  return invoke("git_graph", { root, scope, extra, skip, limit });
}

export function gitCommitDetail(root: string, sha: string): Promise<CommitDetail | null> {
  return invoke("git_commit_detail", { root, sha });
}

/** Resolves with what git printed last; rejects with why it failed. */
export function gitRun(root: string, op: GitOp): Promise<string> {
  return invoke("git_run", { root, op });
}

export function gitCancel(root: string): Promise<void> {
  return invoke("git_cancel", { root });
}

/** The repositories with an action under way. */
export function gitBusy(): Promise<string[]> {
  return invoke("git_busy");
}

export function gitPr(root: string, refresh: boolean): Promise<PrState> {
  return invoke("git_pr", { root, refresh });
}

export function gitPrUpdate(root: string, patch: PrPatch): Promise<PrState> {
  return invoke("git_pr_update", { root, patch });
}

export function gitPrCreate(root: string, pr: NewPr): Promise<PrState> {
  return invoke("git_pr_create", { root, new: pr });
}

export function gitTools(): Promise<GitTools> {
  return invoke("git_tools");
}

export function onGitOp(handler: (event: GitOpEvent) => void): Promise<UnlistenFn> {
  return listen<GitOpEvent>("git-op", (event) => handler(event.payload));
}

/** A code review by an agent CLI: see src-tauri/src/git/review.rs. */
export type ReviewScope = { kind: "changes" } | { kind: "branch" } | { kind: "commit"; sha: string };
export type ReviewSeverity = "serious" | "warning" | "suggestion";

export interface ReviewFinding {
  id: string;
  /** Relative to the repository, with forward slashes. */
  path: string;
  /** In the file's new version; 0 when it's about the file as a whole. */
  line: number;
  endLine: number;
  severity: ReviewSeverity;
  title: string;
  /** Markdown. */
  body: string;
  suggestion: string;
  dismissed: boolean;
  /** The file isn't what was reviewed any more. */
  stale: boolean;
}

export interface Review {
  id: string;
  root: string;
  branch: string;
  scope: ReviewScope;
  /** HEAD when it was made. */
  head: string;
  base: string | null;
  /** The revision the change starts from, to show a file against. */
  from: string | null;
  /** Unix milliseconds. */
  createdAt: number;
  engine: AgentEngine;
  model: string | null;
  costUsd: number | null;
  summary: string;
  findings: ReviewFinding[];
  /** The diff was too long to send whole. */
  partial: boolean;
}

export interface StoredReviews {
  /** What the user wants every review of this repository to mind. */
  note: string;
  reviews: Review[];
}

export interface ReviewRequest {
  scope: ReviewScope;
  /** Whose CLI reviews, when no agent is named. */
  engine: AgentEngine;
  agentId: string | null;
  /** The branch a pull request of this one goes to. */
  base: string | null;
  instructions: string;
  timeoutMin: number;
}

export interface ReviewStepEvent {
  root: string;
  step: PlanStep;
}

/** Resolves with the review when it's done; rejects with why it wasn't. */
export function gitReviewStart(root: string, request: ReviewRequest): Promise<Review> {
  return invoke("git_review_start", { root, request });
}

export function gitReviewCancel(root: string): Promise<void> {
  return invoke("git_review_cancel", { root });
}

/** The repositories with a review under way. */
export function gitReviewBusy(): Promise<string[]> {
  return invoke("git_review_busy");
}

export function gitReviewLoad(root: string): Promise<StoredReviews> {
  return invoke("git_review_load", { root });
}

export function gitReviewDismiss(root: string, reviewId: string, findingId: string, dismissed: boolean): Promise<StoredReviews> {
  return invoke("git_review_dismiss", { root, reviewId, findingId, dismissed });
}

export function gitReviewClear(root: string, reviewId: string): Promise<StoredReviews> {
  return invoke("git_review_clear", { root, reviewId });
}

export function gitReviewSetNote(root: string, note: string): Promise<StoredReviews> {
  return invoke("git_review_set_note", { root, note });
}

/** The comment publishing would leave, to show before it does. */
export function gitReviewComment(root: string, reviewId: string, findingIds: string[]): Promise<string> {
  return invoke("git_review_comment", { root, reviewId, findingIds });
}

export function gitReviewPublish(root: string, reviewId: string, findingIds: string[]): Promise<void> {
  return invoke("git_review_publish", { root, reviewId, findingIds });
}

/** Hands a finding to one of the user's agents to fix: a run of that agent. */
export function gitReviewFix(root: string, reviewId: string, findingId: string, agentId: string): Promise<unknown> {
  return invoke("git_review_fix", { root, reviewId, findingId, agentId });
}

export function onGitReviewStep(handler: (event: ReviewStepEvent) => void): Promise<UnlistenFn> {
  return listen<ReviewStepEvent>("git-review-step", (event) => handler(event.payload));
}
