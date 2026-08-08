$ErrorActionPreference = "Stop"

$revision = "5a110e78a8440921d7d4302769bc049180f9d2bf"
$artifactRoot = Split-Path -Parent $PSScriptRoot
$vendorRoot = Join-Path $artifactRoot "vendor"
$sourceRoot = Join-Path $vendorRoot "Auwgent-$revision"
$patchPath = Join-Path $artifactRoot "patches\parser-hardening.patch"

if (-not (Test-Path -LiteralPath $sourceRoot)) {
    New-Item -ItemType Directory -Force -Path $vendorRoot | Out-Null
    $archive = Join-Path $vendorRoot "Auwgent-$revision.tar.gz"
    $url = "https://codeload.github.com/snrraptopack/Auwgent/tar.gz/$revision"

    Write-Host "Downloading immutable Auwgent source revision $revision"
    curl.exe -L --fail --silent --show-error $url -o $archive
    tar -xzf $archive -C $vendorRoot
    Remove-Item -LiteralPath $archive
}

if (-not (Test-Path -LiteralPath $sourceRoot)) {
    throw "Archive did not produce expected source directory: $sourceRoot"
}

$previousCeiling = $env:GIT_CEILING_DIRECTORIES
$env:GIT_CEILING_DIRECTORIES = $artifactRoot
Push-Location $sourceRoot
try {
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "SilentlyContinue"
    git apply --reverse --check $patchPath 2>$null
    $reverseCheck = $LASTEXITCODE
    $ErrorActionPreference = $previousPreference
    if ($reverseCheck -eq 0) {
        Write-Host "Pinned source and parser-hardening patch already prepared: $sourceRoot"
        exit 0
    }

    $ErrorActionPreference = "SilentlyContinue"
    git apply --check $patchPath
    $forwardCheck = $LASTEXITCODE
    $ErrorActionPreference = $previousPreference
    if ($forwardCheck -ne 0) {
        throw "Pinned source is neither pristine nor patched; remove vendor/Auwgent-$revision and retry"
    }
    git apply $patchPath
    if ($LASTEXITCODE -ne 0) {
        throw "Could not apply parser-hardening patch"
    }
} finally {
    Pop-Location
    $env:GIT_CEILING_DIRECTORIES = $previousCeiling
}

Write-Host "Prepared pinned source plus parser-hardening patch: $sourceRoot"
