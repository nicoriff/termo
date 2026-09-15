/**
 * Pure i18n helpers (no Svelte runes, no Tauri), so they can be unit-tested
 * in plain Node. The reactive store lives in `i18n.svelte.ts`.
 */
import type { es } from "./messages/es";

export type LanguagePreference = "system" | "es" | "en";
export type Locale = "es" | "en";

export type Messages = typeof es;

type Paths<T, Prefix extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${Prefix}${K}` : Paths<T[K], `${Prefix}${K}.`>;
}[keyof T & string];

/** Dotted path to any string in the catalogs, e.g. "preferences.font.size". */
export type MessageKey = Paths<Messages>;

export type MessageParams = Record<string, string | number>;

/** "system" follows the OS UI language as reported by the webview: Spanish
 * for any `es-*` locale, English otherwise. */
export function resolveLocale(preference: LanguagePreference, systemLanguage: string | undefined): Locale {
  if (preference !== "system") return preference;
  return systemLanguage?.toLowerCase().startsWith("es") ? "es" : "en";
}

export function lookup(catalog: Messages, key: MessageKey): string {
  let node: unknown = catalog;
  for (const part of key.split(".")) node = (node as Record<string, unknown>)[part];
  return typeof node === "string" ? node : key;
}

/** Replaces `{name}` placeholders; unknown placeholders are left as-is. */
export function interpolate(template: string, params: MessageParams): string {
  return template.replace(/\{(\w+)\}/g, (match, name: string) => (name in params ? String(params[name]) : match));
}

/** Every leaf path of a catalog, used to check both languages stay in sync. */
export function collectKeys(node: unknown, prefix = ""): string[] {
  if (typeof node === "string") return [prefix];
  return Object.entries(node as Record<string, unknown>).flatMap(([key, value]) =>
    collectKeys(value, prefix ? `${prefix}.${key}` : key),
  );
}
