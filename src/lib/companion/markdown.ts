/**
 * Markdown as a model writes it in an answer, to HTML: paragraphs, headings,
 * lists, quotes, fenced code, and inline code, bold, italics, strikethrough
 * and links. Everything is HTML-escaped before any tag is added, so the
 * result is safe for {@html} whatever the answer says.
 *
 * Not a whole Markdown: what it doesn't know (a table, an image) stays as
 * the text it was written as, which still reads.
 */
export function renderMarkdown(text: string): string {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const html: string[] = [];
  let paragraph: string[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  let quote: string[] = [];

  const flush = () => {
    if (paragraph.length > 0) html.push(`<p>${paragraph.map(inline).join("<br>")}</p>`);
    if (list) {
      const tag = list.ordered ? "ol" : "ul";
      html.push(`<${tag}>${list.items.map((item) => `<li>${inline(item)}</li>`).join("")}</${tag}>`);
    }
    if (quote.length > 0) html.push(`<blockquote>${quote.map(inline).join("<br>")}</blockquote>`);
    paragraph = [];
    list = null;
    quote = [];
  };

  for (let index = 0; index < lines.length; index++) {
    const line = lines[index];

    const fence = /^\s*(```|~~~)/.exec(line);
    if (fence) {
      flush();
      const code: string[] = [];
      // To the closing fence, or the end when the answer was cut short.
      for (index++; index < lines.length && !lines[index].trimStart().startsWith(fence[1]); index++) code.push(lines[index]);
      html.push(`<pre><code>${escape(code.join("\n"))}</code></pre>`);
      continue;
    }

    if (line.trim() === "") {
      flush();
      continue;
    }

    const heading = /^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$/.exec(line);
    if (heading) {
      flush();
      html.push(`<h4>${inline(heading[1])}</h4>`);
      continue;
    }

    if (/^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(line)) {
      flush();
      html.push("<hr>");
      continue;
    }

    const item = /^\s*(?:([-*+])|(\d+)[.)])\s+(.*)$/.exec(line);
    if (item) {
      const ordered = item[2] !== undefined;
      if (!list || list.ordered !== ordered) {
        flush();
        list = { ordered, items: [] };
      }
      list.items.push(item[3]);
      continue;
    }

    const quoted = /^\s{0,3}>\s?(.*)$/.exec(line);
    if (quoted) {
      if (quote.length === 0) flush();
      quote.push(quoted[1]);
      continue;
    }

    // An indented line under a list item goes on with it.
    if (list && /^\s+\S/.test(line)) {
      list.items[list.items.length - 1] += ` ${line.trim()}`;
      continue;
    }

    if (list || quote.length > 0) flush();
    paragraph.push(line.trim());
  }
  flush();
  return html.join("");
}

function escape(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/** Stands in for a code span while the rest of its line is read, so what's
 * inside one is never taken for emphasis. Not a character an answer has. */
const HELD = "\u0000";

function inline(line: string): string {
  const codes: string[] = [];
  const held = escape(line).replace(/(`+)(.+?)\1/g, (_, __, code: string) => {
    codes.push(`<code>${code.trim()}</code>`);
    return `${HELD}${codes.length - 1}${HELD}`;
  });
  return held
    .replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, '<span class="link" title="$2">$1</span>')
    .replace(/\*\*(?=\S)(.+?)(?<=\S)\*\*/g, "<strong>$1</strong>")
    .replace(/(?<![\w_])__(?=\S)(.+?)(?<=\S)__(?![\w_])/g, "<strong>$1</strong>")
    .replace(/(?<![\w*])\*(?=[^\s*])(.+?)(?<=[^\s*])\*(?![\w*])/g, "<em>$1</em>")
    .replace(/(?<![\w_])_(?=[^\s_])(.+?)(?<=[^\s_])_(?![\w_])/g, "<em>$1</em>")
    .replace(/~~(?=\S)(.+?)(?<=\S)~~/g, "<del>$1</del>")
    .replace(new RegExp(`${HELD}(\\d+)${HELD}`, "g"), (_, at: string) => codes[Number(at)]);
}
