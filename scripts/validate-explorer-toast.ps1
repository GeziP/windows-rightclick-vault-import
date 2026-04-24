[CmdletBinding()]
param(
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

$ErrorActionPreference = "Stop"

function Invoke-Step {
    param(
        [string]$Title,
        [scriptblock]$Action
    )

    Write-Host ""
    Write-Host "==> $Title" -ForegroundColor Cyan
    & $Action
}

function Read-YesNo {
    param(
        [string]$Prompt
    )

    while ($true) {
        $answer = (Read-Host "$Prompt [y/n]").Trim().ToLowerInvariant()
        if ($answer -in @("y", "yes")) { return $true }
        if ($answer -in @("n", "no")) { return $false }
    }
}

$kbintakeDir = Join-Path $RepoRoot "kbintake"
$releaseDir = Join-Path $kbintakeDir "target\release"
$installDir = Join-Path $env:LOCALAPPDATA "Programs\kbintake"
$appDataDir = Join-Path $env:TEMP "kbintake-manual-check"
$sampleDir = Join-Path $env:TEMP "kbintake-toast-test"
$note1 = Join-Path $sampleDir "note1.md"
$note2 = Join-Path $sampleDir "note2.md"
$missing = Join-Path $sampleDir "missing.md"

Invoke-Step "Build release binaries" {
    Push-Location $kbintakeDir
    try {
        cargo build --release --locked --bins
    }
    finally {
        Pop-Location
    }
}

Invoke-Step "Stage binaries into $installDir" {
    New-Item -ItemType Directory -Force $installDir | Out-Null
    Copy-Item (Join-Path $releaseDir "kbintake.exe") (Join-Path $installDir "kbintake.exe") -Force
    Copy-Item (Join-Path $releaseDir "kbintakew.exe") (Join-Path $installDir "kbintakew.exe") -Force
    Copy-Item (Join-Path $kbintakeDir "assets\kbintake.ico") (Join-Path $installDir "kbintake.ico") -Force
}

Invoke-Step "Reset isolated app-data directory" {
    Remove-Item $appDataDir -Recurse -Force -ErrorAction SilentlyContinue
    $env:KBINTAKE_APP_DATA_DIR = $appDataDir
    & (Join-Path $installDir "kbintake.exe") doctor --fix
}

Invoke-Step "Register Explorer context menu against kbintakew.exe" {
    & (Join-Path $installDir "kbintake.exe") explorer install `
        --exe-path (Join-Path $installDir "kbintakew.exe") `
        --icon-path (Join-Path $installDir "kbintake.ico")
}

Invoke-Step "Create sample files" {
    Remove-Item $sampleDir -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $sampleDir | Out-Null
    Set-Content $note1 "toast smoke 1"
    Set-Content $note2 "toast smoke 1"
    Write-Host "Sample directory: $sampleDir"
}

Invoke-Step "Open sample folder in Explorer" {
    Start-Process explorer.exe $sampleDir
    Write-Host "Explorer opened. Follow the prompts in this shell."
}

Write-Host ""
Write-Host "Manual step 1:" -ForegroundColor Yellow
Write-Host "  In Explorer, right-click note1.md and trigger the KBIntake action."
[void](Read-Host "Press Enter after you have done that")
$successNoConsole = Read-YesNo "Did note1.md import without a visible console window?"
$successToast = Read-YesNo "Did a success toast appear for note1.md?"

Write-Host ""
Write-Host "Manual step 2:" -ForegroundColor Yellow
Write-Host "  In Explorer, right-click note2.md and trigger the KBIntake action."
[void](Read-Host "Press Enter after you have done that")
$duplicateNoConsole = Read-YesNo "Did note2.md import without a visible console window?"
$duplicateToast = Read-YesNo "Did a duplicate-related toast appear for note2.md?"

Invoke-Step "Trigger a failure toast through kbintakew.exe" {
    & (Join-Path $installDir "kbintakew.exe") explorer run-import $missing
    Start-Sleep -Seconds 2
}
$failureToast = Read-YesNo "Did a failure toast appear for the missing file case?"

Write-Host ""
Write-Host "Current jobs list:" -ForegroundColor Cyan
& (Join-Path $installDir "kbintake.exe") jobs list

Write-Host ""
Write-Host "Summary" -ForegroundColor Green
Write-Host "#42"
Write-Host "- No console window on success import: $successNoConsole"
Write-Host "- Success toast: $successToast"
Write-Host "- No console window on duplicate import: $duplicateNoConsole"
Write-Host "- Duplicate toast: $duplicateToast"
Write-Host "- Failure toast: $failureToast"
Write-Host "- App data dir: $appDataDir"
Write-Host "- Sample dir: $sampleDir"
