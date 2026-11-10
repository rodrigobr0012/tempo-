<#
.SYNOPSIS
  Compila a biblioteca Rust para Android e empacota o APK.

.EXAMPLE
  ./scripts/build-android.ps1              # APK release (arm64-v8a, x86_64 e universal)
  ./scripts/build-android.ps1 -Debug       # APK debug
  ./scripts/build-android.ps1 -Install     # compila e instala no aparelho conectado (adb)

  Variáveis opcionais: ANDROID_HOME, ANDROID_NDK_ROOT, JAVA_HOME (JDK 17).
#>
param(
    [switch]$Debug,
    [switch]$Install
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$ndkVersion = "28.2.13676358"
$platform = "android-36"

if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = "E:\toolchains\android-sdk" }
if (-not $env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT = Join-Path $env:ANDROID_HOME "ndk\$ndkVersion" }
if (-not $env:JAVA_HOME) { $env:JAVA_HOME = Join-Path $env:USERPROFILE "scoop\apps\temurin17-jdk\current" }
$env:ANDROID_JAR = Join-Path $env:ANDROID_HOME "platforms\$platform\android.jar"
$env:PATH = @(
    (Join-Path $env:USERPROFILE ".cargo\bin"),
    (Join-Path $env:USERPROFILE "scoop\apps\mingw-winlibs\current\bin"),
    (Join-Path $env:JAVA_HOME "bin"),
    $env:PATH
) -join ";"

foreach ($p in @($env:ANDROID_HOME, $env:ANDROID_NDK_ROOT, $env:JAVA_HOME, $env:ANDROID_JAR)) {
    if (-not (Test-Path $p)) { throw "Não encontrado: $p (veja o README, seção Ambiente)" }
}

Write-Host "==> Rust -> libtempo_app.so (arm64-v8a, x86_64)" -ForegroundColor Cyan
$cargoArgs = @("ndk", "-t", "arm64-v8a", "-t", "x86_64", "-P", "26",
    "-o", (Join-Path $root "android\app\src\main\jniLibs"),
    "build", "-p", "tempo-app", "--lib")
if (-not $Debug) { $cargoArgs += "--release" }
Push-Location $root
try {
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo ndk falhou" }
} finally { Pop-Location }

Write-Host "==> Gradle -> APK" -ForegroundColor Cyan
$sdk = $env:ANDROID_HOME.Replace('\', '/').Replace(':', '\:')
Set-Content -Path (Join-Path $root "android\local.properties") -Value "sdk.dir=$sdk" -Encoding ascii
$task = if ($Debug) { "assembleDebug" } else { "assembleRelease" }
Push-Location (Join-Path $root "android")
try {
    & .\gradlew.bat $task --no-daemon
    if ($LASTEXITCODE -ne 0) { throw "Gradle falhou" }
} finally { Pop-Location }

$variant = if ($Debug) { "debug" } else { "release" }
$outDir = Join-Path $root "android\app\build\outputs\apk\$variant"
Get-ChildItem $outDir -Filter *.apk | ForEach-Object {
    "{0,-40} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB)
}

if ($Install) {
    $adb = Join-Path $env:ANDROID_HOME "platform-tools\adb.exe"
    $apk = Get-ChildItem $outDir -Filter "*universal*.apk" | Select-Object -First 1
    Write-Host "==> Instalando $($apk.Name)" -ForegroundColor Cyan
    & $adb install -r $apk.FullName
}
