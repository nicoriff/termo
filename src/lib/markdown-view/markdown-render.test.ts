import { describe, expect, it } from "vitest";
import { linkTarget, renderMarkdownDocument, slugify } from "./markdown-render";

describe("renderMarkdownDocument", () => {
  it("renders GitHub's flavour", () => {
    const html = renderMarkdownDocument("# Plan\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n- [ ] todo\n\n~~old~~");
    expect(html).toContain('<h1 id="plan">Plan</h1>');
    expect(html).toContain("<table>");
    expect(html).toContain('type="checkbox"');
    expect(html).toContain("<del>old</del>");
  });

  it("shows HTML in the file as text", () => {
    const html = renderMarkdownDocument('<script>alert(1)</script>\n\nhi <img src=x onerror="alert(1)">');
    expect(html).not.toContain("<script");
    expect(html).not.toContain("<img");
    expect(html).toContain("&lt;script&gt;");
  });

  it("gives links no href and refuses local images", () => {
    const html = renderMarkdownDocument('[x](javascript:alert(1)) ![pic](C:/secret.png) ![web](https://e.com/a.png "t")');
    expect(html).not.toMatch(/\shref=/);
    expect(html).toContain('data-href="javascript:alert(1)"');
    expect(html).not.toContain('src="C:/secret.png"');
    expect(html).toContain('<img src="https://e.com/a.png" alt="web" title="t" loading="lazy">');
  });

  it("numbers headings that repeat", () => {
    const html = renderMarkdownDocument("## Step\n\n## Step");
    expect(html).toContain('id="step"');
    expect(html).toContain('id="step-1"');
  });
});

describe("linkTarget", () => {
  it("tells where a link goes", () => {
    expect(linkTarget("#next-steps")).toEqual({ kind: "anchor", id: "next-steps" });
    expect(linkTarget("https://e.com")).toEqual({ kind: "web", url: "https://e.com" });
    expect(linkTarget("docs/other%20file.md#top")).toEqual({ kind: "file", path: "docs/other file.md", anchor: "top" });
    expect(linkTarget("C:\\x\\a.md")).toEqual({ kind: "file", path: "C:\\x\\a.md" });
    expect(linkTarget("javascript:alert(1)")).toEqual({ kind: "none" });
    expect(linkTarget("file:///etc/passwd")).toEqual({ kind: "none" });
  });
});

describe("slugify", () => {
  it("makes GitHub-style ids", () => {
    expect(slugify("What's next? (v2)")).toBe("whats-next-v2");
    expect(slugify("Próximos pasos")).toBe("próximos-pasos");
  });
});
