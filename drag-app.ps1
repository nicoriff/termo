# Usage: powershell -File drag-app.ps1 -FromX <x> -FromY <y> -ToX <x> -ToY <y> [-ProcessName termo]
# Simulates a press-move-move-...-release drag gesture at CLIENT-area
# coordinates (same space as screenshot-app.ps1's capture), so the app's own
# pointerdown/pointermove/pointerup listeners see a real drag rather than a
# single click.
param(
  [Parameter(Mandatory=$true)][int]$FromX,
  [Parameter(Mandatory=$true)][int]$FromY,
  [Parameter(Mandatory=$true)][int]$ToX,
  [Parameter(Mandatory=$true)][int]$ToY,
  [int]$Steps = 12,
  [string]$ProcessName = "termo"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class DragWin32 {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT lpPoint);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int X, int Y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
    public struct POINT { public int X; public int Y; }
}
"@
[void][DragWin32]::SetProcessDpiAwarenessContext([IntPtr]-4)

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host "PROCESS_NOT_FOUND: $ProcessName"; exit 1 }

[DragWin32]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 150

function ToScreen([int]$x, [int]$y) {
  $pt = New-Object DragWin32+POINT
  $pt.X = $x; $pt.Y = $y
  [DragWin32]::ClientToScreen($proc.MainWindowHandle, [ref]$pt) | Out-Null
  return $pt
}

$start = ToScreen $FromX $FromY
$end = ToScreen $ToX $ToY

[DragWin32]::SetCursorPos($start.X, $start.Y) | Out-Null
Start-Sleep -Milliseconds 80
[DragWin32]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero) # LEFTDOWN
Start-Sleep -Milliseconds 80

for ($i = 1; $i -le $Steps; $i++) {
  $t = $i / [double]$Steps
  $x = [int]($start.X + ($end.X - $start.X) * $t)
  $y = [int]($start.Y + ($end.Y - $start.Y) * $t)
  [DragWin32]::SetCursorPos($x, $y) | Out-Null
  Start-Sleep -Milliseconds 30
}

Start-Sleep -Milliseconds 80
[DragWin32]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero) # LEFTUP

Write-Host "Dragged client($FromX,$FromY) -> client($ToX,$ToY)"
