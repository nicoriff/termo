// Google Fonts monospace families that Nerd Fonts also ships a patched
// (icon-carrying) version of. Built by matching Google's monospace list
// against nerd-fonts' bin/scripts/lib/fonts.json `unpatchedName`s, plus
// Overpass Mono, whose Nerd Font lives in the "Overpass" family's Mono
// folder. Names are Google's exact family names, as the picker lists them.
const NERD_FONT_FAMILIES = new Set([
  "Anonymous Pro",
  "Cousine",
  "Fira Code",
  "Fira Mono",
  "Geist Mono",
  "Google Sans Code",
  "IBM Plex Mono",
  "Inconsolata",
  "Intel One Mono",
  "JetBrains Mono",
  "Lekton",
  "Lilex",
  "Martian Mono",
  "Overpass Mono",
  "Roboto Mono",
  "Share Tech Mono",
  "Source Code Pro",
  "Space Mono",
  "Ubuntu Mono",
  "Victor Mono",
]);

export function hasNerdFontVersion(family: string): boolean {
  return NERD_FONT_FAMILIES.has(family);
}
