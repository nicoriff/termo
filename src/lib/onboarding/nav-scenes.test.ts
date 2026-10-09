import { describe, expect, it } from "vitest";
import { NAV_SCENES, SCENE_LENGTH, STILL_FRAME, sceneFrame } from "./nav-scenes";

describe("the setup's navigation miniatures", () => {
  it("has a still frame inside every scene", () => {
    for (const scene of NAV_SCENES) expect(STILL_FRAME[scene]).toBeLessThan(SCENE_LENGTH[scene]);
  });

  it("walks the tile picker to another tile and focuses it on Enter", () => {
    expect(sceneFrame("picker", 0).picker).toBeNull();
    expect(sceneFrame("picker", 1)).toMatchObject({ picker: "a", key: "zoomDown", focused: "a" });
    expect(sceneFrame("picker", 3)).toMatchObject({ picker: "c", key: "down" });
    expect(sceneFrame("picker", 4)).toMatchObject({ picker: null, key: "enter", focused: "c" });
  });

  it("zooms the focused tile and puts it back", () => {
    expect(sceneFrame("zoom", 1)).toMatchObject({ zoomed: "b", focused: "b", key: "zoomUp" });
    expect(sceneFrame("zoom", 3)).toMatchObject({ zoomed: null, key: "zoomDown" });
  });

  it("cycles the tabs back to the first", () => {
    expect([0, 1, 2, 3].map((frame) => sceneFrame("tabs", frame).activeTab)).toEqual([0, 1, 2, 0]);
  });

  it("splits one tile into three, one split at a time", () => {
    const shown = (frame: number) =>
      Object.values(sceneFrame("split", frame).tiles).filter((tile) => tile.shown).length;
    expect([0, 1, 3].map(shown)).toEqual([1, 2, 3]);
  });
});
