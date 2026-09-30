import { describe, expect, it } from "vitest";
import { placePanel } from "./dropdown-placement";

const trigger = (top: number) => ({ left: 100, top, bottom: top + 28, width: 180 });

describe("placePanel", () => {
  it("opens downwards when the panel fits below", () => {
    const placement = placePanel(trigger(100), 800, 240);
    expect(placement.top).toBe(132);
    expect(placement.bottom).toBeUndefined();
    expect(placement.maxHeight).toBe(240);
    expect(placement).toMatchObject({ left: 100, width: 180 });
  });

  it("opens upwards near the window's bottom, right above the trigger", () => {
    const placement = placePanel(trigger(700), 800, 240);
    expect(placement.top).toBeUndefined();
    // Its bottom edge sits 4px above the trigger's top (800 - 700 + 4).
    expect(placement.bottom).toBe(104);
    expect(placement.maxHeight).toBe(240);
  });

  it("shrinks to the room there is, whichever way it opens", () => {
    // 200px window, trigger in the middle: 72px below, 76px above.
    const placement = placePanel(trigger(88), 200, 240);
    expect(placement.bottom).toBeDefined();
    expect(placement.maxHeight).toBe(76);
  });

  it("stays down when it doesn't fit either way but there's more room below", () => {
    const placement = placePanel(trigger(20), 300, 400);
    expect(placement.top).toBe(52);
    expect(placement.maxHeight).toBe(240);
  });

  it("never gets shorter than a couple of rows", () => {
    expect(placePanel(trigger(10), 60, 240).maxHeight).toBe(64);
  });
});
