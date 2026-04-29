<#
.SYNOPSIS
    Build, publish, and pack ImageViewer as a Velopack release.

.DESCRIPTION
    Cleans previous output, runs dotnet publish self-contained for win-x64,
    then runs vpk pack to produce Setup.exe and the .nupkg files inside
    the Releases\ folder.

    Run this from anywhere - the script resolves its own location and
    operates on the repo root.

    Requires the Velopack CLI (vpk). Install it once with:
        dotnet tool install -g vpk

.PARAMETER Version
    SemVer to stamp on the Velopack package. If omitted, the script reads
    <Version> from ImageViewer\ImageViewer.csproj.

.EXAMPLE
    .\Build-Release.ps1
    .\Build-Release.ps1 -Version 1.0.1
#>

[CmdletBinding()]
param(
    [string]$Version
)

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
Push-Location $RepoRoot

try {
    if (-not $Version) {
        $csprojPath = Join-Path $RepoRoot 'ImageViewer\ImageViewer.csproj'
        $xml = [xml](Get-Content -LiteralPath $csprojPath)
        $Version = ($xml.Project.PropertyGroup | Where-Object { $_.Version }).Version
        if (-not $Version) { throw "Could not read <Version> from $csprojPath" }
        Write-Host "Using version from csproj: $Version" -ForegroundColor Cyan
    }

    Write-Host "==> Cleaning publish\ and Releases\" -ForegroundColor Yellow
    Remove-Item -Recurse -Force 'publish', 'Releases' -ErrorAction SilentlyContinue

    Write-Host "==> dotnet publish (self-contained, win-x64)" -ForegroundColor Yellow
    dotnet publish ImageViewer\ImageViewer.csproj -c Release -r win-x64 -o publish
    if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed (exit $LASTEXITCODE)" }

    Write-Host "==> vpk pack $Version" -ForegroundColor Yellow
    vpk pack `
        --packId Dragon.ImageViewer `
        --packTitle "Image Viewer" `
        --packAuthors "DragonOfMercy" `
        --packVersion $Version `
        --packDir publish `
        --mainExe ImageViewer.exe `
        --icon ImageViewer\ImageViewer.ico `
        --shortcuts StartMenuRoot
    if ($LASTEXITCODE -ne 0) { throw "vpk pack failed (exit $LASTEXITCODE)" }

    Write-Host ""
    Write-Host "==> Done. Artifacts in Releases\:" -ForegroundColor Green
    Get-ChildItem Releases | Format-Table Name, @{Name='Size';Expression={'{0:N1} MB' -f ($_.Length / 1MB)}} -AutoSize
}
finally {
    Pop-Location
}
