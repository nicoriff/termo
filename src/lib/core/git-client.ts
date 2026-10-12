import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { GitMark } from "./explorer-client";

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
