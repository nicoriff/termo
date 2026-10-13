/**
 * The notification sounds Modo IA can play when a console needs the user
 * (awaiting) or is done (finished). Synthesized with the Web Audio API
 * rather than shipped as audio files: nothing to bundle or license, and each
 * one is a few lines of notes.
 *
 * The AudioContext is created on first use, never at import, so this module
 * loads fine in Node for tests.
 */

import type { ToolId } from "./pattern-table";

export const SOUND_IDS = [
  "chime",
  "alert",
  "pop",
  "marimba",
  "soft",
  "bell",
  "doorbell",
  "blip",
  "drop",
  "success",
  "knock",
  "harp",
  "sonar",
  "ping",
  "glass",
  "kalimba",
  "cuckoo",
  "whistle",
  "fanfare",
  "bubbles",
  "gong",
  "coin",
  "cricket",
  "musicbox",
  "woodblock",
] as const;
export type SoundId = (typeof SOUND_IDS)[number];
/** What a setting holds: a sound, or none at all. */
export type SoundChoice = SoundId | "none";

/** The two moments a sound can mark. */
export type SoundEvent = "awaiting" | "finished";

/** Different per agent out of the box, so they're told apart by ear before
 * anyone opens Preferences. Mirrors the defaults in config/schema.rs. */
export const DEFAULT_SOUNDS: Record<ToolId, Record<SoundEvent, SoundChoice>> = {
  claude: { awaiting: "alert", finished: "chime" },
  codex: { awaiting: "pop", finished: "marimba" },
  gemini: { awaiting: "doorbell", finished: "harp" },
  copilot: { awaiting: "knock", finished: "success" },
  grok: { awaiting: "blip", finished: "bell" },
};

export function isSoundChoice(value: string): value is SoundChoice {
  return value === "none" || (SOUND_IDS as readonly string[]).includes(value);
}

interface Note {
  /** Hz; `to` glides the pitch there over the note. */
  freq: number;
  to?: number;
  /** Seconds from the sound's start. */
  at: number;
  length: number;
  wave?: OscillatorType;
  /** Peak level before the master volume, 0–1. */
  level?: number;
  /** Seconds to reach the peak; the rest of the note decays. */
  attack?: number;
}

export const SOUNDS: Record<SoundId, Note[]> = {
  // Two bell partials struck together, ringing out.
  chime: [
    { freq: 1318.5, at: 0, length: 1.1, level: 0.5 },
    { freq: 1975.5, at: 0, length: 0.7, level: 0.22 },
  ],
  // A rising two-note call: something's waiting on you.
  alert: [
    { freq: 659.3, at: 0, length: 0.22, wave: "triangle", level: 0.55 },
    { freq: 987.8, at: 0.14, length: 0.35, wave: "triangle", level: 0.55 },
  ],
  // A quick drop in pitch, like a bubble.
  pop: [{ freq: 880, to: 220, at: 0, length: 0.14, level: 0.7, attack: 0.002 }],
  // A bright three-note arpeggio.
  marimba: [
    { freq: 523.3, at: 0, length: 0.3, level: 0.5 },
    { freq: 659.3, at: 0.09, length: 0.3, level: 0.5 },
    { freq: 784, at: 0.18, length: 0.45, level: 0.5 },
  ],
  // A low, slow swell for when anything sharper would be too much.
  soft: [{ freq: 440, at: 0, length: 0.9, level: 0.45, attack: 0.18 }],
  // A church-bell strike: a low fundamental with the inharmonic partials
  // (×2.76, ×5.4) that make metal sound like metal, the high ones dying first.
  bell: [
    { freq: 392, at: 0, length: 1.8, level: 0.45 },
    { freq: 1082, at: 0, length: 1.0, level: 0.2 },
    { freq: 2117, at: 0, length: 0.5, level: 0.1 },
  ],
  // Ding-dong: a falling third, like a doorbell.
  doorbell: [
    { freq: 659.3, at: 0, length: 0.55, wave: "triangle", level: 0.55 },
    { freq: 523.3, at: 0.32, length: 0.9, wave: "triangle", level: 0.55 },
  ],
  // Two short square-wave beeps, old-computer style. Square waves carry more
  // energy than sines, hence the lower level.
  blip: [
    { freq: 1318.5, at: 0, length: 0.07, wave: "square", level: 0.18, attack: 0.002 },
    { freq: 1760, at: 0.09, length: 0.09, wave: "square", level: 0.18, attack: 0.002 },
  ],
  // A water drop: a fast upward glide, and a faint second one after it.
  drop: [
    { freq: 520, to: 1500, at: 0, length: 0.12, level: 0.6, attack: 0.003 },
    { freq: 700, to: 1800, at: 0.16, length: 0.1, level: 0.2, attack: 0.003 },
  ],
  // A quick major arpeggio up to the octave: done, and done well.
  success: [
    { freq: 523.3, at: 0, length: 0.18, wave: "triangle", level: 0.45 },
    { freq: 659.3, at: 0.07, length: 0.18, wave: "triangle", level: 0.45 },
    { freq: 784, at: 0.14, length: 0.18, wave: "triangle", level: 0.45 },
    { freq: 1046.5, at: 0.21, length: 0.5, wave: "triangle", level: 0.45 },
  ],
  // Two dull knocks on a door: low thumps that drop in pitch.
  knock: [
    { freq: 190, to: 110, at: 0, length: 0.09, level: 0.85, attack: 0.002 },
    { freq: 190, to: 110, at: 0.17, length: 0.09, level: 0.85, attack: 0.002 },
  ],
  // A pentatonic run up the strings, each note left to ring.
  harp: [
    { freq: 523.3, at: 0, length: 0.9, level: 0.32 },
    { freq: 587.3, at: 0.06, length: 0.9, level: 0.32 },
    { freq: 659.3, at: 0.12, length: 0.9, level: 0.32 },
    { freq: 784, at: 0.18, length: 0.9, level: 0.32 },
    { freq: 880, at: 0.24, length: 1.1, level: 0.32 },
  ],
  // A sonar ping and its quieter echo.
  sonar: [
    { freq: 1175, at: 0, length: 1.0, level: 0.45, attack: 0.004 },
    { freq: 1175, at: 0.5, length: 0.8, level: 0.14, attack: 0.004 },
  ],
  // One short, clean high note with a trace of its octave.
  ping: [
    { freq: 1568, at: 0, length: 0.5, level: 0.5, attack: 0.003 },
    { freq: 3136, at: 0, length: 0.2, level: 0.12, attack: 0.003 },
  ],
  // A tap on a wine glass: high inharmonic partials, gone quickly.
  glass: [
    { freq: 2093, at: 0, length: 0.6, level: 0.35, attack: 0.002 },
    { freq: 3520, at: 0, length: 0.4, level: 0.18, attack: 0.002 },
    { freq: 5274, at: 0, length: 0.25, level: 0.08, attack: 0.002 },
  ],
  // Two plucked tines a fourth apart, each with the click of its overtone.
  kalimba: [
    { freq: 659.3, at: 0, length: 0.5, level: 0.5, attack: 0.003 },
    { freq: 1977.9, at: 0, length: 0.08, level: 0.15, attack: 0.002 },
    { freq: 880, at: 0.16, length: 0.7, level: 0.5, attack: 0.003 },
    { freq: 2640, at: 0.16, length: 0.08, level: 0.15, attack: 0.002 },
  ],
  // The clock's bird: a falling minor third, breathy rather than struck.
  cuckoo: [
    { freq: 784, at: 0, length: 0.22, level: 0.5, attack: 0.02 },
    { freq: 659.3, at: 0.26, length: 0.4, level: 0.5, attack: 0.02 },
  ],
  // A whistle that slides up and holds the top note.
  whistle: [
    { freq: 1200, to: 2000, at: 0, length: 0.22, level: 0.4, attack: 0.04 },
    { freq: 2000, at: 0.24, length: 0.3, level: 0.4, attack: 0.03 },
  ],
  // A brass call: two short notes and a long one a fourth up. Sawtooth waves
  // are the loudest of all, hence the low level.
  fanfare: [
    { freq: 392, at: 0, length: 0.12, wave: "sawtooth", level: 0.16, attack: 0.01 },
    { freq: 392, at: 0.13, length: 0.12, wave: "sawtooth", level: 0.16, attack: 0.01 },
    { freq: 523.3, at: 0.26, length: 0.5, wave: "sawtooth", level: 0.16, attack: 0.01 },
  ],
  // Three bubbles rising, each a little higher than the last.
  bubbles: [
    { freq: 400, to: 900, at: 0, length: 0.08, level: 0.5, attack: 0.003 },
    { freq: 550, to: 1200, at: 0.1, length: 0.08, level: 0.5, attack: 0.003 },
    { freq: 750, to: 1600, at: 0.2, length: 0.1, level: 0.5, attack: 0.003 },
  ],
  // A gong: low and long, its inharmonic partials fading one after another.
  gong: [
    { freq: 147, at: 0, length: 2.4, level: 0.5, attack: 0.02 },
    { freq: 219, at: 0, length: 1.8, level: 0.25, attack: 0.02 },
    { freq: 351, at: 0, length: 1.2, level: 0.15, attack: 0.02 },
    { freq: 554, at: 0, length: 0.6, level: 0.08, attack: 0.02 },
  ],
  // The arcade coin: a short square note and a held one a fourth above.
  coin: [
    { freq: 987.8, at: 0, length: 0.08, wave: "square", level: 0.16, attack: 0.002 },
    { freq: 1318.5, at: 0.08, length: 0.45, wave: "square", level: 0.16, attack: 0.002 },
  ],
  // A cricket: two bursts of three very high, very short chirps.
  cricket: [
    { freq: 4000, at: 0, length: 0.03, level: 0.3, attack: 0.004 },
    { freq: 4000, at: 0.05, length: 0.03, level: 0.3, attack: 0.004 },
    { freq: 4000, at: 0.1, length: 0.03, level: 0.3, attack: 0.004 },
    { freq: 4000, at: 0.3, length: 0.03, level: 0.3, attack: 0.004 },
    { freq: 4000, at: 0.35, length: 0.03, level: 0.3, attack: 0.004 },
    { freq: 4000, at: 0.4, length: 0.03, level: 0.3, attack: 0.004 },
  ],
  // A music box: a high broken chord that comes down and jumps back up.
  musicbox: [
    { freq: 1568, at: 0, length: 0.5, level: 0.35, attack: 0.002 },
    { freq: 1318.5, at: 0.15, length: 0.5, level: 0.35, attack: 0.002 },
    { freq: 1046.5, at: 0.3, length: 0.5, level: 0.35, attack: 0.002 },
    { freq: 1568, at: 0.45, length: 0.8, level: 0.35, attack: 0.002 },
  ],
  // Tick-tock on a woodblock: two dry taps, the second one lower.
  woodblock: [
    { freq: 800, at: 0, length: 0.05, wave: "triangle", level: 0.6, attack: 0.001 },
    { freq: 600, at: 0.14, length: 0.06, wave: "triangle", level: 0.6, attack: 0.001 },
  ],
};

let context: AudioContext | null = null;

/** Plays `choice` at `volume` (0–100). "none" and a muted volume are silent. */
export function playSound(choice: SoundChoice, volume: number): void {
  if (choice === "none" || volume <= 0) return;
  context ??= new AudioContext();
  // A context made before the page had any user input starts suspended.
  if (context.state === "suspended") void context.resume();

  const master = context.createGain();
  master.gain.value = Math.min(100, volume) / 100;
  master.connect(context.destination);

  const start = context.currentTime + 0.01;
  for (const note of SOUNDS[choice]) {
    const oscillator = context.createOscillator();
    const envelope = context.createGain();
    const begin = start + note.at;
    const end = begin + note.length;
    const attack = note.attack ?? 0.008;

    oscillator.type = note.wave ?? "sine";
    oscillator.frequency.setValueAtTime(note.freq, begin);
    if (note.to) oscillator.frequency.exponentialRampToValueAtTime(note.to, end);

    // Exponential ramps can't touch zero, hence the tiny floor.
    envelope.gain.setValueAtTime(0.0001, begin);
    envelope.gain.exponentialRampToValueAtTime(note.level ?? 0.5, begin + attack);
    envelope.gain.exponentialRampToValueAtTime(0.0001, end);

    oscillator.connect(envelope).connect(master);
    oscillator.start(begin);
    oscillator.stop(end + 0.02);
  }
}
