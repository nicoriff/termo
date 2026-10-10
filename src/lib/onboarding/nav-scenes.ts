/**
 * The storyboards behind the setup's navigation miniatures: for each scene,
 * what the little window looks like at each beat. Pure, so the component only
 * draws (and so it can be tested in plain Node).
 */

export type NavScene = "picker" | "zoom" | "tabs" | "split" | "palette";
export type TileId = "a" | "b" | "c";

/** The key (or command) a beat shows as pressed. */
export type SceneKey =
  | "zoomUp"
  | "zoomDown"
  | "right"
  | "down"
  | "enter"
  | "cycleTab"
  | "commandPalette"
  | "splitVertical"
  | "splitHorizontal";

/** In percent of the miniature window. */
export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface SceneFrame {
  tiles: Record<TileId, { rect: Rect; shown: boolean }>;
  focused: TileId;
  zoomed: TileId | null;
  /** The tile picker's selection while it's open. */
  picker: TileId | null;
  /** Which of tile "a"'s tabs is in front. */
  activeTab: number;
  /** The command palette while it's open: how much of the search has been
   * typed (0 to 1) and which of its commands is selected. */
  palette: { typed: number; selected: number } | null;
  key: SceneKey | null;
}

export const NAV_SCENES: readonly NavScene[] = ["picker", "zoom", "tabs", "split", "palette"];

export const SCENE_LENGTH: Record<NavScene, number> = { picker: 6, zoom: 5, tabs: 4, split: 5, palette: 7 };

/** The beat that stands for the whole scene when motion is reduced. */
export const STILL_FRAME: Record<NavScene, number> = { picker: 2, zoom: 1, tabs: 1, split: 3, palette: 4 };

export const FULL: Rect = { left: 0, top: 0, width: 100, height: 100 };
const LEFT: Rect = { left: 0, top: 0, width: 50, height: 100 };
const RIGHT: Rect = { left: 50, top: 0, width: 50, height: 100 };
const TOP_RIGHT: Rect = { left: 50, top: 0, width: 50, height: 50 };
const BOTTOM_RIGHT: Rect = { left: 50, top: 50, width: 50, height: 50 };
// Where a tile grows from when a split brings it in.
const RIGHT_EDGE: Rect = { left: 100, top: 0, width: 0, height: 100 };
const BOTTOM_RIGHT_EDGE: Rect = { left: 50, top: 100, width: 50, height: 0 };

function threeTiles(): SceneFrame {
  return {
    tiles: {
      a: { rect: LEFT, shown: true },
      b: { rect: TOP_RIGHT, shown: true },
      c: { rect: BOTTOM_RIGHT, shown: true },
    },
    focused: "a",
    zoomed: null,
    picker: null,
    activeTab: 0,
    palette: null,
    key: null,
  };
}

export function sceneFrame(scene: NavScene, frame: number): SceneFrame {
  const view = threeTiles();
  switch (scene) {
    case "picker": {
      const selection: (TileId | null)[] = [null, "a", "b", "c", null, null];
      const keys: (SceneKey | null)[] = [null, "zoomDown", "right", "down", "enter", null];
      view.picker = selection[frame] ?? null;
      view.key = keys[frame] ?? null;
      view.focused = frame >= 4 ? "c" : "a";
      return view;
    }
    case "zoom":
      view.focused = "b";
      view.zoomed = frame === 1 || frame === 2 ? "b" : null;
      view.key = frame === 1 ? "zoomUp" : frame === 3 ? "zoomDown" : null;
      return view;
    case "tabs":
      view.activeTab = frame % 3;
      view.key = frame > 0 ? "cycleTab" : null;
      return view;
    case "split":
      if (frame === 0) {
        view.tiles = {
          a: { rect: FULL, shown: true },
          b: { rect: RIGHT_EDGE, shown: false },
          c: { rect: BOTTOM_RIGHT_EDGE, shown: false },
        };
      } else if (frame < 3) {
        view.tiles.b = { rect: RIGHT, shown: true };
        view.tiles.c = { rect: BOTTOM_RIGHT_EDGE, shown: false };
        view.focused = "b";
      } else {
        view.focused = "c";
      }
      view.key = frame === 1 ? "splitVertical" : frame === 3 ? "splitHorizontal" : null;
      return view;
    // Two tiles; the palette opens, a search is typed, the second command it
    // finds is picked, and running it splits the focused tile.
    case "palette": {
      const ran = frame >= 5;
      view.tiles.b = { rect: ran ? TOP_RIGHT : RIGHT, shown: true };
      view.tiles.c = { rect: ran ? BOTTOM_RIGHT : BOTTOM_RIGHT_EDGE, shown: ran };
      view.focused = ran ? "c" : "b";
      if (frame >= 1 && frame <= 4) {
        view.palette = { typed: frame === 1 ? 0 : frame === 2 ? 0.5 : 1, selected: frame === 4 ? 1 : 0 };
      }
      const keys: (SceneKey | null)[] = [null, "commandPalette", null, null, "down", "enter", null];
      view.key = keys[frame] ?? null;
      return view;
    }
  }
}
