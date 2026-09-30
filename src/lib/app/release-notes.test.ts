import { describe, expect, it } from "vitest";
import changelog from "./changelog.json";
import pkg from "../../../package.json";
import { asList, compareVersions, entriesBetween, notesFor, type ChangelogEntry } from "./release-notes";

const entry = (version: string): ChangelogEntry => ({ version, date: "2026-01-01", es: [`es ${version}`], en: [`en ${version}`] });

describe("compareVersions", () => {
  it("compares each part as a number", () => {
    expect(compareVersions("0.10.0", "0.9.2")).toBeGreaterThan(0);
    expect(compareVersions("1.0.0", "1.0.0")).toBe(0);
    expect(compareVersions("v0.8.1", "0.8.0")).toBeGreaterThan(0);
    expect(compareVersions("0.8", "0.8.0")).toBe(0);
    expect(compareVersions("0.8.0-beta.1", "0.8.0")).toBe(0);
    expect(compareVersions("", "0.1.0")).toBeLessThan(0);
  });
});

describe("entriesBetween", () => {
  const log = [entry("0.8.0"), entry("0.10.0"), entry("0.9.0"), entry("0.7.0")];

  it("takes what came after the old version, up to the new one, newest first", () => {
    expect(entriesBetween(log, "0.7.0", "0.9.0").map((e) => e.version)).toEqual(["0.9.0", "0.8.0"]);
    expect(entriesBetween(log, "0.8.0", "0.10.0").map((e) => e.version)).toEqual(["0.10.0", "0.9.0"]);
  });

  it("is empty when nothing moved", () => {
    expect(entriesBetween(log, "0.10.0", "0.10.0")).toEqual([]);
  });
});

describe("notesFor", () => {
  const body = "<!-- lang:es -->\n- Nuevo\n\n<!-- lang:en -->\n- New\n";

  it("picks the section in the current language", () => {
    expect(notesFor(body, "es")).toBe("- Nuevo");
    expect(notesFor(body, "en")).toBe("- New");
  });

  it("falls back to the other language, then to the whole body", () => {
    expect(notesFor("<!-- lang:es -->\n- Solo español", "en")).toBe("- Solo español");
    expect(notesFor("Written by hand", "es")).toBe("Written by hand");
    expect(notesFor(undefined, "es")).toBe("");
  });

  it("reads back what the release script writes", () => {
    const written = `<!-- lang:es -->\n${asList(["a", "b"])}\n\n<!-- lang:en -->\n${asList(["c"])}\n`;
    expect(notesFor(written, "es")).toBe("- a\n- b");
    expect(notesFor(written, "en")).toBe("- c");
  });
});

/** changelog.json is what the app shows after an update and what the
 * release's notes are made from: every entry in both languages. */
describe("changelog.json", () => {
  const entries = changelog as ChangelogEntry[];

  it("has every entry in Spanish and English", () => {
    for (const e of entries) {
      expect(e.es.length, e.version).toBeGreaterThan(0);
      expect(e.en.length, e.version).toBeGreaterThan(0);
      expect([...e.es, ...e.en].every((item) => item.trim().length > 0), e.version).toBe(true);
      expect(e.date, e.version).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    }
  });

  // Between releases the first entry is the one being written, ahead of
  // package.json; the release script insists on an exact match for the tag.
  it("never falls behind the app's version and never repeats one", () => {
    expect(compareVersions(entries[0].version, pkg.version)).toBeGreaterThanOrEqual(0);
    const versions = entries.map((e) => e.version);
    expect(new Set(versions).size).toBe(versions.length);
    const sorted = [...versions].sort((a, b) => compareVersions(b, a));
    expect(versions).toEqual(sorted);
  });
});
