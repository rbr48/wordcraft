<#
.SYNOPSIS
  Builds and packages the WordCraft Android APK for version 0.4.1.

.DESCRIPTION
  1. Locates the Android SDK build-tools, platform android.jar, and JDK.
  2. Compiles Android XML resources and generates the base APK using aapt2.
  3. Bundles the native shared libraries (arm64-v8a and x86_64).
  4. 4-byte aligns the APK using zipalign.
  5. Generates/uses a debug keystore and signs the APK with apksigner (v1, v2, v3 schemes).
  6. Outputs the signed APK to dist/wordcraft/wordcraft-0.4.1.apk and packaging/android/Output/.
#>

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepoRoot   = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Version    = "0.6.1"
$BuildDir   = Join-Path $PSScriptRoot "build"
$OutputDir  = Join-Path $PSScriptRoot "Output"
$DistDir    = Join-Path $RepoRoot "dist\wordcraft"

Write-Host ""
Write-Host "=== WordCraft Android APK Builder ===" -ForegroundColor Cyan
Write-Host "Version: $Version" -ForegroundColor Gray
Write-Host ""

# ── Step 1: Toolchain Discovery ──────────────────────────────────────────────
$SdkDir = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "$env:LOCALAPPDATA\Android\Sdk" }
$BuildToolsDir = Join-Path $SdkDir "build-tools\34.0.0"
if (-not (Test-Path $BuildToolsDir)) {
    $BuildToolsDir = (Get-ChildItem (Join-Path $SdkDir "build-tools") | Sort-Object Name -Descending | Select-Object -First 1).FullName
}

$Aapt2    = Join-Path $BuildToolsDir "aapt2.exe"
$ZipAlign = Join-Path $BuildToolsDir "zipalign.exe"
$ApkSigner = Join-Path $BuildToolsDir "apksigner.bat"
$AndroidJar = Join-Path $SdkDir "platforms\android-34\android.jar"
if (-not (Test-Path $AndroidJar)) {
    $AndroidJar = (Get-ChildItem (Join-Path $SdkDir "platforms") -Filter "android.jar" -Recurse | Select-Object -First 1).FullName
}

$JdkBin = "C:\Program Files\Microsoft\jdk-17.0.20.8-hotspot\bin"
$KeyTool = Join-Path $JdkBin "keytool.exe"
$JarTool = Join-Path $JdkBin "jar.exe"

Write-Host "Using Android Build Tools: $BuildToolsDir" -ForegroundColor Gray
Write-Host "Using Android Platform:    $AndroidJar" -ForegroundColor Gray
Write-Host "Using JDK Tools:           $JdkBin" -ForegroundColor Gray

# ── Step 2: Check Native Libraries ───────────────────────────────────────────
$Arm64So = Join-Path $RepoRoot "target\aarch64-linux-android\release\libwordcraft_android.so"
$X64So   = Join-Path $RepoRoot "target\x86_64-linux-android\release\libwordcraft_android.so"

if (-not (Test-Path $Arm64So)) {
    throw "ARM64 library not found at $Arm64So. Run 'cargo ndk --target aarch64-linux-android build --release -p wordcraft-android' first."
}

# ── Step 3: Setup Working Folders ────────────────────────────────────────────
if (Test-Path $BuildDir) { Remove-Item -Recurse -Force $BuildDir }
New-Item -ItemType Directory -Path $BuildDir -Force | Out-Null
New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

$ResDir = Join-Path $PSScriptRoot "res"
$Manifest = Join-Path $PSScriptRoot "AndroidManifest.xml"

# ── Step 4: Compile Resources with AAPT2 ──────────────────────────────────────
Write-Host "[1/5] Compiling resources with aapt2..." -ForegroundColor Yellow
$CompiledRes = Join-Path $BuildDir "compiled_res.zip"
& $Aapt2 compile --dir $ResDir -o $CompiledRes
if ($LASTEXITCODE -ne 0) { throw "aapt2 compile failed with exit code $LASTEXITCODE" }

Write-Host "[2/5] Linking APK package..." -ForegroundColor Yellow
$BaseApk = Join-Path $BuildDir "base.apk"
& $Aapt2 link -I $AndroidJar --manifest $Manifest -o $BaseApk $CompiledRes
if ($LASTEXITCODE -ne 0) { throw "aapt2 link failed with exit code $LASTEXITCODE" }

# ── Step 5: Bundle Native Libraries ──────────────────────────────────────────
Write-Host "[3/5] Bundling native shared libraries into APK..." -ForegroundColor Yellow
$LibStaging = Join-Path $BuildDir "lib"
$Arm64Dir = Join-Path $LibStaging "arm64-v8a"
$X64Dir   = Join-Path $LibStaging "x86_64"

New-Item -ItemType Directory -Path $Arm64Dir -Force | Out-Null
Copy-Item -Force $Arm64So $Arm64Dir\

if (Test-Path $X64So) {
    New-Item -ItemType Directory -Path $X64Dir -Force | Out-Null
    Copy-Item -Force $X64So $X64Dir\
    Write-Host "  Included arm64-v8a and x86_64 architectures." -ForegroundColor Green
} else {
    Write-Host "  Included arm64-v8a architecture." -ForegroundColor Green
}

Push-Location $BuildDir
try {
    & $JarTool -uf $BaseApk lib/
    if ($LASTEXITCODE -ne 0) { throw "jar -uf failed with exit code $LASTEXITCODE" }
} finally {
    Pop-Location
}

# ── Step 6: 4-Byte Page Align with Zipalign ──────────────────────────────────
Write-Host "[4/5] Aligning APK with zipalign..." -ForegroundColor Yellow
$AlignedApk = Join-Path $BuildDir "aligned.apk"
& $ZipAlign -f -p 4 $BaseApk $AlignedApk
if ($LASTEXITCODE -ne 0) { throw "zipalign failed with exit code $LASTEXITCODE" }

# ── Step 7: Sign APK with apksigner ──────────────────────────────────────────
Write-Host "[5/5] Signing APK..." -ForegroundColor Yellow
$Keystore = Join-Path $BuildDir "debug.keystore"
if (-not (Test-Path $Keystore)) {
    & $KeyTool -genkey -v -keystore $Keystore -storepass android -alias androiddebugkey -keypass android -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=WordCraft,O=StoryTold,C=US"
    if ($LASTEXITCODE -ne 0) { throw "keytool failed to generate debug keystore" }
}

$FinalApkOutput = Join-Path $OutputDir "wordcraft-$Version.apk"
$FinalApkDist   = Join-Path $DistDir "wordcraft-$Version.apk"

& $ApkSigner sign --ks $Keystore --ks-pass pass:android --ks-key-alias androiddebugkey --key-pass pass:android --out $FinalApkOutput $AlignedApk
if ($LASTEXITCODE -ne 0) { throw "apksigner sign failed with exit code $LASTEXITCODE" }

Copy-Item -Force $FinalApkOutput $FinalApkDist

Write-Host "  Verifying signature..." -ForegroundColor Gray
& $ApkSigner verify $FinalApkOutput
if ($LASTEXITCODE -ne 0) { throw "apksigner verify failed" }

Write-Host ""
Write-Host "=== Done! ===" -ForegroundColor Green
Write-Host "Signed APK: $FinalApkOutput" -ForegroundColor White
Write-Host "Dist APK:   $FinalApkDist" -ForegroundColor White
$ApkSize = (Get-Item $FinalApkOutput).Length / 1MB
Write-Host ("Size:       {0:N2} MB" -f $ApkSize) -ForegroundColor White
Write-Host ""
Write-Host "Install on any connected Android device or emulator with:" -ForegroundColor Gray
Write-Host "  adb install -r `"$FinalApkOutput`"" -ForegroundColor Cyan
