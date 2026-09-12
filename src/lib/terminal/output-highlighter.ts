/**
 * Colors terminal OUTPUT after the fact by injecting ANSI SGR codes around
 * pattern matches (IPs, URLs, paths, sizes, timestamps, error/warning/success
 * keywords) before the text reaches xterm — the same technique tools like
 * `grc`/`ccze` use to colorize piped command output. This can't touch what
 * the user is TYPING (that's rendered by the shell itself, e.g. PSReadLine),
 * only text the shell already sent back.
 *
 * Each match is wrapped with a foreground-only color (`38;5;N`) and reset
 * with `39` (foreground reset only, not a full reset) so we don't clobber
 * bold/background the shell may already have set around the match.
 */

interface HighlightRule {
  key: string;
  colorCode: number;
  pattern: string;
  /**
   * Rejects a match whose immediately preceding character matches this —
   * i.e. a hand-rolled negative lookbehind. Written this way because the
   * WebKit builds Tauri renders in (WKWebView on macOS, WebKitGTK on Linux)
   * only gained lookbehind support in Safari 16.4; on anything older
   * `new RegExp` throws at module scope, which takes the whole app down to
   * a blank window. Chromium (WebView2 on Windows) has no such limit, so
   * this only ever failed off-Windows.
   */
  notPrecededBy?: RegExp;
}

const RULES: HighlightRule[] = [
  { key: "url", colorCode: 75, pattern: String.raw`https?:\/\/[^\s"'<>]+` },
  { key: "ip", colorCode: 81, pattern: String.raw`(?:\d{1,3}\.){3}\d{1,3}(?::\d+)?` },
  {
    key: "timestamp",
    colorCode: 244,
    pattern: String.raw`\d{4}-\d{2}-\d{2}[ T]\d{2}:\d{2}:\d{2}(?:\.\d+)?|\b\d{1,2}:\d{2}:\d{2}\b`,
  },
  { key: "winPath", colorCode: 176, pattern: String.raw`[A-Za-z]:\\[^\s"'<>|]+` },
  {
    // Absolute POSIX paths only: the preceding-character check keeps the
    // tail of a relative path (`src/lib/foo`) or of something URL-shaped
    // from lighting up as a path of its own.
    key: "unixPath",
    colorCode: 176,
    pattern: String.raw`\/[\w.\-]+(?:\/[\w.\-]+)+`,
    notPrecededBy: /[\w./]/,
  },
  { key: "size", colorCode: 222, pattern: String.raw`\b\d+(?:\.\d+)?\s?(?:B|KB|MB|GB|TB|%)\b` },
  { key: "error", colorCode: 203, pattern: String.raw`\b(?:error|fail(?:ed)?|fatal|exception)\b` },
  { key: "warning", colorCode: 214, pattern: String.raw`\b(?:warn(?:ing)?|deprecated)\b` },
  { key: "success", colorCode: 114, pattern: String.raw`\b(?:success(?:ful)?|done|completed|passed)\b` },
];

const COMBINED_PATTERN = new RegExp(RULES.map((r) => `(?<${r.key}>${r.pattern})`).join("|"), "gi");

// Matches ANSI escape sequences (CSI, OSC, and simple two-byte ESC forms) so
// they can be passed through untouched — running the highlight patterns over
// them could match digits/letters inside an escape code and corrupt it.
const ANSI_SEQUENCE = /\x1b(?:\[[0-?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[@-Z\\-_])/g;

function colorizePlainText(text: string): string {
  if (!text) return text;
  let result = "";
  let lastIndex = 0;
  for (const match of text.matchAll(COMBINED_PATTERN)) {
    const rule = RULES.find((r) => match.groups?.[r.key] !== undefined);
    if (!rule || match.index === undefined) continue;
    if (rule.notPrecededBy && match.index > 0 && rule.notPrecededBy.test(text[match.index - 1])) {
      continue;
    }
    result += text.slice(lastIndex, match.index);
    result += `\x1b[38;5;${rule.colorCode}m${match[0]}\x1b[39m`;
    lastIndex = match.index + match[0].length;
  }
  result += text.slice(lastIndex);
  return result;
}

/** Highlights a chunk of decoded PTY output text, leaving any ANSI escape
 * sequences already present in it untouched. */
export function highlightChunk(text: string): string {
  if (!text) return text;
  let result = "";
  let lastIndex = 0;
  for (const match of text.matchAll(ANSI_SEQUENCE)) {
    if (match.index === undefined) continue;
    result += colorizePlainText(text.slice(lastIndex, match.index)) + match[0];
    lastIndex = match.index + match[0].length;
  }
  result += colorizePlainText(text.slice(lastIndex));
  return result;
}

// Anchored copy of ANSI_SEQUENCE for testing "does a complete escape
// sequence start right here", used below to find a trailing partial one.
const ANSI_SEQUENCE_AT_START = new RegExp(`^(?:${ANSI_SEQUENCE.source})`);

// A real CSI/OSC sequence is short; if a trailing `\x1b` still hasn't
// resolved into a complete sequence after this many characters it's not
// going to (malformed, or not actually an escape sequence) — stop holding
// it back so a fluke ESC byte can't stall highlighting indefinitely.
const MAX_PENDING_TAIL = 64;

/** Streaming wrapper around highlightChunk: PTY reads land as arbitrary
 * byte chunks that don't respect escape-sequence boundaries, so a sequence
 * can arrive split across two onData calls. highlightChunk alone can't see
 * that — it treats each chunk in isolation, so the tail fragment of a split
 * sequence (missing its leading `\x1b[`) reads as plain text and can match
 * one of the highlight rules by coincidence (e.g. a split cursor-down
 * sequence's "1B" tail matches the size rule's `\d+B`), get wrapped in
 * injected color codes, and corrupt the sequence xterm was expecting to
 * reassemble — visibly as a misplaced cursor in redraw-heavy TUIs (claude).
 * This holds back a trailing incomplete-looking escape sequence instead of
 * running it through the rules, and prepends it to the next chunk. */
export class OutputHighlighter {
  private pending = "";

  feed(text: string): string {
    const combined = this.pending + text;
    this.pending = "";
    if (!combined) return combined;

    const lastEsc = combined.lastIndexOf("\x1b");
    if (lastEsc === -1) return highlightChunk(combined);

    const tail = combined.slice(lastEsc);
    if (ANSI_SEQUENCE_AT_START.test(tail) || tail.length > MAX_PENDING_TAIL) {
      // Either already a complete, self-contained sequence right there
      // (whatever comes after it is irrelevant to whether *it's* whole),
      // or a tail that's given up any hope of completing — safe either way.
      return highlightChunk(combined);
    }

    // No complete sequence starts here yet — hold the tail back for the
    // next chunk instead of letting the rules see (and possibly corrupt) a
    // fragment of an escape sequence.
    this.pending = tail;
    return highlightChunk(combined.slice(0, lastEsc));
  }
}
