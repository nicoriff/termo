import type { ThemeAnsiColors } from "$lib/theme/theme-types";

/** A file's badge in the tree: a short tag for its kind, tinted with one of
 * the theme's ANSI colors so it follows the palette. */
export interface FileBadge {
  label: string;
  color: keyof ThemeAnsiColors;
}

const BY_EXTENSION: Record<string, FileBadge> = {
  ts: { label: "TS", color: "blue" },
  tsx: { label: "TS", color: "blue" },
  mts: { label: "TS", color: "blue" },
  cts: { label: "TS", color: "blue" },
  js: { label: "JS", color: "yellow" },
  jsx: { label: "JS", color: "yellow" },
  mjs: { label: "JS", color: "yellow" },
  cjs: { label: "JS", color: "yellow" },
  svelte: { label: "SV", color: "red" },
  vue: { label: "VU", color: "green" },
  rs: { label: "RS", color: "brightRed" },
  py: { label: "PY", color: "blue" },
  go: { label: "GO", color: "cyan" },
  java: { label: "JV", color: "red" },
  kt: { label: "KT", color: "magenta" },
  cs: { label: "C#", color: "magenta" },
  c: { label: "C", color: "blue" },
  h: { label: "H", color: "blue" },
  cpp: { label: "C+", color: "blue" },
  hpp: { label: "H+", color: "blue" },
  rb: { label: "RB", color: "red" },
  php: { label: "PH", color: "magenta" },
  swift: { label: "SW", color: "brightRed" },
  json: { label: "{}", color: "yellow" },
  jsonc: { label: "{}", color: "yellow" },
  toml: { label: "TM", color: "brightBlack" },
  yaml: { label: "YM", color: "magenta" },
  yml: { label: "YM", color: "magenta" },
  xml: { label: "<>", color: "brightYellow" },
  html: { label: "<>", color: "brightRed" },
  htm: { label: "<>", color: "brightRed" },
  css: { label: "#", color: "brightBlue" },
  scss: { label: "#", color: "magenta" },
  less: { label: "#", color: "brightBlue" },
  md: { label: "MD", color: "cyan" },
  mdx: { label: "MD", color: "cyan" },
  txt: { label: "TX", color: "brightBlack" },
  sh: { label: "$", color: "green" },
  bash: { label: "$", color: "green" },
  zsh: { label: "$", color: "green" },
  ps1: { label: ">_", color: "brightBlue" },
  psm1: { label: ">_", color: "brightBlue" },
  bat: { label: ">_", color: "green" },
  cmd: { label: ">_", color: "green" },
  sql: { label: "DB", color: "brightYellow" },
  lock: { label: "LK", color: "brightBlack" },
  svg: { label: "SV", color: "brightYellow" },
  png: { label: "IM", color: "magenta" },
  jpg: { label: "IM", color: "magenta" },
  jpeg: { label: "IM", color: "magenta" },
  gif: { label: "IM", color: "magenta" },
  ico: { label: "IM", color: "magenta" },
  icns: { label: "IM", color: "magenta" },
  webp: { label: "IM", color: "magenta" },
  wav: { label: "♪", color: "cyan" },
  mp3: { label: "♪", color: "cyan" },
  ogg: { label: "♪", color: "cyan" },
};

/** Whole names that say more than their extension (or have none). */
const BY_NAME: Record<string, FileBadge> = {
  "package.json": { label: "NP", color: "red" },
  "package-lock.json": { label: "LK", color: "brightBlack" },
  "cargo.toml": { label: "RS", color: "brightRed" },
  "cargo.lock": { label: "LK", color: "brightBlack" },
  dockerfile: { label: "DK", color: "blue" },
  makefile: { label: "MK", color: "brightBlack" },
  license: { label: "LC", color: "brightBlack" },
  ".gitignore": { label: "GI", color: "brightRed" },
  ".gitattributes": { label: "GI", color: "brightRed" },
  ".env": { label: "EN", color: "brightYellow" },
};

/** The badge for a file name, or null to draw the plain document icon. */
export function fileBadge(name: string): FileBadge | null {
  const lower = name.toLowerCase();
  if (BY_NAME[lower]) return BY_NAME[lower];
  if (lower.startsWith(".env.")) return BY_NAME[".env"];
  const dot = lower.lastIndexOf(".");
  if (dot <= 0) return null;
  return BY_EXTENSION[lower.slice(dot + 1)] ?? null;
}
