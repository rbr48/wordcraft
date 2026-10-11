<#
.SYNOPSIS
  Builds the WordCraft release binaries and compiles the Windows installer.

.DESCRIPTION
  1. Sets up the MinGW toolchain path.
  2. Compiles wordcraft.exe and wordcraft-cli.exe in release mode.
  3. Copies the binaries into the portable distribution folder.
  4. Invokes Inno Setup 6 (ISCC) to build the installer EXE.

.PARAMETER IsccPath
  Path to ISCC.exe. Defaults to the standard Inno Setup 6 installation path.

.EXAMPLE
  .\packaging\windows\build-installer.ps1
  .\packaging\windows\build-installer.ps1 -IsccPath "D:\Tools\InnoSetup6\ISCC.exe"
#>

param(
    [string]$IsccPath = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Version  = "0.1.0"
$PortDir  = Join-Path $RepoRoot "dist\wordcraft\wordcraft-$Version-windows-x64-portable"

Write-Host ""
Write-Host "=== WordCraft Windows Installer Builder ===" -ForegroundColor Cyan
Write-Host ""

# ── Step 1: Set up toolchain ─────────────────────────────────────────────────
Write-Host "[1/4] Configuring MinGW toolchain..." -ForegroundColor Yellow
$mingw = "$env:USERPROFILE\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.WinGet.Source_8wekyb3d8bbwe\mingw64\bin"
if (Test-Path $mingw) {
    $env:PATH = "$mingw;$env:USERPROFILE\.cargo\bin;$env:PATH"
    Write-Host "  MinGW found at: $mingw"
} else {
    Write-Warning "  MinGW not found at expected path; hoping it's already on PATH."
}

# ── Step 2: Build release binaries ───────────────────────────────────────────
Write-Host "[2/4] Building release binaries (this takes ~2 minutes)..." -ForegroundColor Yellow
Push-Location $RepoRoot
try {
    & cargo build --release -p wordcraft -p wordcraft-cli
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
} finally {
    Pop-Location
}
Write-Host "  Build complete."

# ── Step 3: Copy into portable distribution folder ────────────────────────────
Write-Host "[3/4] Copying binaries into portable distribution..." -ForegroundColor Yellow
if (-not (Test-Path $PortDir)) {
    New-Item -ItemType Directory -Path $PortDir -Force | Out-Null
}
Copy-Item -Force (Join-Path $RepoRoot "target\release\wordcraft.exe")     $PortDir\
Copy-Item -Force (Join-Path $RepoRoot "target\release\wordcraft-cli.exe") $PortDir\
Write-Host "  Copied wordcraft.exe and wordcraft-cli.exe to:"
Write-Host "  $PortDir"

# ── Step 4: Compile installer ─────────────────────────────────────────────────
Write-Host "[4/4] Compiling installer with Inno Setup..." -ForegroundColor Yellow
if (-not (Test-Path $IsccPath)) {
    Write-Error @"
ISCC.exe not found at: $IsccPath
Please install Inno Setup 6 from https://jrsoftware.org/isdl.php
or pass -IsccPath to this script.
"@
}

$IssFile = Join-Path $PSScriptRoot "wordcraft.iss"
& $IsccPath $IssFile
if ($LASTEXITCODE -ne 0) { throw "ISCC.exe failed with exit code $LASTEXITCODE" }

$SetupExe = Join-Path $PSScriptRoot "Output\wordcraft-$Version-windows-x64-setup.exe"
Write-Host ""
Write-Host "=== Done! ===" -ForegroundColor Green
Write-Host ""
Write-Host "Installer: $SetupExe" -ForegroundColor White
Write-Host ""
Write-Host "Share or double-click the installer to install WordCraft." -ForegroundColor White
Write-Host "During install, tick the file types you want WordCraft to open by default." -ForegroundColor White
