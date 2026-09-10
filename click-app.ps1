# Usage: powershell -File click-app.ps1 -X <clientX> -Y <clientY> [-ProcessName termini]
# Clicks at CLIENT-area coordinates (same coordinate space as screenshot-app.ps1's
# PrintWindow capture), converting to screen coords via ClientToScreen before
# synthesizing the click. DPI-aware to match screenshot-app.ps1.
param(
  [Parameter(Mandatory=$true)][int]$X,
  [Parameter(Mandatory=$true)][int]$Y,
  [string]$ProcessName = "termini"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ClickWin32 {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
    public struct POINT { public int X; public int Y; }
}
"@
[void][ClickWin32]::SetProcessDpiAwarenessContext([IntPtr]-4) # PER_MONITOR_AWARE_V2

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host "PROCESS_NOT_FOUND: $ProcessName"; exit 1 }

[ClickWin32]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 150

$pt = New-Object ClickWin32+POINT
$pt.X = $X; $pt.Y = $Y
[ClickWin32]::ClientToScreen($proc.MainWindowHandle, [ref]$pt) | Out-Null

[ClickWin32]::SetCursorPos($pt.X, $pt.Y) | Out-Null
Start-Sleep -Milliseconds 80
[ClickWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero) # LEFTDOWN
Start-Sleep -Milliseconds 60
[ClickWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero) # LEFTUP

Write-Host "Clicked client($X,$Y) -> screen($($pt.X),$($pt.Y))"
