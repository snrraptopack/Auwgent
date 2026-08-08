$ErrorActionPreference = "Stop"
$artifactRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
& (Join-Path $artifactRoot "scripts\prepare-source.ps1")
Push-Location $artifactRoot
try {
    cargo run --release -- @args
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    Pop-Location
}

