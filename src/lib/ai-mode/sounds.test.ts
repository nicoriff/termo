import { describe, expect, it } from "vitest";
import { SOUND_IDS, isSoundChoice, playSound } from "./sounds";

describe("isSoundChoice", () => {
  it("accepts every sound in the bank and none", () => {
    for (const id of SOUND_IDS) expect(isSoundChoice(id)).toBe(true);
    expect(isSoundChoice("none")).toBe(true);
  });

  it("rejects names that aren't in the bank", () => {
    expect(isSoundChoice("")).toBe(false);
    expect(isSoundChoice("Chime")).toBe(false);
    expect(isSoundChoice("siren")).toBe(false);
  });
});

describe("playSound", () => {
  // No AudioContext in Node: these must return before ever reaching for one.
  it("stays silent for none and for a muted volume", () => {
    expect(() => playSound("none", 80)).not.toThrow();
    expect(() => playSound("chime", 0)).not.toThrow();
  });
});
