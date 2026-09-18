/**
 * Colors terminal OUTPUT after the fact by injecting ANSI SGR codes around
 * pattern matches (IPs, URLs, paths, sizes, timestamps, error/warning/success
 * keywords) before the text reaches xterm — the same technique tools like
 * `grc`/`ccze` use to colorize piped command output. This can't touch what
 * the user is TYPING (that's rendered by the shell itself, e.g. PSReadLine),
 * only text the shell already sent back.
 *
 * Each match is wrapped with a foreground-only color (`38;5;N`) so we don't
 * clobber bold or background the shell may already have set around it, and
 * closed by restoring whatever foreground the shell itself had in effect —
 * tracked across the whole stream, since a color set in one chunk stays in
 * effect for the next. Closing with `39` instead would look right only when
 * the shell happened to be at its default: `39` doesn't mean "put back what
 * was there", it means "go to the terminal default", so the rest of the
 * shell's own line would lose its color from the match onwards.
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

// Matches ANSI escape sequences (CSI, the string-introducer families, and
// simple two-byte ESC forms) so they can be passed through untouched —
// running the highlight patterns over them could match digits/letters inside
// an escape code and corrupt it.
//
// The middle branch covers everything that introduces a *string* running to
// BEL or ST: OSC (`]`), DCS (`P`), PM (`^`) and APC (`_`). They must stay out
// of the two-byte branch, which is why that one skips 0x50 and 0x5B-0x5F
// rather than running `@` straight through `_`. Letting `ESC ]` pass as a
// complete two-byte sequence made an OSC that hadn't arrived whole look
// finished, so feed() handed its payload to the rules instead of holding it
// back — and since OSC 0 (window title) and OSC 7 (cwd) both carry a path,
// the path rule then injected color codes inside the sequence. xterm reads
// the injected ESC as the end of the string and prints the rest of the path
// onto the prompt line as ordinary text.
const ANSI_SEQUENCE = /\x1b(?:\[[0-?]*[ -/]*[@-~]|[\]P^_][^\x07\x1b]*(?:\x07|\x1b\\)|[@-OQ-Z\\])/g;

/** The SGR sequence that reinstates the shell's own foreground, or `\x1b[39m`
 * when it really was sitting at the terminal default. `null` means default. */
function restoreForeground(foreground: string | null): string {
  return foreground ?? "\x1b[39m";
}

/** Reads an SGR sequence for the foreground it leaves behind, so an injected
 * span can hand that exact color back afterwards. Anything that isn't an SGR
 * sequence leaves the foreground alone. */
function foregroundAfter(sequence: string, current: string | null): string | null {
  const body = /^\x1b\[([0-?]*)m$/.exec(sequence);
  if (!body) return current;
  // `ESC[m` is `ESC[0m`: a full reset, default foreground included.
  const params = body[1] === "" ? ["0"] : body[1].split(";");
  let foreground = current;
  for (let i = 0; i < params.length; i++) {
    const code = Number(params[i]);
    if (code === 0 || code === 39) {
      foreground = null;
    } else if ((code >= 30 && code <= 37) || (code >= 90 && code <= 97)) {
      foreground = `\x1b[${code}m`;
    } else if (code === 38 || code === 48) {
      // 5 = 256-colour (one more param), 2 = truecolour (three more). Both
      // families share this shape; only 38 sets the foreground, but 48 has to
      // be stepped over the same way or its arguments read as colour codes.
      const span = Number(params[i + 1]) === 5 ? 2 : Number(params[i + 1]) === 2 ? 4 : 0;
      if (code === 38 && span > 0) {
        foreground = `\x1b[${params.slice(i, i + span + 1).join(";")}m`;
      }
      i += span;
    }
  }
  return foreground;
}

function colorizePlainText(text: string, foreground: string | null): string {
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
    result += `\x1b[38;5;${rule.colorCode}m${match[0]}${restoreForeground(foreground)}`;
    lastIndex = match.index + match[0].length;
  }
  result += text.slice(lastIndex);
  return result;
}

/** Highlights a chunk of decoded PTY output text, leaving any ANSI escape
 * sequences already present in it untouched. `foreground` is the colour the
 * shell had in effect when the chunk starts; the colour it leaves in effect
 * comes back out, for the next chunk to carry on from. */
export function highlightChunk(text: string, foreground: string | null = null): { text: string; foreground: string | null } {
  if (!text) return { text, foreground };
  let result = "";
  let lastIndex = 0;
  for (const match of text.matchAll(ANSI_SEQUENCE)) {
    if (match.index === undefined) continue;
    result += colorizePlainText(text.slice(lastIndex, match.index), foreground) + match[0];
    foreground = foregroundAfter(match[0], foreground);
    lastIndex = match.index + match[0].length;
  }
  result += colorizePlainText(text.slice(lastIndex), foreground);
  return { text: result, foreground };
}

// Anchored copy of ANSI_SEQUENCE for testing "does a complete escape
// sequence start right here", used below to find a trailing partial one.
const ANSI_SEQUENCE_AT_START = new RegExp(`^(?:${ANSI_SEQUENCE.source})`);

// If a trailing `\x1b` still hasn't resolved into a complete sequence after
// this many characters it's not going to (malformed, or not actually an
// escape sequence) — stop holding it back so a fluke ESC byte can't stall
// highlighting indefinitely. Sized for the longest sequence a shell
// realistically emits, which is an OSC carrying a full path (the window
// title and the cwd report): a deep directory alone can run past a couple
// hundred characters, and giving up early hands that path to the rules with
// its introducer still unterminated — the corruption ANSI_SEQUENCE's middle
// branch exists to prevent.
const MAX_PENDING_TAIL = 1024;

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
  /** The foreground the shell has in effect right now; an SGR from one chunk
   * still applies to the next, so this has to outlive a single call. */
  private foreground: string | null = null;

  private run(text: string): string {
    const result = highlightChunk(text, this.foreground);
    this.foreground = result.foreground;
    return result.text;
  }

  feed(text: string): string {
    const combined = this.pending + text;
    this.pending = "";
    if (!combined) return combined;

    const lastEsc = combined.lastIndexOf("\x1b");
    if (lastEsc === -1) return this.run(combined);

    const tail = combined.slice(lastEsc);
    if (ANSI_SEQUENCE_AT_START.test(tail) || tail.length > MAX_PENDING_TAIL) {
      // Either already a complete, self-contained sequence right there
      // (whatever comes after it is irrelevant to whether *it's* whole),
      // or a tail that's given up any hope of completing — safe either way.
      return this.run(combined);
    }

    // No complete sequence starts here yet — hold the tail back for the
    // next chunk instead of letting the rules see (and possibly corrupt) a
    // fragment of an escape sequence.
    this.pending = tail;
    return this.run(combined.slice(0, lastEsc));
  }
}
