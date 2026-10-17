import { invoke } from "@tauri-apps/api/core";
import type { KeptConsole } from "./pty-client";

/** Remote access over the user's tailnet (see src-tauri/src/mux/remote.rs). */

export type RemoteAccess = "full" | "readOnly";

/** Where another computer's sessions daemon listens on the tailnet. */
export const REMOTE_PORT = 47830;

/** `ip:port` of another computer's daemon, as pty_attach takes it. */
export function remoteAddress(ip: string): string {
  return `${ip}:${REMOTE_PORT}`;
}

/** Someone allowed to connect: a Tailscale login, from one of its
 * computers or (node null) from any. */
export interface RemoteAllowed {
  login: string;
  node: string | null;
  access: RemoteAccess;
}

export interface RemoteConfig {
  enabled: boolean;
  port: number;
  allowed: RemoteAllowed[];
}

/** Someone turned away, waiting for an answer. */
export interface RemotePending {
  login: string;
  node: string;
  /** Unix seconds of their last attempt. */
  at: number;
}

/** Someone connected now. */
export interface RemoteViewer {
  login: string;
  node: string;
  access: RemoteAccess;
  /** The consoles they're showing (ids). */
  sessions: string[];
  /** Unix seconds. */
  since: number;
}

/** A computer on the tailnet. */
export interface TailnetMachine {
  name: string;
  dnsName: string;
  ip: string | null;
  os: string;
  online: boolean;
  login: string | null;
}

export interface TailscaleStatus {
  backend: string;
  me: TailnetMachine;
  peers: TailnetMachine[];
  /** Whether machines are reachable by name (MagicDNS), or only by address. */
  magicDns: boolean;
}

export interface RemoteOverview {
  tailscaleInstalled: boolean;
  tailscale: TailscaleStatus | null;
  config: RemoteConfig;
  pending: RemotePending[];
  viewers: RemoteViewer[];
}

/** Whether Tailscale is up and signed in on this computer. */
export function tailscaleRunning(overview: RemoteOverview | null): boolean {
  return overview?.tailscale?.backend === "Running" && !!overview.tailscale.me.ip;
}

export function remoteOverview(): Promise<RemoteOverview> {
  return invoke("remote_overview");
}

export function setRemoteEnabled(enabled: boolean): Promise<RemoteConfig> {
  return invoke("remote_set_enabled", { enabled });
}

export function setRemoteAllowed(allowed: RemoteAllowed[]): Promise<RemoteConfig> {
  return invoke("remote_set_allowed", { allowed });
}

/** Lets someone pending in with `access`, or (null) forgets them. */
export function answerRemotePending(login: string, node: string, access: RemoteAccess | null): Promise<RemoteOverview> {
  return invoke("remote_answer_pending", { login, node, access });
}

export function disconnectAllRemote(): Promise<void> {
  return invoke("remote_disconnect_all");
}

/** The other computers on the tailnet that are online. */
export function tailnetMachines(): Promise<TailnetMachine[]> {
  return invoke("remote_machines");
}

/** The consoles another computer keeps running, asked over the tailnet. */
export function remoteConsoles(host: string): Promise<KeptConsole[]> {
  return invoke("remote_consoles", { host });
}

/** The theme and console font this window shows, for the web client to
 * look the same (see src-tauri/src/mux/remote.rs, WebLook). */
export interface WebLook {
  appearance: string;
  colors: Record<string, string>;
  ui: Record<string, string>;
  fontFamily: string;
  lineHeight: number;
}

export function shareRemoteLook(look: WebLook): Promise<void> {
  return invoke("remote_share_look", { look });
}
