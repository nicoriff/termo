/**
 * Fuzzy text matching shared by the command palette and the shortcut search.
 * Pure — no runes, no Tauri, no i18n store — so it can be unit-tested in
 * plain Node.
 */

/** Lowercased and accent-free, so "seleccion" also matches "Selección". */
export function normalize(text: string): string {
  return text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

const STARTS_WITH = 0;
const WORD_START = 1;
const CONTAINS = 2;
const SUBSEQUENCE = 3;

/** Lower is a better match; `null` means the query doesn't match at all.
 * Subsequence is what makes initials work: "gtp" finds "Go to Preferences". */
export function scoreMatch(label: string, query: string): number | null {
  const text = normalize(label);
  const needle = normalize(query).trim();
  if (needle === "") return STARTS_WITH;

  if (text.startsWith(needle)) return STARTS_WITH;
  const at = text.indexOf(needle);
  if (at > 0) return text[at - 1] === " " ? WORD_START : CONTAINS;

  let cursor = 0;
  for (const char of needle) {
    cursor = text.indexOf(char, cursor);
    if (cursor === -1) return null;
    cursor++;
  }
  return SUBSEQUENCE;
}
