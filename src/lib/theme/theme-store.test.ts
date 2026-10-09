import { beforeEach, describe, expect, it, vi } from "vitest";
import { ThemeStore } from "./theme-store.svelte";

vi.mock("./theme-loader", () => ({ applyThemeToDocument: vi.fn() }));

describe("theme preferences", () => {
  let store: ThemeStore;
  beforeEach(() => { store = new ThemeStore(); });

  it.each(["Dracula", "Solarized Light"])("migrates the old %s selection into its category", (name) => {
    store.applyPreferences({ manualTheme: name });
    expect(store.current.name).toBe(name);
    expect(store.syncWithOs).toBe(false);
    expect(store.lightTheme).toBe(name === "Solarized Light" ? name : "Light");
    expect(store.darkTheme).toBe(name === "Dracula" ? name : "Dark");
  });

  it("uses category defaults for unknown or incorrectly categorized choices", () => {
    store.applyPreferences({ manualTheme: "deleted", syncWithOs: true, lightTheme: "Dracula", darkTheme: "missing" });
    expect(store.manualTheme).toBe("Dark");
    expect(store.lightTheme).toBe("Light");
    expect(store.darkTheme).toBe("Dark");
    store.setSystemAppearance("light");
    expect(store.current.name).toBe("Light");
  });

  it("follows the OS without modifying any saved choices", () => {
    store.applyPreferences({ manualTheme: "Nord", syncWithOs: true, lightTheme: "Solarized Light", darkTheme: "Dracula" });
    store.setSystemAppearance("light");
    expect(store.current.name).toBe("Solarized Light");
    store.setSystemAppearance("dark");
    expect(store.current.name).toBe("Dracula");
    expect(store.manualTheme).toBe("Nord");
    expect(store.lightTheme).toBe("Solarized Light");
    expect(store.darkTheme).toBe("Dracula");
    expect(store.syncWithOs).toBe(true);
  });

  it("ignores OS changes in manual mode and restores the manual choice when sync stops", () => {
    store.selectTheme("Nord");
    store.setSystemAppearance("light");
    expect(store.current.name).toBe("Nord");
    store.setSyncWithOs(true);
    expect(store.current.name).toBe("Light");
    store.setSyncWithOs(false);
    expect(store.current.name).toBe("Nord");
  });

  it("saves the inactive category without applying it until the OS changes", () => {
    store.setSystemAppearance("dark");
    store.setSyncWithOs(true);
    store.setAppearanceTheme("light", "Solarized Light");
    expect(store.current.name).toBe("Dark");
    store.setSystemAppearance("light");
    expect(store.current.name).toBe("Solarized Light");
    store.setAppearanceTheme("light", "Light");
    expect(store.current.name).toBe("Light");
  });

  it("manual commands disable sync and remember the chosen category", () => {
    store.setSyncWithOs(true);
    store.selectTheme("Solarized Light");
    expect(store.syncWithOs).toBe(false);
    expect(store.manualTheme).toBe("Solarized Light");
    expect(store.lightTheme).toBe("Solarized Light");
    store.setSystemAppearance("dark");
    expect(store.current.name).toBe("Solarized Light");
  });

  it("rejects unknown themes and choices in the wrong category", () => {
    store.setSyncWithOs(true);
    store.selectTheme("missing");
    store.setAppearanceTheme("light", "Dracula");
    expect(store.syncWithOs).toBe(true);
    expect(store.lightTheme).toBe("Light");
    expect(store.current.name).toBe("Dark");
  });

  it("applies another window's preferences using this window's current OS appearance", () => {
    store.setSystemAppearance("light");
    store.applyPreferences({ manualTheme: "Nord", syncWithOs: true, lightTheme: "Solarized Light", darkTheme: "Dracula" });
    expect(store.current.name).toBe("Solarized Light");
    store.applyPreferences({ manualTheme: "Nord", syncWithOs: false, lightTheme: "Light", darkTheme: "Dracula" });
    expect(store.current.name).toBe("Nord");
  });
});
