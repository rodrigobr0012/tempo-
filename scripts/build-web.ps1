<#
.SYNOPSIS
  Compila a versão web (WebAssembly) em crates/tempo-web/dist, pronta para publicar.

.EXAMPLE
  ./scripts/build-web.ps1           # release (otimizado)
  ./scripts/build-web.ps1 -Debug    # debug

  O conteúdo de dist/ é estático: sirva em qualquer hospedagem de arquivos (GitHub Pages,
  Netlify, Cloudflare Pages, nginx...). Não há servidor nem banco remoto.
#>
param([switch]$Debug)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "web-env.ps1")

Push-Location (Join-Path $root "crates\tempo-web")
try {
    $trunkArgs = @("build")
    if (-not $Debug) { $trunkArgs += "--release" }
    trunk @trunkArgs; if ($LASTEXITCODE -ne 0) { throw "trunk build" }
    Write-Host "==> Pronto: crates\tempo-web\dist" -ForegroundColor Green
} finally { Pop-Location }
