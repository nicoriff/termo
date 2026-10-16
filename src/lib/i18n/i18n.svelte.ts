import { invoke } from "@tauri-apps/api/core";
import { en } from "./messages/en";
import { es } from "./messages/es";
import {
  interpolate,
  lookup,
  resolveLocale,
  type LanguagePreference,
  type Locale,
  type MessageKey,
  type MessageParams,
  type Messages,
} from "./translate";

export type { LanguagePreference, Locale, MessageKey } from "./translate";

const CATALOGS: Record<Locale, Messages> = { es, en };

class I18nStore {
  preference = $state<LanguagePreference>("system");
  locale = $derived(resolveLocale(this.preference, navigator.language));

  setPreference(preference: LanguagePreference): void {
    this.preference = preference;
  }
}

export const i18n = new I18nStore();

/** Translates `key` into the current locale. Reads reactive state, so calls in
 * templates and `$derived` re-run when the language changes. */
export function t(key: MessageKey, params?: MessageParams): string {
  const text = lookup(CATALOGS[i18n.locale], key);
  return params ? interpolate(text, params) : text;
}

/** Names as a list in the current locale: "A, B y C", "A, B and C". */
export function listOf(items: string[]): string {
  return new Intl.ListFormat(i18n.locale, { style: "long", type: "conjunction" }).format(items);
}

/** Keeps `<html lang>` and the backend's language (Rust-side messages and the
 * Preferences window title) in sync with the resolved locale. Each window
 * calls this once; returns a teardown function. */
export function startI18nSync(): () => void {
  return $effect.root(() => {
    $effect(() => {
      const locale = i18n.locale;
      document.documentElement.lang = locale;
      void invoke("set_ui_language", { language: locale }).catch(() => {});
    });
  });
}
