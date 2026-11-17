import { Marked, type Tokens } from "marked";

/**
 * A Markdown file to HTML for the viewer tab: GitHub's flavour (tables, task
 * lists, strikethrough), headings with ids to jump to.
 *
 * Safe for {@html} whatever the file says — it's an agent's, not ours. HTML
 * written in it shows as the text it is, a link carries no `href` (the
 * viewer opens it on click, see MarkdownView), and an image only loads from
 * the web or a data: address, never from this computer.
 *
 * Pure (no runes), so it's tested in plain Node.
 */
export function renderMarkdownDocument(text: string): string {
  const slugs = new Map<string, number>();
  const marked = new Marked({
    gfm: true,
    async: false,
    renderer: {
      html({ text }: Tokens.HTML | Tokens.Tag): string {
        return escape(text);
      },
      heading({ tokens, depth }: Tokens.Heading): string {
        const inner = this.parser.parseInline(tokens);
        const id = uniqueSlug(slugify(plain(inner)), slugs);
        return `<h${depth} id="${id}">${inner}</h${depth}>\n`;
      },
      link({ href, title, tokens }: Tokens.Link): string {
        const inner = this.parser.parseInline(tokens);
        const tip = title ? ` title="${escape(title)}"` : ` title="${escape(href)}"`;
        return `<a data-href="${escape(href)}"${tip} tabindex="0" role="link">${inner}</a>`;
      },
      image({ href, title, text }: Tokens.Image): string {
        if (!/^(https:|data:image\/)/i.test(href)) {
          return `<span class="md-image-missing" title="${escape(href)}">${escape(text || href)}</span>`;
        }
        const tip = title ? ` title="${escape(title)}"` : "";
        return `<img src="${escape(href)}" alt="${escape(text)}"${tip} loading="lazy">`;
      },
    },
  });
  return marked.parse(text.replace(/^﻿/, "")) as string;
}

/** Where a link in the document goes: within it, the web, or another file
 * (relative to the document's folder, read by the viewer). */
export type LinkTarget =
  | { kind: "anchor"; id: string }
  | { kind: "web"; url: string }
  | { kind: "file"; path: string; anchor?: string }
  | { kind: "none" };

export function linkTarget(href: string): LinkTarget {
  const trimmed = href.trim();
  if (trimmed.startsWith("#")) return { kind: "anchor", id: decode(trimmed.slice(1)) };
  if (/^(https?:|mailto:)/i.test(trimmed)) return { kind: "web", url: trimmed };
  // Any other scheme (javascript:, file:, a custom one) goes nowhere.
  if (/^[a-z][\w+.-]*:/i.test(trimmed) && !/^[a-z]:[\\/]/i.test(trimmed)) return { kind: "none" };
  if (trimmed === "") return { kind: "none" };
  const hash = trimmed.indexOf("#");
  const path = decode(hash === -1 ? trimmed : trimmed.slice(0, hash));
  const anchor = hash === -1 ? undefined : decode(trimmed.slice(hash + 1));
  return anchor ? { kind: "file", path, anchor } : { kind: "file", path };
}

/** As GitHub makes a heading's id: lowercase, punctuation out, spaces to dashes. */
export function slugify(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
}

function uniqueSlug(slug: string, seen: Map<string, number>): string {
  const count = seen.get(slug) ?? 0;
  seen.set(slug, count + 1);
  return count === 0 ? slug : `${slug}-${count}`;
}

function plain(html: string): string {
  return html
    .replace(/<[^>]*>/g, "")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, "&");
}

function decode(text: string): string {
  try {
    return decodeURIComponent(text);
  } catch {
    return text;
  }
}

function escape(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
