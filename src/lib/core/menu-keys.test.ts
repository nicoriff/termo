import { describe, expect, it } from "vitest";
import { indexByLetter, stepIndex } from "./menu-keys";

describe("stepIndex", () => {
  it("moves and wraps around", () => {
    expect(stepIndex("ArrowDown", 0, 3)).toBe(1);
    expect(stepIndex("ArrowDown", 2, 3)).toBe(0);
    expect(stepIndex("ArrowUp", 0, 3)).toBe(2);
    expect(stepIndex("Home", 2, 3)).toBe(0);
    expect(stepIndex("End", 0, 3)).toBe(2);
  });

  it("starts at an end when nothing has focus yet", () => {
    expect(stepIndex("ArrowDown", -1, 3)).toBe(0);
    expect(stepIndex("ArrowUp", -1, 3)).toBe(2);
  });

  it("leaves other keys and empty menus alone", () => {
    expect(stepIndex("Enter", 0, 3)).toBeNull();
    expect(stepIndex("ArrowDown", -1, 0)).toBeNull();
  });
});

describe("indexByLetter", () => {
  const labels = ["  New window", "Edit", "Extensions...", "Quit"];

  it("finds the next item starting with the letter, wrapping", () => {
    expect(indexByLetter(labels, 0, "e")).toBe(1);
    expect(indexByLetter(labels, 1, "E")).toBe(2);
    expect(indexByLetter(labels, 2, "e")).toBe(1);
    expect(indexByLetter(labels, -1, "n")).toBe(0);
  });

  it("is -1 when nothing starts with it", () => {
    expect(indexByLetter(labels, 0, "z")).toBe(-1);
  });
});
