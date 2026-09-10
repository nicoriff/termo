// PSReadLine ships with every PowerShell 5.1+/pwsh install, but guard with
// Get-Module in case a stripped-down install ever lacks it.
const PSREADLINE_MENU_COMPLETE_SCRIPT =
  "if (Get-Module -ListAvailable PSReadLine) { " +
  "Set-PSReadLineKeyHandler -Key Tab -Function Complete; " +
  "Set-PSReadLineKeyHandler -Chord 'Tab,Tab' -Function MenuComplete }";

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
