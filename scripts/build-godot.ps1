# PowerShell script to build the Rust GDExtension and stage dynamic libraries into godot/bin/
param(
    [string]$Configuration = "release"
)

$ErrorActionPreference = "Stop"

Write-Host "Building street-smarts-godot crate ($Configuration)..." -ForegroundColor Cyan

$destDir = "godot/bin"
if (-not (Test-Path $destDir)) {
    New-Item -ItemType Directory -Path $destDir | Out-Null
}

$targetFlag = if ($Configuration -eq "release") { "--release" } else { "" }
$srcDll = if ($Configuration -eq "release") { "target/release/street_smarts_godot.dll" } else { "target/debug/street_smarts_godot.dll" }

Write-Host "Command: cargo build --package street-smarts-godot $targetFlag" -ForegroundColor Gray
Write-Host "Target output binary will be staged to: $destDir/street_smarts_godot.dll" -ForegroundColor Gray
