/**
 * The words around a run's diagnosis (see src-tauri/src/agents/diagnose.rs):
 * who answered, what kind of failure it was and what to do about it, the
 * session a run reported, and its usage warnings. Pure: the caller hands in
 * the translator, so it's testable in Node.
 */

import type { FailureKind, Provider, RunFailure, SessionInfo, UsageInfo } from "$lib/core/agents-client";
import type { MessageKey } from "$lib/i18n/translate";
import { ENGINE_NAMES } from "./orchestration-spec";

type Translate = (key: MessageKey, params?: Record<string, string>) => string;

export const PROVIDER_KEYS: Record<Provider, MessageKey> = {
  anthropic: "agents.diagnosis.provider.anthropic",
  openai: "agents.diagnosis.provider.openai",
  google: "agents.diagnosis.provider.google",
  github: "agents.diagnosis.provider.github",
  xai: "agents.diagnosis.provider.xai",
  local: "agents.diagnosis.provider.local",
};

export const FAILURE_KEYS: Record<FailureKind, { title: MessageKey; hint: MessageKey }> = {
  auth: { title: "agents.diagnosis.kind.auth.title", hint: "agents.diagnosis.kind.auth.hint" },
  modelAccess: { title: "agents.diagnosis.kind.modelAccess.title", hint: "agents.diagnosis.kind.modelAccess.hint" },
  rateLimit: { title: "agents.diagnosis.kind.rateLimit.title", hint: "agents.diagnosis.kind.rateLimit.hint" },
  quota: { title: "agents.diagnosis.kind.quota.title", hint: "agents.diagnosis.kind.quota.hint" },
  overloaded: { title: "agents.diagnosis.kind.overloaded.title", hint: "agents.diagnosis.kind.overloaded.hint" },
  server: { title: "agents.diagnosis.kind.server.title", hint: "agents.diagnosis.kind.server.hint" },
  badRequest: { title: "agents.diagnosis.kind.badRequest.title", hint: "agents.diagnosis.kind.badRequest.hint" },
  network: { title: "agents.diagnosis.kind.network.title", hint: "agents.diagnosis.kind.network.hint" },
  timeout: { title: "agents.diagnosis.kind.timeout.title", hint: "agents.diagnosis.kind.timeout.hint" },
  unknown: { title: "agents.diagnosis.kind.unknown.title", hint: "agents.diagnosis.kind.unknown.hint" },
};

/** "API de Anthropic · HTTP 403" — who answered, and with what. */
export function failureSource(failure: RunFailure, tr: Translate): string {
  const who =
    failure.provider === "local"
      ? tr(PROVIDER_KEYS.local, { cli: ENGINE_NAMES[failure.engine] })
      : tr(PROVIDER_KEYS[failure.provider], { cli: ENGINE_NAMES[failure.engine] });
  return failure.status ? `${who} · HTTP ${failure.status}` : who;
}

/** Everything about the failure as plain text, to paste in a ticket. */
export function diagnosisText(failure: RunFailure, tr: Translate, at?: number): string {
  const lines = [
    `${tr(FAILURE_KEYS[failure.kind].title)} — ${failureSource(failure, tr)}`,
    `${tr("agents.diagnosis.cli")}: ${ENGINE_NAMES[failure.engine]}`,
  ];
  if (failure.model) lines.push(`${tr("agents.diagnosis.model")}: ${failure.model}`);
  if (failure.requestId) lines.push(`${tr("agents.diagnosis.requestId")}: ${failure.requestId}`);
  if (failure.exitCode !== null && failure.exitCode !== undefined) {
    lines.push(`${tr("agents.diagnosis.exitCode")}: ${failure.exitCode}`);
  }
  if (at) lines.push(`${tr("agents.diagnosis.when")}: ${new Date(at).toISOString()}`);
  if (failure.message) lines.push("", `${tr("agents.diagnosis.message")}:`, failure.message);
  if (failure.stderr.length > 0) lines.push("", "stderr:", ...failure.stderr);
  lines.push("", tr(FAILURE_KEYS[failure.kind].hint));
  return lines.join("\n");
}

/** What ran it, in short pieces: "Claude Code 2.1.290", the model, how it's
 * logged in. */
export function sessionParts(session: SessionInfo, tr: Translate): string[] {
  const parts = [`${ENGINE_NAMES[session.engine]}${session.version ? ` ${session.version}` : ""}`];
  parts.push(session.model || tr("agents.diagnosis.defaultModel"));
  if (session.auth === "subscription") parts.push(tr("agents.diagnosis.auth.subscription"));
  else if (session.auth === "apiKey") {
    parts.push(
      session.authSource
        ? tr("agents.diagnosis.auth.apiKeyFrom", { source: session.authSource })
        : tr("agents.diagnosis.auth.apiKey"),
    );
  }
  return parts;
}

/** The MCP servers that didn't connect: worth saying, they're tools missing. */
export function brokenServers(session: SessionInfo): string[] {
  return session.mcp.filter((server) => server.status && server.status !== "connected").map((server) => server.name);
}

const WINDOW_KEYS: Record<string, MessageKey> = {
  five_hour: "agents.diagnosis.usage.fiveHour",
  seven_day: "agents.diagnosis.usage.sevenDay",
  seven_day_opus: "agents.diagnosis.usage.sevenDayOpus",
};

/** "Uso semanal: 92% · se renueva el lun 12:00" */
export function usageText(usage: UsageInfo, tr: Translate, locale?: string): string {
  const window = WINDOW_KEYS[usage.window] ? tr(WINDOW_KEYS[usage.window]) : usage.window;
  const percent = `${Math.round(usage.utilization * 100)}%`;
  const text =
    usage.status === "rejected"
      ? tr("agents.diagnosis.usage.rejected", { window })
      : tr("agents.diagnosis.usage.warning", { window, percent });
  if (!usage.resetsAt) return text;
  const when = new Date(usage.resetsAt).toLocaleString(locale, { weekday: "short", hour: "2-digit", minute: "2-digit" });
  return `${text} · ${tr("agents.diagnosis.usage.resets", { when })}`;
}
