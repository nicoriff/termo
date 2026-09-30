import { describe, expect, it } from "vitest";
import { es } from "$lib/i18n/messages/es";
import { en } from "$lib/i18n/messages/en";
import { SOUNDS, SOUND_IDS, isSoundChoice, playSound } from "./sounds";

describe("the sound bank", () => {
  it("has notes Web Audio can play for every sound", () => {
    for (const id of SOUND_IDS) {
      expect(SOUNDS[id].length, id).toBeGreaterThan(0);
      for (const note of SOUNDS[id]) {
        expect(note.freq, id).toBeGreaterThan(0);
        // Exponential ramps can't reach zero.
        if (note.to !== undefined) expect(note.to, id).toBeGreaterThan(0);
        expect(note.length, id).toBeGreaterThan(0);
        expect(note.at, id).toBeGreaterThanOrEqual(0);
        expect(note.level ?? 0.5, id).toBeGreaterThan(0);
        expect(note.level ?? 0.5, id).toBeLessThanOrEqual(1);
        // The swell has to peak before the note ends.
        expect(note.attack ?? 0.008, id).toBeLessThan(note.length);
      }
    }
  });

  it("names every sound in both languages", () => {
    for (const id of SOUND_IDS) {
      expect(es.preferences.aiMode.soundNames[id], id).toBeTruthy();
      expect(en.preferences.aiMode.soundNames[id], id).toBeTruthy();
    }
  });
});

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
