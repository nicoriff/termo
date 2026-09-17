// PSReadLine ships with every PowerShell 5.1+/pwsh install; the try/catch is
// for the stripped-down install that lacks it. It used to be a
// `Get-Module -ListAvailable PSReadLine` guard, which scans every directory in
// PSModulePath: measured at ~290ms of a three-console layout restore on this
// machine, for an answer the shell itself already has. `-ErrorAction` is no
// substitute — a missing command throws before any parameter is bound, so only
// try/catch swallows it.
const PSREADLINE_MENU_COMPLETE_SCRIPT =
  "try { " +
  "Set-PSReadLineKeyHandler -Key Tab -Function Complete; " +
  "Set-PSReadLineKeyHandler -Chord 'Tab,Tab' -Function MenuComplete } catch {}";

const POWERSHELL_IDS = new Set(["pwsh", "powershell"]);

/** Appends a PSReadLine startup command (double-Tab opens a navigable
 * completion menu, like oh-my-zsh) to a PowerShell/pwsh session's args —
 * only when the user opted in via Settings, and only for those two shells.
 * Runs after the user's own $profile (no -NoProfile passed), so it layers
 * on top rather than replacing their own PSReadLine customizations. */
export function resolveShellArgs(
  shellId: string | undefined,
  baseArgs: string[] | undefined,
  menuCompleteEnabled: boolean,
): string[] | undefined {
  if (!menuCompleteEnabled || !shellId || !POWERSHELL_IDS.has(shellId)) return baseArgs;
  return [...(baseArgs ?? []), "-NoExit", "-Command", PSREADLINE_MENU_COMPLETE_SCRIPT];
}
