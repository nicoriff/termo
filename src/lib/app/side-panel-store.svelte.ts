import {
  EMPTY_LAYOUT,
  closePanel,
  expand,
  groupOf,
  isExpanded,
  moveIntoGroup,
  openAsGroup,
  openInAccordion,
  resizeGroups,
  sanitizeLayout,
  toggleExpanded,
  type DropSide,
  type SideLayout,
  type SidePanelId,
} from "./side-panel-layout";

/** The column's width: a default, and how far it can be dragged. A section
 * that needs more room raises the minimum while it's in the column. */
export const SIDE_PANEL_WIDTH = { initial: 264, min: 200, max: 760 };
const PANEL_MIN_WIDTH: Partial<Record<SidePanelId, number>> = { harness: 300, tabGroups: 220 };
/** The agents panel's key, where the shared width always lived. */
const WIDTH_KEY = "termo.agents.sidebarWidth";
const LAYOUT_KEY = "termo.sidePanel.layout";

function loadWidth(): number {
  try {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved > 0 ? saved : SIDE_PANEL_WIDTH.initial;
  } catch {
    return SIDE_PANEL_WIDTH.initial;
  }
}

function loadLayout(): { layout: SideLayout; visible: boolean } {
  try {
    const raw = localStorage.getItem(LAYOUT_KEY);
    if (!raw) return { layout: EMPTY_LAYOUT, visible: false };
    const parsed = JSON.parse(raw) as { visible?: unknown };
    return { layout: sanitizeLayout(parsed), visible: parsed.visible === true };
  } catch {
    return { layout: EMPTY_LAYOUT, visible: false };
  }
}

/** What's being dragged toward the column: a section, from the activity
 * bar or from its header. */
export interface SidePanelDrag {
  id: SidePanelId;
  /** Where it would land if let go now. */
  target: { kind: "group"; group: number; side: DropSide } | { kind: "accordion"; group: number; position: number } | { kind: "column" } | null;
}

/**
 * The side panel docked at the window's left edge: groups stacked top to
 * bottom, each an accordion of sections (explorer, Git, Harness...). Each
 * section's store asks it whether it's showing (`isShown`), so what a panel
 * does only while open keeps working, and stops while it's folded.
 */
class SidePanelStore {
  layout = $state<SideLayout>(EMPTY_LAYOUT);
  /** Hidden from the activity bar, the column keeps what it had. */
  visible = $state(false);
  private rawWidth = $state(loadWidth());
  drag = $state<SidePanelDrag | null>(null);

  constructor() {
    const saved = loadLayout();
    this.layout = saved.layout;
    this.visible = saved.visible && saved.layout.groups.length > 0;
  }

  /** On screen: visible, with something in it. */
  get shown(): boolean {
    return this.visible && this.layout.groups.length > 0;
  }

  get minWidth(): number {
    let min = SIDE_PANEL_WIDTH.min;
    for (const group of this.layout.groups) for (const id of group.panels) min = Math.max(min, PANEL_MIN_WIDTH[id] ?? 0);
    return min;
  }

  get width(): number {
    return Math.round(Math.min(SIDE_PANEL_WIDTH.max, Math.max(this.minWidth, this.rawWidth)));
  }

  get limits(): { initial: number; min: number; max: number } {
    return { initial: Math.max(SIDE_PANEL_WIDTH.initial, this.minWidth), min: this.minWidth, max: SIDE_PANEL_WIDTH.max };
  }

  /** Its content on screen: in the column, unfolded, with the column shown. */
  isShown(id: SidePanelId): boolean {
    return this.shown && isExpanded(this.layout, id);
  }

  /** In the column, unfolded or not. */
  has(id: SidePanelId): boolean {
    return groupOf(this.layout, id) >= 0;
  }

  /** The activity bar's button: shows the section (in the topmost group's
   * accordion when it wasn't there), or hides the column when it's the one
   * already showing. */
  toggle(id: SidePanelId): void {
    if (this.isShown(id)) this.hide();
    else this.show(id);
  }

  show(id: SidePanelId): void {
    this.set(openInAccordion(this.layout, id), true);
  }

  close(id: SidePanelId): void {
    if (!this.has(id)) return;
    this.set(closePanel(this.layout, id), this.visible);
  }

  hide(): void {
    this.set(this.layout, false);
  }

  expand(group: number, id: SidePanelId): void {
    this.set(expand(this.layout, group, id), true);
  }

  /** A section's header clicked: it folds or unfolds. */
  toggleExpanded(group: number, id: SidePanelId): void {
    this.set(toggleExpanded(this.layout, group, id), true);
  }

  /** Lets go of a drag where it points. */
  drop(): void {
    const drag = this.drag;
    this.drag = null;
    if (!drag?.target) return;
    const { id, target } = drag;
    if (target.kind === "column") this.show(id);
    else if (target.kind === "group") this.set(openAsGroup(this.layout, id, target.group, target.side), true);
    else this.set(moveIntoGroup(this.layout, id, target.group, target.position), true);
  }

  resize(index: number, delta: number, min: number, persist = false): void {
    this.layout = resizeGroups(this.layout, index, delta, min);
    if (persist) this.save();
  }

  setWidth(width: number, persist = false): void {
    this.rawWidth = Math.round(Math.min(SIDE_PANEL_WIDTH.max, Math.max(this.minWidth, width)));
    if (!persist) return;
    try {
      localStorage.setItem(WIDTH_KEY, String(this.rawWidth));
    } catch {
      // Only a convenience.
    }
  }

  private set(layout: SideLayout, visible: boolean): void {
    this.layout = layout;
    this.visible = visible && layout.groups.length > 0;
    this.save();
  }

  private save(): void {
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify({ ...$state.snapshot(this.layout), visible: this.visible }));
    } catch {
      // Only a convenience: next time it starts closed.
    }
  }
}

export const sidePanelStore = new SidePanelStore();
