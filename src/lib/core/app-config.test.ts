import { beforeEach, describe, expect, it, vi } from "vitest";
import { appConfigStore } from "./app-config.svelte";
import { getConfig, saveConfig } from "./config-client";
import { themeStore } from "../theme/theme-store.svelte";
import { observeSystemAppearance } from "../theme/system-appearance";
import { listen } from "@tauri-apps/api/event";
import type { AppConfig } from "./types";
import type { Appearance } from "../theme/theme-types";

vi.mock("./config-client", () => ({ getConfig: vi.fn(), saveConfig: vi.fn().mockResolvedValue(undefined) }));
vi.mock("./window-client", () => ({ getWindowSize: vi.fn(), setWindowSize: vi.fn() }));
vi.mock("./platform", () => ({ isMac: true }));
vi.mock("../i18n/i18n.svelte", () => ({ i18n: { setPreference: vi.fn() } }));
vi.mock("../theme/theme-loader", () => ({ applyThemeToDocument: vi.fn() }));
vi.mock("../theme/system-appearance", () => ({ observeSystemAppearance: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => ({ label: "main" }) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

// Only fields applyFields reads as structured data are necessary here. The
// schema's defaults and JSON serialization are exercised by the Rust tests.
const config = {
  theme: "Nord", sync_with_os: true, light_theme: "Solarized Light", dark_theme: "Dracula",
  language: "en", window: { width: 1000, height: 640 }, shortcuts: {},
  font_size_override: null, font_family_override: null, installed_font_path: null,
  background_opacity: 100, output_highlighting: true,
} as AppConfig;

describe("appearance config persistence and window sync", () => {
  let systemChanged: (appearance: Appearance) => void;

  beforeEach(async () => {
    vi.clearAllMocks();
    vi.mocked(getConfig).mockResolvedValue(config);
    vi.mocked(observeSystemAppearance).mockImplementation((callback) => {
      systemChanged = callback;
      callback("light");
      return { ready: Promise.resolve(), stop: vi.fn() };
    });
    await appConfigStore.init({ restoreWindowSize: false });
  });

  it("restores the effective theme at startup without saving a new config", () => {
    expect(themeStore.current.name).toBe("Solarized Light");
    expect(saveConfig).not.toHaveBeenCalled();
  });

  it("OS changes never save, and later unrelated saves retain the manual theme", async () => {
    systemChanged("dark");
    expect(themeStore.current.name).toBe("Dracula");
    expect(saveConfig).not.toHaveBeenCalled();
    await appConfigStore.persist();
    expect(saveConfig).toHaveBeenLastCalledWith(expect.objectContaining({
      theme: "Nord", sync_with_os: true, light_theme: "Solarized Light", dark_theme: "Dracula",
    }));
  });

  it("applies other windows' choices without an echo save and ignores its own broadcasts", () => {
    const receive = vi.mocked(listen).mock.calls[0][1];
    receive({ payload: { source: "preferences", config: { ...config, light_theme: "Light" } } } as Parameters<typeof receive>[0]);
    expect(themeStore.current.name).toBe("Light");
    expect(saveConfig).not.toHaveBeenCalled();
    receive({ payload: { source: "main", config } } as Parameters<typeof receive>[0]);
    expect(themeStore.current.name).toBe("Light");
  });

  it("persists switching off sync and manual choices made while sync is enabled", async () => {
    themeStore.setSyncWithOs(false);
    await appConfigStore.persist();
    expect(themeStore.current.name).toBe("Nord");
    expect(saveConfig).toHaveBeenLastCalledWith(expect.objectContaining({ theme: "Nord", sync_with_os: false }));
    themeStore.setSyncWithOs(true);
    themeStore.setTheme("Tokyo Night");
    await appConfigStore.persist();
    expect(saveConfig).toHaveBeenLastCalledWith(expect.objectContaining({
      theme: "Tokyo Night", sync_with_os: false, dark_theme: "Tokyo Night",
    }));
  });
});
