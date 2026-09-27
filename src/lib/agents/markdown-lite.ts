/**
 * Just enough Markdown for an agent's answers: **bold**, *italics*, `code`,
 * headings and bullets. Everything is HTML-escaped first, so the output is
 * safe for {@html} whatever the agent wrote; anything this doesn't know stays
 * as plain text.
 */
export function renderMarkdownLite(text: string): string {
  const escaped = text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  return escaped
    .split("\n")
    .map((line) => {
      const heading = /^#{1,6}\s+(.*)$/.exec(line);
      if (heading) return `<strong>${inline(heading[1])}</strong>`;
      const bullet = /^(\s*)[-*]\s+(.*)$/.exec(line);
      if (bullet) return `${bullet[1]}• ${inline(bullet[2])}`;
      return inline(line);
    })
    .join("\n");
}

function inline(line: string): string {
  // Code first, so what's inside it isn't read as emphasis.
  const parts = line.split(/(`[^`]+`)/g);
  return parts
    .map((part) => {
      if (/^`[^`]+`$/.test(part)) return `<code>${part.slice(1, -1)}</code>`;
      return part.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>").replace(/(^|[^*])\*([^*\s][^*]*)\*/g, "$1<em>$2</em>");
    })
    .join("");
}
