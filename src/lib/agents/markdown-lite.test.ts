import { describe, expect, it } from "vitest";
import { renderMarkdownLite } from "./markdown-lite";

describe("renderMarkdownLite", () => {
  it("renders emphasis and code", () => {
    expect(renderMarkdownLite("Hay **30** archivos en `src`, *todos* PNG")).toBe(
      "Hay <strong>30</strong> archivos en <code>src</code>, <em>todos</em> PNG",
    );
  });

  it("leaves emphasis inside code alone", () => {
    expect(renderMarkdownLite("`a **b** c`")).toBe("<code>a **b** c</code>");
  });

  it("turns headings and bullets into readable lines", () => {
    expect(renderMarkdownLite("## Resumen\n- uno\n  * dos")).toBe("<strong>Resumen</strong>\n• uno\n  • dos");
  });

  it("escapes any HTML the agent wrote", () => {
    expect(renderMarkdownLite('<img src=x onerror="alert(1)">')).toBe("&lt;img src=x onerror=&quot;alert(1)&quot;&gt;");
  });
});
