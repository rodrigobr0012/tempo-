<#
.SYNOPSIS
  Modo de desenvolvimento no desktop: abre o app e atualiza sozinho quando você salva.

  - Arquivos .slint (visual): a tela muda na hora, com o app aberto e os dados mantidos.
  - Arquivos .rs (regras, banco): o app é recompilado e reaberto sozinho.

.EXAMPLE
  ./scripts/dev.ps1
  Pare com Ctrl+C.

  Usa uma pasta de compilação própria (target/dev), para não conflitar com o
  target/debug/tempo-desktop.exe que você abre com dois cliques.
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:PATH = @(
    (Join-Path $env:USERPROFILE ".cargo\bin"),
    (Join-Path $env:USERPROFILE "scoop\apps\mingw-winlibs\current\bin"),
    $env:PATH
) -join ";"
$env:SLINT_LIVE_PREVIEW = "1"

if (-not (Get-Command cargo-watch -ErrorAction SilentlyContinue)) {
    Write-Host "Instalando cargo-watch (só na primeira vez)..." -ForegroundColor Cyan
    cargo install cargo-watch
}

Push-Location $root
try {
    # Observa só o código Rust; os .slint são recarregados pelo próprio app.
    cargo watch `
        -w crates/tempo-core/src `
        -w crates/tempo-store/src `
        -w crates/tempo-app/src `
        -x "run -p tempo-app --bin tempo-desktop --features live-preview --target-dir target/dev"
} finally { Pop-Location }
