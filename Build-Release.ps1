#Requires -Version 7.3
<#
.SYNOPSIS
    Build and pack ImageViewer as a Velopack release.

.DESCRIPTION
    Cleans previous output, runs cargo build --release, stages the exe and the
    native DLLs in publish\, then runs vpk pack to produce Setup.exe, the portable
    zip and the .nupkg inside the Releases\ folder.

    The package version is read from [workspace.package] version in Cargo.toml,
    so it stays in sync with the binary. Bump it before running this script.

    Run this from anywhere - the script resolves its own location.

    Requires the Velopack CLI (vpk). Install it once with:
        dotnet tool install -g vpk --version 1.2.161

    The script never tags, pushes, nor creates a GitHub release.

.EXAMPLE
    .\Build-Release.ps1
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

Push-Location $PSScriptRoot

try {
    $cargoToml = Get-Content -Raw -LiteralPath 'Cargo.toml'
    if ($cargoToml -notmatch '(?ms)^\[workspace\.package\]\s*$(?:(?!^\[).)*?^version\s*=\s*"([^"]+)"') {
        throw 'Could not read [workspace.package] version from Cargo.toml'
    }
    $Version = $Matches[1]
    Write-Host "Version (from Cargo.toml): $Version" -ForegroundColor Cyan

    if (-not (Get-Command vpk -ErrorAction SilentlyContinue)) {
        throw 'vpk not found on PATH. Install it with: dotnet tool install -g vpk --version 1.2.161'
    }

    Write-Host '==> Cleaning publish\ and Releases\' -ForegroundColor Yellow
    Remove-Item -Recurse -Force 'publish' -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force 'Releases' -ErrorAction SilentlyContinue

    Write-Host '==> cargo build --release' -ForegroundColor Yellow
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }

    Write-Host '==> Staging publish\' -ForegroundColor Yellow
    New-Item -ItemType Directory -Path 'publish' | Out-Null
    Copy-Item 'target\release\ImageViewer.exe' 'publish\'
    Copy-Item 'native\*.dll' 'publish\'
    if (-not (Test-Path 'publish\libheif.dll')) { throw 'libheif.dll is missing from publish\' }

    Write-Host "==> vpk pack $Version" -ForegroundColor Yellow
    vpk pack `
        --packId Dragon.ImageViewer `
        --packTitle "Image Viewer" `
        --packAuthors "DragonOfMercy" `
        --packVersion $Version `
        --packDir publish `
        --mainExe ImageViewer.exe `
        --icon app\assets\ImageViewer.ico `
        --shortcuts StartMenuRoot `
        --delta None `
        --aumid velopack.Dragon.ImageViewer
    if ($LASTEXITCODE -ne 0) { throw "vpk pack failed (exit $LASTEXITCODE)" }

    # On failure the throw above skips this, so publish\ stays in place for inspection.
    Write-Host '==> Cleaning publish\ (pack succeeded)' -ForegroundColor Yellow
    Remove-Item -Recurse -Force 'publish' -ErrorAction SilentlyContinue

    Write-Host ''
    Write-Host '==> Done. Artifacts in Releases\:' -ForegroundColor Green
    Get-ChildItem Releases | Format-Table Name, @{Name = 'Size'; Expression = { '{0:N1} MB' -f ($_.Length / 1MB) }} -AutoSize
}
finally {
    Pop-Location
}
