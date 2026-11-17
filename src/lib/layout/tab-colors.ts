/** Which of the theme's tab colors each tab takes (see theme-paint's
 * tabPalette): the next one in turn as it first shows, and that one for
 * good, so closing or moving a tab never repaints the others. */
const assigned = new Map<string, number>();
let next = 0;

export function tabColorIndex(tabId: string): number {
  let index = assigned.get(tabId);
  if (index === undefined) {
    index = next++;
    assigned.set(tabId, index);
  }
  return index;
}
