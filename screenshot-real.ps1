# Usage: powershell -File screenshot-real.ps1 -OutPath ".\shot.png" [-ProcessName termo]
# Captures the real composited desktop pixels behind the app window via
# CopyFromScreen (DPI-aware) -- unlike screenshot-app.ps1's PrintWindow
# approach, this correctly shows true OS-level window transparency/rounded
# corners against whatever is actually behind the window.
param(
  [string]$OutPath = "E:\Proyectos\Termini\.screenshot-real.png",
  [string]$ProcessName = "termo"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class RealShotWin32 {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
[void][RealShotWin32]::SetProcessDpiAwarenessContext([IntPtr]-4) # PER_MONITOR_AWARE_V2

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host "PROCESS_NOT_FOUND: $ProcessName"; exit 1 }

[RealShotWin32]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
Start-Sleep -Milliseconds 300

$rect = New-Object RealShotWin32+RECT
[RealShotWin32]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size $width, $height))
$bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose(); $bmp.Dispose()

Write-Host "Saved: $OutPath ($width x $height)"
