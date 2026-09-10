# Dot-source this before any cargo/tauri command:
#   . E:\Proyectos\Termini\dev-env.ps1
#
# Rust toolchain lives on D: (C: has almost no free space).
# MSVC 2022 Enterprise's C++ workload is missing lib\x64 (only "onecore" present),
# so we use the complete 2017 BuildTools toolset for cl/link/lib instead.
# The registry's WindowsSdkDir (KitsRoot10) points at a path that no longer
# exists on this machine; the real SDK files live on E:, so INCLUDE/LIB/PATH
# are set explicitly rather than relying on vcvarsall.bat's auto-detection.

$env:RUSTUP_HOME = "D:\Toolchains\rustup"
$env:CARGO_HOME = "D:\Toolchains\cargo"

# C: has almost no free space, and rustc/link.exe write intermediate object
# files to %TEMP% — leaving it on C: causes corrupted/truncated artifacts
# ("invalid metadata files", "found staticlib std instead of rlib").
New-Item -ItemType Directory -Force -Path "D:\Toolchains\temp" | Out-Null
$env:TEMP = "D:\Toolchains\temp"
$env:TMP = "D:\Toolchains\temp"

$msvc = "C:\Program Files (x86)\Microsoft Visual Studio\2017\BuildTools\VC\Tools\MSVC\14.16.27023"
$sdk = "E:\Windows Kits\10"
$sdkver = "10.0.17763.0"

$env:PATH = "D:\Toolchains\cargo\bin;$msvc\bin\Hostx64\x64;$sdk\bin\$sdkver\x64;" + $env:PATH
$env:INCLUDE = "$msvc\include;$sdk\Include\$sdkver\ucrt;$sdk\Include\$sdkver\um;$sdk\Include\$sdkver\shared;$sdk\Include\$sdkver\winrt"
$env:LIB = "$msvc\lib\x64;$sdk\Lib\$sdkver\ucrt\x64;$sdk\Lib\$sdkver\um\x64"
