import { describe, expect, it } from "vitest";
import { en } from "./messages/en";
import { es } from "./messages/es";
import { collectKeys, interpolate, lookup, resolveLocale } from "./translate";

describe("catalogs", () => {
  it("es and en define exactly the same keys", () => {
    expect(collectKeys(en).sort()).toEqual(collectKeys(es).sort());
  });

  it("has no empty translations", () => {
    for (const catalog of [es, en]) {
      for (const key of collectKeys(catalog)) {
        expect(lookup(catalog, key as never).trim(), key).not.toBe("");
      }
    }
  });

  it("uses the same placeholders in both languages", () => {
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const key of collectKeys(es)) {
      expect(placeholders(lookup(en, key as never)), key).toEqual(placeholders(lookup(es, key as never)));
    }
  });
});

describe("resolveLocale", () => {
  it("returns an explicit choice as-is", () => {
    expect(resolveLocale("en", "es-AR")).toBe("en");
    expect(resolveLocale("es", "en-US")).toBe("es");
  });

  it("follows the system language for 'system'", () => {
    expect(resolveLocale("system", "es-AR")).toBe("es");
    expect(resolveLocale("system", "ES")).toBe("es");
    expect(resolveLocale("system", "en-US")).toBe("en");
    expect(resolveLocale("system", "pt-BR")).toBe("en");
    expect(resolveLocale("system", undefined)).toBe("en");
  });
});

describe("lookup / interpolate", () => {
  it("resolves a dotted key", () => {
    expect(lookup(es, "preferences.title")).toBe("Preferencias");
    expect(lookup(en, "preferences.title")).toBe("Preferences");
  });

  it("fills placeholders and leaves unknown ones untouched", () => {
    expect(interpolate("Memoria: {value}", { value: "4 MB" })).toBe("Memoria: 4 MB");
    expect(interpolate("{a} y {b}", { a: 1 })).toBe("1 y {b}");
  });
});
