import { describe, expect, it } from "vitest";
import { renderMarkdown } from "./markdown";

describe("an answer's Markdown", () => {
  it("reads emphasis, also around code and next to punctuation", () => {
    expect(renderMarkdown("Es **importante** y *sutil*.")).toBe("<p>Es <strong>importante</strong> y <em>sutil</em>.</p>");
    expect(renderMarkdown("**Resumen:** usa `git stash -u`.")).toBe(
      "<p><strong>Resumen:</strong> usa <code>git stash -u</code>.</p>",
    );
    // Bold around a code span, which the span must not break up.
    expect(renderMarkdown("Mirá **`session.rs`** primero")).toBe("<p>Mirá <strong><code>session.rs</code></strong> primero</p>");
    expect(renderMarkdown("__fuerte__ y _suave_ y ~~no~~")).toBe("<p><strong>fuerte</strong> y <em>suave</em> y <del>no</del></p>");
  });

  it("leaves alone what only looks like emphasis", () => {
    expect(renderMarkdown("2 * 3 * 4")).toBe("<p>2 * 3 * 4</p>");
    expect(renderMarkdown("the snake_case_name stays")).toBe("<p>the snake_case_name stays</p>");
    // Inside code nothing is emphasis.
    expect(renderMarkdown("`a **b** c`")).toBe("<p><code>a **b** c</code></p>");
  });

  it("reads headings, lists and quotes", () => {
    expect(renderMarkdown("## Pasos\n\n1. uno\n2. **dos**\n\n- a\n- b\n  sigue")).toBe(
      "<h4>Pasos</h4><ol><li>uno</li><li><strong>dos</strong></li></ol><ul><li>a</li><li>b sigue</li></ul>",
    );
    expect(renderMarkdown("> dicho\n> así\n\ny después")).toBe("<blockquote>dicho<br>así</blockquote><p>y después</p>");
    expect(renderMarkdown("antes\n\n---\n\ndespués")).toBe("<p>antes</p><hr><p>después</p>");
  });

  it("keeps a paragraph's own line breaks", () => {
    expect(renderMarkdown("una\ndos\n\ntres")).toBe("<p>una<br>dos</p><p>tres</p>");
  });

  it("shows fenced code as written, even cut short", () => {
    expect(renderMarkdown("Así:\n```ts\nconst a = **1**;\n  if (a < 2) {}\n```\nlisto")).toBe(
      "<p>Así:</p><pre><code>const a = **1**;\n  if (a &lt; 2) {}</code></pre><p>listo</p>",
    );
    expect(renderMarkdown("```\nsin cerrar")).toBe("<pre><code>sin cerrar</code></pre>");
  });

  it("never lets through HTML of the answer's own", () => {
    expect(renderMarkdown('<img src=x onerror="alert(1)"> **ok**')).toBe(
      "<p>&lt;img src=x onerror=&quot;alert(1)&quot;&gt; <strong>ok</strong></p>",
    );
    expect(renderMarkdown("[docs](https://example.com/a?b=1)")).toBe(
      '<p><span class="link" title="https://example.com/a?b=1">docs</span></p>',
    );
    expect(renderMarkdown('[x](javascript:alert("1"))')).not.toContain("<a");
  });
});
