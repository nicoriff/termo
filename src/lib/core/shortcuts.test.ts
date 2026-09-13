import { describe, expect, it } from "vitest";
import {
  eventMatchesKeybinding,
  findConflict,
  formatKeybindingLabel,
  getDefaultKeybinding,
  normalizeEventToBinding,
  normalizeEventToZoomBinding,
  parseKeybinding,
  resolveKeybinding,
  serializeKeybinding,
  type KeyLike,
} from "./shortcuts";

function key(overrides: Partial<KeyLike> & { key: string }): KeyLike {
  return { altKey: false, ctrlKey: false, metaKey: false, shiftKey: false, ...overrides };
}

describe("parseKeybinding / serializeKeybinding", () => {
  it("round-trips a single-modifier binding", () => {
    expect(parseKeybinding("meta+t")).toEqual({ modifiers: new Set(["meta"]), key: "t" });
    expect(serializeKeybinding(["meta"], "t")).toBe("meta+t");
  });

  it("canonicalizes modifier order regardless of input order", () => {
    expect(serializeKeybinding(["shift", "ctrl"], "tab")).toBe("ctrl+shift+tab");
  });

  it("parses a binding with no modifiers", () => {
    expect(parseKeybinding("tab")).toEqual({ modifiers: new Set(), key: "tab" });
  });
});

describe("eventMatchesKeybinding", () => {
  it("matches an exact modifier + key combo", () => {
    expect(eventMatchesKeybinding(key({ key: "t", metaKey: true }), "meta+t")).toBe(true);
  });

  it("rejects when the event has an extra modifier not in the binding", () => {
    expect(eventMatchesKeybinding(key({ key: "t", metaKey: true, altKey: true }), "meta+t")).toBe(false);
  });

  it("rejects when the event is missing a modifier the binding requires", () => {
    expect(eventMatchesKeybinding(key({ key: "tab", ctrlKey: true }), "ctrl+shift+tab")).toBe(false);
  });

  it("is case-insensitive on the key", () => {
    expect(eventMatchesKeybinding(key({ key: "T", metaKey: true }), "meta+t")).toBe(true);
  });

  it("honors the exclude option even if the event would otherwise match a looser binding", () => {
    // Ctrl+Alt+T must never resolve to "new tab", no matter what newTab is
    // bound to — this is the generic replacement for the old !altKey guard.
    const ctrlAltT = key({ key: "t", ctrlKey: true, altKey: true });
    expect(eventMatchesKeybinding(ctrlAltT, "ctrl+t", { exclude: ["alt"] })).toBe(false);
    // Sanity check: without exclude, and without alt in the binding, the
    // exact modifier-set match already rejects it too.
    expect(eventMatchesKeybinding(ctrlAltT, "ctrl+t")).toBe(false);
  });

  it("ignores a modifier on both sides when told to, e.g. Shift flipping cycle-tab direction", () => {
    expect(eventMatchesKeybinding(key({ key: "Tab", ctrlKey: true }), "ctrl+tab", { ignore: ["shift"] })).toBe(true);
    expect(
      eventMatchesKeybinding(key({ key: "Tab", ctrlKey: true, shiftKey: true }), "ctrl+tab", { ignore: ["shift"] }),
    ).toBe(true);
    // Still rejects an unrelated extra modifier.
    expect(
      eventMatchesKeybinding(key({ key: "Tab", ctrlKey: true, altKey: true }), "ctrl+tab", { ignore: ["shift"] }),
    ).toBe(false);
  });

  it("matches the zoom placeholder key against either arrow", () => {
    expect(eventMatchesKeybinding(key({ key: "ArrowUp", metaKey: true }), "meta+arrow")).toBe(true);
    expect(eventMatchesKeybinding(key({ key: "ArrowDown", metaKey: true }), "meta+arrow")).toBe(true);
    expect(eventMatchesKeybinding(key({ key: "ArrowLeft", metaKey: true }), "meta+arrow")).toBe(false);
  });
});

describe("normalizeEventToBinding", () => {
  it("rejects a bare modifier press", () => {
    expect(normalizeEventToBinding(key({ key: "Control", ctrlKey: true }))).toBeNull();
  });

  it("rejects an unmodified printable key", () => {
    expect(normalizeEventToBinding(key({ key: "t" }))).toBeNull();
  });

  it("accepts an unmodified non-printable key", () => {
    expect(normalizeEventToBinding(key({ key: "Tab" }))).toBe("tab");
  });

  it("accepts a modified printable key, canonicalized", () => {
    expect(normalizeEventToBinding(key({ key: "T", ctrlKey: true, shiftKey: true }))).toBe("ctrl+shift+t");
  });
});

describe("normalizeEventToZoomBinding", () => {
  it("rejects a non-arrow key", () => {
    expect(normalizeEventToZoomBinding(key({ key: "t", metaKey: true }))).toEqual({
      error: "Grabá con una flecha ↑ o ↓",
    });
  });

  it("rejects an arrow with no modifier", () => {
    expect(normalizeEventToZoomBinding(key({ key: "ArrowUp" }))).toEqual({
      error: "Grabá la combinación con al menos un modificador",
    });
  });

  it("accepts an arrow with a modifier, regardless of direction", () => {
    expect(normalizeEventToZoomBinding(key({ key: "ArrowUp", metaKey: true }))).toEqual({ binding: "meta+arrow" });
    expect(normalizeEventToZoomBinding(key({ key: "ArrowDown", metaKey: true }))).toEqual({ binding: "meta+arrow" });
  });
});

describe("getDefaultKeybinding / resolveKeybinding", () => {
  it("differs by platform", () => {
    expect(getDefaultKeybinding("newTab", true)).toBe("meta+t");
    expect(getDefaultKeybinding("newTab", false)).toBe("ctrl+t");
  });

  it("falls back to the default when there's no override", () => {
    expect(resolveKeybinding("cycleTab", {}, true)).toBe("ctrl+tab");
  });

  it("prefers an override when present", () => {
    expect(resolveKeybinding("newTab", { newTab: "ctrl+shift+n" }, false)).toBe("ctrl+shift+n");
  });

  it("treats an explicit null override as 'use the default'", () => {
    expect(resolveKeybinding("newTab", { newTab: null }, false)).toBe("ctrl+t");
  });
});

describe("findConflict", () => {
  it("finds another action already bound to the same combo", () => {
    const overrides = { cycleTab: "meta+t" };
    expect(findConflict("meta+t", "newTab", overrides, true)).toBe("cycleTab");
  });

  it("returns null when there's no conflict", () => {
    expect(findConflict("ctrl+shift+n", "newTab", {}, false)).toBeNull();
  });

  it("never reports a conflict against the action's own current binding", () => {
    expect(findConflict("meta+t", "newTab", {}, true)).toBeNull();
  });
});

describe("formatKeybindingLabel", () => {
  it("uses symbols on mac", () => {
    expect(formatKeybindingLabel("meta+t", true)).toBe("⌘T");
    expect(formatKeybindingLabel("ctrl+shift+tab", true)).toBe("⌃⇧Tab");
  });

  it("uses words elsewhere", () => {
    expect(formatKeybindingLabel("ctrl+t", false)).toBe("Ctrl+T");
    expect(formatKeybindingLabel("ctrl+shift+tab", false)).toBe("Ctrl+Shift+Tab");
  });

  it("formats the zoom placeholder as an up/down pair", () => {
    expect(formatKeybindingLabel("meta+arrow", true)).toBe("⌘↑ / ⌘↓");
    expect(formatKeybindingLabel("alt+arrow", false)).toBe("Alt+↑ / Alt+↓");
  });
});
