<#
.SYNOPSIS
  Ambiente para compilar a versão web (usado por build-web.ps1 e dev-web.ps1).

  O SQLite é compilado em C para WebAssembly e precisa de um clang com o alvo wasm32.
  Usa, nesta ordem: CC_wasm32_unknown_unknown já definido, clang do LLVM no PATH ou o clang do
  Android NDK (que já está instalado para o app).
#>
$root = Split-Path -Parent $PSScriptRoot
$env:PATH = @(
    (Join-Path $env:USERPROFILE ".cargo\bin"),
    $env:PATH
) -join ";"

if (-not $env:CC_wasm32_unknown_unknown) {
    $clang = Get-Command clang -ErrorAction SilentlyContinue
    if ($clang -and ((& $clang.Source --print-targets) -match "wasm32")) {
        $bin = Split-Path $clang.Source
    } else {
        if (-not $env:ANDROID_HOME) { $env:ANDROID_HOME = "E:\toolchains\android-sdk" }
        $ndk = if ($env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT } else {
            Get-ChildItem (Join-Path $env:ANDROID_HOME "ndk") -Directory -ErrorAction SilentlyContinue |
                Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty FullName
        }
        if (-not $ndk) { throw "clang não encontrado: instale o LLVM (winget install LLVM.LLVM) ou o Android NDK" }
        $bin = Join-Path $ndk "toolchains\llvm\prebuilt\windows-x86_64\bin"
    }
    $env:CC_wasm32_unknown_unknown = Join-Path $bin "clang.exe"
    $env:AR_wasm32_unknown_unknown = Join-Path $bin "llvm-ar.exe"
}

# O strip do perfil release (pensado para o Android) esconde do wasm-bindgen os recursos do Wasm
# usados; o wasm-opt já remove o que sobra no fim.
$env:CARGO_PROFILE_RELEASE_STRIP = "false"

if (-not ((rustup target list --installed) -contains "wasm32-unknown-unknown")) {
    Write-Host "Instalando o alvo wasm32-unknown-unknown (só na primeira vez)..." -ForegroundColor Cyan
    rustup target add wasm32-unknown-unknown
}
if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
    Write-Host "Instalando o trunk (só na primeira vez)..." -ForegroundColor Cyan
    cargo install trunk --locked
}
