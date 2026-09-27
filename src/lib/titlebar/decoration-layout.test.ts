import { describe, expect, it } from "vitest";
import { DEFAULT_DECORATION_LAYOUT, parseDecorationLayout } from "./decoration-layout";

describe("parseDecorationLayout", () => {
  it("falls back to the default without a value", () => {
    expect(parseDecorationLayout(null)).toEqual(DEFAULT_DECORATION_LAYOUT);
    expect(parseDecorationLayout("")).toEqual(DEFAULT_DECORATION_LAYOUT);
  });

  it("reads Ubuntu's layout, ignoring the appmenu", () => {
    expect(parseDecorationLayout("appmenu:minimize,maximize,close")).toEqual({
      start: [],
      end: ["minimize", "maximize", "close"],
      menuSide: "start",
    });
  });

  it("keeps GNOME's close-only layout", () => {
    expect(parseDecorationLayout("appmenu:close")).toEqual({ start: [], end: ["close"], menuSide: "start" });
  });

  it("puts the menu on the right when the buttons are on the left", () => {
    expect(parseDecorationLayout("close,minimize,maximize:")).toEqual({
      start: ["close", "minimize", "maximize"],
      end: [],
      menuSide: "end",
    });
  });

  it("treats a value without a colon as all on the left", () => {
    expect(parseDecorationLayout("close")).toEqual({ start: ["close"], end: [], menuSide: "end" });
  });

  it("drops unknown names, spaces and repeats", () => {
    expect(parseDecorationLayout("icon, close : spacer,close,maximize")).toEqual({
      start: ["close"],
      end: ["maximize"],
      menuSide: "end",
    });
  });
});
