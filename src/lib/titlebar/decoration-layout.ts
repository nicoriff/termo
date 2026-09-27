/**
 * Where Linux puts the titlebar's window buttons. Parses GTK's
 * `gtk-decoration-layout` (see decoration_layout in window_commands.rs):
 * "appmenu:minimize,maximize,close" — names left of the colon go on the
 * left, the rest on the right. Anything but these three buttons (appmenu,
 * icon, spacer) is ignored; this bar has its own menu button.
 */
export type WindowButton = "minimize" | "maximize" | "close";

export interface DecorationLayout {
  start: WindowButton[];
  end: WindowButton[];
  /** The side the app menu goes on: opposite the close button, like GNOME. */
  menuSide: "start" | "end";
}

const BUTTONS: readonly string[] = ["minimize", "maximize", "close"];

export const DEFAULT_DECORATION_LAYOUT: DecorationLayout = {
  start: [],
  end: ["minimize", "maximize", "close"],
  menuSide: "start",
};

export function parseDecorationLayout(value: string | null | undefined): DecorationLayout {
  if (!value) return DEFAULT_DECORATION_LAYOUT;
  const seen = new Set<string>();
  const side = (part: string | undefined): WindowButton[] =>
    (part ?? "")
      .split(",")
      .map((name) => name.trim())
      .filter((name) => BUTTONS.includes(name) && !seen.has(name) && seen.add(name)) as WindowButton[];
  const colon = value.indexOf(":");
  // No colon: GTK puts everything on the left.
  const start = side(colon === -1 ? value : value.slice(0, colon));
  const end = side(colon === -1 ? "" : value.slice(colon + 1));
  return { start, end, menuSide: start.includes("close") ? "end" : "start" };
}
