# Usage: powershell -File screenshot-app.ps1 -OutPath ".\shot.png" [-ProcessName termo]
# Captures the app window's own content via PrintWindow (immune to
# multi-monitor/DPI-scaling coordinate issues that broke CopyFromScreen here).
param(
  [string]$OutPath = "E:\Proyectos\Termini\.screenshot.png",
  [string]$ProcessName = "termo"
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ScreenshotWin32 {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdcBlt, uint nFlags);
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
}
"@
[void][ScreenshotWin32]::SetProcessDpiAwarenessContext([IntPtr]-4) # PER_MONITOR_AWARE_V2

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host "PROCESS_NOT_FOUND: $ProcessName"; exit 1 }

$rect = New-Object ScreenshotWin32+RECT
[ScreenshotWin32]::GetClientRect($proc.MainWindowHandle, [ref]$rect) | Out-Null
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top

Add-Type -AssemblyName System.Drawing
$bmp = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $graphics.GetHdc()
[ScreenshotWin32]::PrintWindow($proc.MainWindowHandle, $hdc, 2) | Out-Null # PW_RENDERFULLCONTENT
$graphics.ReleaseHdc($hdc)
$bmp.Save($OutPath, [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose(); $bmp.Dispose()

Write-Host "Saved: $OutPath ($width x $height)"
