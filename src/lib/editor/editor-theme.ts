import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { EditorView } from "@codemirror/view";
import type { Extension } from "@codemirror/state";
import { tags } from "@lezer/highlight";
import type { Theme } from "$lib/theme/theme-types";

/**
 * The editor in the colors of the app's theme: its terminal palette for the
 * syntax (so code reads like it does in the console next to it) and its UI
 * colors for the chrome around the text.
 */
export function editorTheme(theme: Theme, font: { family: string; size: number }): Extension {
  const c = theme.colors;
  const ui = theme.ui;
  const dark = isDark(c.background);
  const selection = visibleSelection(c.background, c.selectionBackground, c.blue);

  const chrome = EditorView.theme(
    {
      "&": {
        height: "100%",
        color: c.foreground,
        backgroundColor: "transparent",
        fontSize: `${font.size}px`,
      },
      ".cm-scroller": { fontFamily: font.family, lineHeight: `${theme.font.lineHeight}` },
      ".cm-content": { caretColor: c.cursor },
      ".cm-cursor, .cm-dropCursor": { borderLeftColor: c.cursor },
      // As specific as CodeMirror's own rule for these, or its default
      // (a near-black gray in a dark theme) wins.
      "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionLayer .cm-selectionBackground, .cm-content ::selection":
        {
          backgroundColor: selection,
        },
      ".cm-activeLine": { backgroundColor: withAlpha(c.foreground, 0.035) },
      ".cm-gutters": {
        backgroundColor: "transparent",
        color: c.brightBlack,
        border: "none",
        borderRight: `1px solid ${ui.border}`,
      },
      ".cm-activeLineGutter": { backgroundColor: "transparent", color: c.foreground },
      ".cm-matchingBracket, &.cm-focused .cm-matchingBracket": {
        backgroundColor: withAlpha(c.cyan, 0.2),
        outline: `1px solid ${withAlpha(c.cyan, 0.5)}`,
      },
      ".cm-searchMatch": { backgroundColor: withAlpha(c.yellow, 0.25) },
      ".cm-searchMatch.cm-searchMatch-selected": { backgroundColor: withAlpha(c.yellow, 0.45) },
      ".cm-foldPlaceholder": { backgroundColor: ui.hoverBackground, border: "none", color: c.foreground },
      ".cm-tooltip, .cm-panels": {
        backgroundColor: ui.activeBackground,
        color: ui.activeForeground,
        border: `1px solid ${ui.border}`,
      },
      ".cm-panels input, .cm-panels button": { color: "inherit" },
    },
    { dark },
  );

  const syntax = HighlightStyle.define([
    { tag: [tags.keyword, tags.controlKeyword, tags.moduleKeyword, tags.operatorKeyword], color: c.magenta },
    { tag: [tags.definitionKeyword, tags.modifier, tags.self], color: c.magenta },
    { tag: [tags.string, tags.special(tags.string), tags.regexp], color: c.green },
    { tag: [tags.number, tags.bool, tags.null, tags.atom], color: c.brightYellow },
    { tag: [tags.comment, tags.lineComment, tags.blockComment, tags.docComment], color: c.brightBlack, fontStyle: "italic" },
    { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: c.blue },
    { tag: [tags.typeName, tags.className, tags.namespace], color: c.yellow },
    { tag: [tags.propertyName, tags.attributeName], color: c.cyan },
    { tag: [tags.tagName, tags.angleBracket], color: c.red },
    { tag: [tags.definition(tags.variableName), tags.variableName], color: c.foreground },
    { tag: [tags.operator, tags.punctuation, tags.separator, tags.bracket], color: c.white },
    { tag: [tags.constant(tags.variableName), tags.standard(tags.variableName)], color: c.brightYellow },
    { tag: [tags.special(tags.variableName), tags.macroName], color: c.brightMagenta },
    { tag: [tags.labelName], color: c.yellow },
    { tag: [tags.escape, tags.character], color: c.brightCyan },
    { tag: [tags.color, tags.unit], color: c.brightYellow },
    { tag: [tags.attributeValue, tags.docString], color: c.green },
    { tag: [tags.invalid], color: c.brightRed },
    { tag: [tags.meta, tags.annotation], color: c.brightMagenta },
    { tag: [tags.documentMeta], color: c.brightBlack },
    // Prose: markdown and the like. A quote or a heading styles everything
    // inside it, so they come first: a rule later in the list wins, and what
    // sits inside (code, links, the marks) keeps its own color. The marks
    // around the text (#, **, `, > and list bullets) fade, so what they mark
    // is what reads.
    { tag: [tags.quote], color: c.brightBlack, fontStyle: "italic" },
    { tag: [tags.heading1], color: c.blue, fontWeight: "bold", fontSize: "1.15em" },
    { tag: [tags.heading2], color: c.blue, fontWeight: "bold", fontSize: "1.08em" },
    { tag: [tags.heading], color: c.blue, fontWeight: "bold" },
    { tag: [tags.processingInstruction, tags.contentSeparator], color: c.brightBlack },
    { tag: [tags.monospace], color: c.green },
    { tag: [tags.link], color: c.cyan },
    { tag: [tags.url], color: c.cyan, textDecoration: "underline" },
    { tag: [tags.emphasis], fontStyle: "italic" },
    { tag: [tags.strong], fontWeight: "bold" },
    { tag: [tags.strikethrough], textDecoration: "line-through" },
    // Diffs and patches.
    { tag: [tags.inserted], color: c.green },
    { tag: [tags.deleted], color: c.red },
    { tag: [tags.changed], color: c.yellow },
  ]);

  return [chrome, syntaxHighlighting(syntax)];
}

/** Least contrast between the selection and the background for it to show
 * at a glance; some themes' own (made for a terminal, where selected text
 * often changes color too) sit right next to their background. */
const MIN_SELECTION_CONTRAST = 1.5;

/**
 * The theme's selection color, or, when it's too close to the background,
 * the background mixed with the theme's blue just enough to stand out.
 * Exported to be tested.
 */
export function visibleSelection(background: string, selection: string, accent: string): string {
  const bg = parseHex(background);
  const sel = parseHex(selection);
  const blue = parseHex(accent);
  if (!bg || !sel || !blue) return selection;
  if (contrast(bg, sel) >= MIN_SELECTION_CONTRAST) return selection;
  for (let amount = 0.2; amount <= 0.8; amount += 0.05) {
    const mixed = bg.map((value, i) => Math.round(value + (blue[i] - value) * amount)) as [number, number, number];
    if (contrast(bg, mixed) >= MIN_SELECTION_CONTRAST) return toHex(mixed);
  }
  return withAlpha(accent, 0.45);
}

/** WCAG's contrast ratio between two colors. */
function contrast(a: [number, number, number], b: [number, number, number]): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

function luminance(rgb: [number, number, number]): number {
  const [r, g, b] = rgb.map((value) => {
    const v = value / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function toHex(rgb: [number, number, number]): string {
  return `#${rgb.map((value) => value.toString(16).padStart(2, "0")).join("")}`;
}

/** Whether a #rrggbb color is on the dark side, which CodeMirror uses to
 * pick its own defaults for anything the theme doesn't set. */
function isDark(hex: string): boolean {
  const rgb = parseHex(hex);
  if (!rgb) return true;
  return (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]) / 255 < 0.5;
}

function withAlpha(hex: string, alpha: number): string {
  const rgb = parseHex(hex);
  return rgb ? `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})` : hex;
}

function parseHex(hex: string): [number, number, number] | null {
  const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!match) return null;
  const value = parseInt(match[1], 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}
