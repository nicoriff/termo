/**
 * What has to be asked before an MCP server can be installed: header values
 * for a remote, or environment variables for a stdio package. Pure — no
 * runes, no Tauri — so it's unit-testable in plain Node, same approach as
 * `filter.ts`.
 *
 * Mirrors `install.rs`'s "first remote, else first package" choice exactly:
 * that's what actually gets installed, so asking about anything else would
 * collect values nothing ever uses.
 */

import type { McpDetail } from "$lib/core/marketplace-client";

export interface ParamSpec {
  name: string;
  description?: string;
  required: boolean;
  secret: boolean;
  /** A real prefillable value — never the registry's own placeholder text
   * for a secret field ("Bearer {api_key}"), which reads like a working
   * value but isn't one. */
  default?: string;
}

/** The parameters to collect before installing this entry. */
export function paramsFor(detail: McpDetail): ParamSpec[] {
  const remote = detail.remotes[0];
  if (remote) {
    const headers = (remote.headers ?? []).map((header) => ({
      name: header.name,
      description: header.description,
      required: Boolean(header.isRequired),
      secret: Boolean(header.isSecret),
      // A registry's `value` for a header is a template; a source that gives
      // a real default says so as `default`.
      default: header.isSecret ? undefined : ((header as { default?: string }).default ?? header.value),
    }));
    // What goes in the address rather than in a header.
    const query = (remote.queryParams ?? []).map((param) => ({
      name: param.name,
      description: param.description,
      required: Boolean(param.isRequired),
      secret: Boolean(param.isSecret),
      default: param.isSecret ? undefined : param.default,
    }));
    return [...headers, ...query];
  }
  const pkg = detail.packages[0];
  if (pkg) {
    return (pkg.environmentVariables ?? []).map((variable) => ({
      name: variable.name,
      description: variable.description,
      required: Boolean(variable.isRequired),
      secret: Boolean(variable.isSecret),
      default: variable.isSecret ? undefined : variable.default,
    }));
  }
  return [];
}

/** The values to start a fresh form with: every param that has a real
 * (non-secret) default pre-filled, everything else blank. */
export function initialParamValues(specs: ParamSpec[]): Record<string, string> {
  return Object.fromEntries(specs.filter((spec) => spec.default !== undefined).map((spec) => [spec.name, spec.default as string]));
}

/** Whether every required parameter has a non-empty value. Install stays
 * disabled until this is true, so a required secret is never sent empty. */
export function paramsSatisfied(specs: ParamSpec[], values: Record<string, string>): boolean {
  return specs.every((spec) => !spec.required || Boolean(values[spec.name]?.trim()));
}

/** A short, safe-to-show summary of what will actually be installed — real
 * non-secret values, a mask for secret ones, never the registry's own
 * placeholder text passed off as a value. */
export function previewLines(detail: McpDetail, values: Record<string, string>): string[] {
  const remote = detail.remotes[0];
  if (remote) {
    const lines = [`${remote.type ?? "http"} ${remote.url}`];
    for (const header of remote.headers ?? []) {
      const value = values[header.name]?.trim();
      lines.push(`${header.name}: ${header.isSecret ? (value ? "••••••" : "—") : (value ?? "—")}`);
    }
    for (const param of remote.queryParams ?? []) {
      const value = values[param.name]?.trim();
      lines.push(`?${param.name}=${param.isSecret ? (value ? "••••••" : "—") : (value ?? "—")}`);
    }
    return lines;
  }
  const pkg = detail.packages[0];
  if (pkg) {
    const lines = [`${pkg.registryType} ${pkg.identifier}${pkg.version ? `@${pkg.version}` : ""}`];
    for (const variable of pkg.environmentVariables ?? []) {
      const value = values[variable.name]?.trim();
      lines.push(`${variable.name}: ${variable.isSecret ? (value ? "••••••" : "—") : (value ?? "—")}`);
    }
    return lines;
  }
  return [];
}
