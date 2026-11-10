<#
.SYNOPSIS
  Modo de desenvolvimento da versão web: serve em http://127.0.0.1:8080 e recarrega a página
  sozinho quando você salva um arquivo.

.EXAMPLE
  ./scripts/dev-web.ps1
  Pare com Ctrl+C.
#>
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "web-env.ps1")

Push-Location (Join-Path $root "crates\tempo-web")
try {
    trunk serve
} finally { Pop-Location }
