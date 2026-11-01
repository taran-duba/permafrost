<#
.SYNOPSIS
  Publishes the SwRefs sidecar into src-tauri/binaries/ for Tauri's externalBin.

.DESCRIPTION
  Runs a self-contained, single-file win-x64 `dotnet publish` of sidecar/SwRefs and copies the
  result to src-tauri/binaries/SwRefs-<target triple>.exe, the name Tauri's externalBin expects.
  `pnpm tauri build` runs this on Windows through beforeBuildCommand (src-tauri/tauri.windows.conf.json).

  SwRefs talks to SolidWorks through late-bound COM, so building it needs only the .NET 8 SDK:
  no SolidWorks, no interop DLL and no license key.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/publish-swrefs.ps1
#>
param(
    [string]$TargetTriple = "x86_64-pc-windows-msvc",
    [string]$Rid = "win-x64"
)

$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo "sidecar/SwRefs/SwRefs.csproj"
$publishDir = Join-Path $repo "sidecar/SwRefs/bin/publish-$Rid"
$binDir = Join-Path $repo "src-tauri/binaries"
$dest = Join-Path $binDir "SwRefs-$TargetTriple.exe"

if (Test-Path -LiteralPath $publishDir) {
    Remove-Item -LiteralPath $publishDir -Recurse -Force
}

& dotnet publish $project -c Release -r $Rid --self-contained true -o $publishDir `
    -p:PublishSingleFile=true -p:EnableCompressionInSingleFile=true -p:DebugType=None
if ($LASTEXITCODE -ne 0) {
    throw "dotnet publish failed with exit code $LASTEXITCODE."
}

$exe = Join-Path $publishDir "SwRefs.exe"
if (-not (Test-Path -LiteralPath $exe)) {
    throw "dotnet publish did not produce $exe."
}

New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item -LiteralPath $exe -Destination $dest -Force
Write-Host "Wrote $dest"
