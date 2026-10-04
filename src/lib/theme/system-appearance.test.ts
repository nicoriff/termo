import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { observeSystemAppearance } from "./system-appearance";
import type { Appearance } from "./theme-types";

vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: vi.fn() }));

describe("system appearance observer", () => {
  let media: { matches: boolean; addEventListener: ReturnType<typeof vi.fn>; removeEventListener: ReturnType<typeof vi.fn> };
  let mediaChange: () => void;
  let nativeChange: (event: { payload: Appearance }) => void;
  let removeNative: () => void;
  let theme: ReturnType<typeof vi.fn>;
  let onThemeChanged: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    removeNative = vi.fn(() => {});
    media = {
      matches: false,
      addEventListener: vi.fn((_, callback) => { mediaChange = callback; }),
      removeEventListener: vi.fn(),
    };
    theme = vi.fn().mockResolvedValue("dark");
    onThemeChanged = vi.fn(async (callback) => { nativeChange = callback; return removeNative; });
    vi.stubGlobal("window", { matchMedia: vi.fn(() => media) });
    vi.mocked(getCurrentWindow).mockReturnValue({ theme, onThemeChanged } as unknown as ReturnType<typeof getCurrentWindow>);
  });

  afterEach(() => { vi.unstubAllGlobals(); });

  it("uses the native initial appearance and live changes, keeping media as a fallback", async () => {
    const changed = vi.fn();
    const observer = observeSystemAppearance(changed);
    await observer.ready;
    expect(changed).toHaveBeenLastCalledWith("dark");
    mediaChange();
    expect(changed).toHaveBeenLastCalledWith("dark");
    nativeChange({ payload: "light" });
    expect(changed).toHaveBeenLastCalledWith("light");
    observer.stop();
    expect(removeNative).toHaveBeenCalledOnce();
    expect(media.removeEventListener).toHaveBeenCalledWith("change", mediaChange);
    nativeChange({ payload: "dark" });
    expect(changed).toHaveBeenLastCalledWith("light");
  });

  it("tracks media changes when running without Tauri", async () => {
    vi.mocked(getCurrentWindow).mockImplementation(() => { throw new Error("browser preview"); });
    const changed = vi.fn();
    const observer = observeSystemAppearance(changed);
    await observer.ready;
    expect(changed).toHaveBeenLastCalledWith("light");
    media.matches = true;
    mediaChange();
    expect(changed).toHaveBeenLastCalledWith("dark");
    observer.stop();
    media.matches = false;
    mediaChange();
    expect(changed).toHaveBeenLastCalledWith("dark");
  });

  it("retains the fallback if native event registration fails", async () => {
    onThemeChanged.mockRejectedValue(new Error("unavailable"));
    const changed = vi.fn();
    const observer = observeSystemAppearance(changed);
    await observer.ready;
    expect(changed).toHaveBeenLastCalledWith("dark");
    mediaChange();
    expect(changed).toHaveBeenLastCalledWith("light");
    observer.stop();
  });

  it("retains the fallback if the native initial appearance is unknown", async () => {
    theme.mockResolvedValue(null);
    const changed = vi.fn();
    const observer = observeSystemAppearance(changed);
    await observer.ready;
    media.matches = true;
    mediaChange();
    expect(changed).toHaveBeenLastCalledWith("dark");
    observer.stop();
  });

  it("does not overwrite a newer event with an older initial snapshot", async () => {
    theme.mockImplementation(async () => { nativeChange({ payload: "light" }); return "dark"; });
    const changed = vi.fn();
    const observer = observeSystemAppearance(changed);
    await observer.ready;
    expect(changed).toHaveBeenLastCalledWith("light");
    observer.stop();
  });

  it("removes a native listener that finishes registering after disposal", async () => {
    let finish!: (remove: () => void) => void;
    onThemeChanged.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const changed = vi.fn();
    const observer = observeSystemAppearance(changed);
    observer.stop();
    finish(removeNative);
    await observer.ready;
    expect(removeNative).toHaveBeenCalledOnce();
    expect(theme).not.toHaveBeenCalled();
    expect(changed).toHaveBeenCalledOnce();
  });

  it("supports older webviews with the legacy MediaQueryList API", async () => {
    vi.mocked(getCurrentWindow).mockImplementation(() => { throw new Error("browser preview"); });
    const legacy = { matches: false, addListener: vi.fn(), removeListener: vi.fn() };
    vi.stubGlobal("window", { matchMedia: () => legacy });
    const observer = observeSystemAppearance(vi.fn());
    await observer.ready;
    observer.stop();
    expect(legacy.addListener).toHaveBeenCalledOnce();
    expect(legacy.removeListener).toHaveBeenCalledWith(legacy.addListener.mock.calls[0][0]);
  });
});
