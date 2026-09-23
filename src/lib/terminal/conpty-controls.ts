/**
 * Windows 10's ConPTY keeps a cell for a control character a program wrote
 * with processed output off (Claude Code does this with SI, 0x0F), and when
 * it later paints that cell it sends the raw byte, counting it as one column.
 * xterm executes or ignores the byte instead, without advancing, so from then
 * on ConPTY's relative moves land one column short: typed text lands on top
 * of the prompt, and since ConPTY only resends cells it thinks changed, the
 * stray characters stay until a resize repaints everything.
 *
 * Swapping each such byte for a space gives it the one column ConPTY counted.
 * Only bytes ConPTY never uses as controls itself are touched: BEL, BS, TAB,
 * LF, CR and ESC keep their meaning.
 *
 * Pure — no runes, no xterm — so it can be unit-tested in plain Node, same
 * approach as `osc7.ts`.
 */

const PAINTED_CONTROLS = /[\x00-\x06\x0b\x0c\x0e-\x1a\x1c-\x1f]/g;

export function neutralizePaintedControls(text: string): string {
  return text.replace(PAINTED_CONTROLS, " ");
}
