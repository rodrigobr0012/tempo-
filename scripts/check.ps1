<#
.SYNOPSIS
  Verificações de qualidade (RNF23, RNF24): formatação, clippy e testes.
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$env:PATH = @(
    (Join-Path $env:USERPROFILE ".cargo\bin"),
    (Join-Path $env:USERPROFILE "scoop\apps\mingw-winlibs\current\bin"),
    $env:PATH
) -join ";"
Push-Location $root
try {
    cargo fmt --all -- --check;               if ($LASTEXITCODE -ne 0) { throw "cargo fmt" }
    cargo clippy --workspace --all-targets -- -D warnings; if ($LASTEXITCODE -ne 0) { throw "cargo clippy" }
    cargo test --workspace;                   if ($LASTEXITCODE -ne 0) { throw "cargo test" }
} finally { Pop-Location }
