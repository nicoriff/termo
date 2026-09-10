# Usage: powershell -File type-app.ps1 -Text "hello" [-ProcessName termini]
# Sends keystrokes to the foreground app via SendKeys (assumes the target
# input already has focus, e.g. an autofocused search box).
param(
  [Parameter(Mandatory=$true)][string]$Text,
  [string]$ProcessName = "termini"
)

Add-Type -AssemblyName System.Windows.Forms

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host "PROCESS_NOT_FOUND: $ProcessName"; exit 1 }

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class TypeWin32 {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
}
"@
[void][TypeWin32]::SetForegroundWindow($proc.MainWindowHandle)
Start-Sleep -Milliseconds 150

# Only escapes literal SendKeys metacharacters, NOT curly braces -- callers
# use {ENTER}, {RIGHT}, etc. directly as real SendKeys control sequences.
$escaped = $Text -replace '([+^%~()])', '{$1}'
[System.Windows.Forms.SendKeys]::SendWait($escaped)

Write-Host "Typed: $Text"
