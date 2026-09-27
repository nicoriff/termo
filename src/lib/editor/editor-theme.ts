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
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
        backgroundColor: c.selectionBackground,
      },
      ".cm-activeLine": { backgroundColor: withAlpha(c.foreground, 0.05) },
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
    { tag: [tags.heading], color: c.blue, fontWeight: "bold" },
    { tag: [tags.link, tags.url], color: c.cyan, textDecoration: "underline" },
    { tag: [tags.emphasis], fontStyle: "italic" },
    { tag: [tags.strong], fontWeight: "bold" },
    { tag: [tags.invalid], color: c.brightRed },
    { tag: [tags.meta, tags.annotation], color: c.brightMagenta },
  ]);

  return [chrome, syntaxHighlighting(syntax)];
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
