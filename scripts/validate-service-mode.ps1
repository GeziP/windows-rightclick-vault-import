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

function Test-IsAdmin {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdmin)) {
    Write-Error "This script must be run from an elevated Administrator PowerShell session."
}

$kbintakeDir = Join-Path $RepoRoot "kbintake"
$exe = Join-Path $kbintakeDir "target\release\kbintake.exe"
$appDataDir = Join-Path $env:TEMP "kbintake-service-check"
$sampleDir = Join-Path $env:TEMP "kbintake-service-files"
$sampleFile = Join-Path $sampleDir "svc-note.md"
$vaultFile = Join-Path $appDataDir "vault\svc-note.md"
$logDir = Join-Path $appDataDir "logs"

$results = [ordered]@{
    Install = $false
    Start = $false
    AutoProcess = $false
    LogExists = $false
    Stop = $false
    Uninstall = $false
}

Invoke-Step "Build release binaries" {
    Push-Location $kbintakeDir
    try {
        cargo build --release --locked --bins
    }
    finally {
        Pop-Location
    }
}

Invoke-Step "Reset isolated service app-data directory" {
    Remove-Item $appDataDir -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item $sampleDir -Recurse -Force -ErrorAction SilentlyContinue
    $env:KBINTAKE_APP_DATA_DIR = $appDataDir
    & $exe doctor --fix
}

Invoke-Step "Remove any previous KBIntake service" {
    sc.exe stop KBIntake | Out-Null
    sc.exe delete KBIntake | Out-Null
    Start-Sleep -Seconds 2
}

Invoke-Step "Install service" {
    & $exe service install
    $results.Install = $true
    & $exe service status
    sc.exe query KBIntake
}

Invoke-Step "Create queued import item" {
    New-Item -ItemType Directory -Force $sampleDir | Out-Null
    Set-Content $sampleFile "service smoke"
    & $exe import $sampleFile
    & $exe jobs list
}

Invoke-Step "Start service" {
    & $exe service start
    $results.Start = $true
    Start-Sleep -Seconds 8
    & $exe service status
    sc.exe query KBIntake
}

Invoke-Step "Verify queued item processed" {
    & $exe jobs list
    if (Test-Path $vaultFile) {
        $results.AutoProcess = $true
    }
    else {
        throw "Expected imported file was not found at $vaultFile"
    }
}

Invoke-Step "Verify service log exists" {
    $logFile = Get-ChildItem $logDir -Filter "service.log*" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    if ($null -ne $logFile) {
        $results.LogExists = $true
        Get-Content $logFile.FullName -Tail 20
    }
    else {
        throw "Expected a service.log* file under $logDir"
    }
}

Invoke-Step "Stop service" {
    & $exe service stop
    $results.Stop = $true
    & $exe service status
}

Invoke-Step "Uninstall service" {
    & $exe service uninstall
    $results.Uninstall = $true
    & $exe service status
    $queryOutput = sc.exe query KBIntake 2>&1
    if ($LASTEXITCODE -eq 1060) {
        Write-Host "[OK] Service no longer installed"
    }
    else {
        $queryOutput
    }
}

Write-Host ""
Write-Host "Summary" -ForegroundColor Green
Write-Host "#46"
Write-Host "- service install: $($results.Install)"
Write-Host "- service start: $($results.Start)"
Write-Host "- service auto-process queued item: $($results.AutoProcess)"
Write-Host "- service.log exists: $($results.LogExists)"
Write-Host "- service stop: $($results.Stop)"
Write-Host "- service uninstall: $($results.Uninstall)"
Write-Host "- App data dir: $appDataDir"
Write-Host "- Sample file: $sampleFile"
