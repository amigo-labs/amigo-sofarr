#Requires -Version 7
<#
.SYNOPSIS
  Runs the four comparisons against the owner's Radarr and Sonarr that the manual
  acceptance criteria need: mediainfo AC-07 (compare-mediainfo) and web-ui-library AC-23
  (compare-rename), each for movies (Radarr) and series (Sonarr).

.DESCRIPTION
  Every run writes its full output to a log file under target/acceptance/<time>/ and the
  script ends with a summary. A comparison that finds differences exits non-zero; that is
  expected and not an error of the script. Credentials are read by amigo-sofarr from the
  config file only; this script never reads or prints them.

.EXAMPLE
  pwsh scripts/compare-acceptance.ps1
  pwsh scripts/compare-acceptance.ps1 -MoviesRoot 'Z:\Filme' -SeriesRoot 'Z:\Serien' -Limit 5000
#>
param(
    # The sofarr configuration with [radarr], [sonarr] and the database.
    [string] $Config = 'config/sofarr.toml',
    # Radarr's root folder as this machine sees it (compare-mediainfo reads the files).
    [string] $MoviesRoot = '\\wdnas\media\Filme',
    # Sonarr's root folder as this machine sees it.
    [string] $SeriesRoot = '\\wdnas\media\Serien',
    # At most this many movies or series per compare-rename run.
    [int] $Limit = 200,
    # The amigo-sofarr binary; built with cargo when it is missing.
    [string] $Binary = 'target/debug/amigo-sofarr.exe'
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
Set-Location (Join-Path $PSScriptRoot '..')

if (-not (Test-Path $Config)) {
    throw "Konfiguration nicht gefunden: $Config"
}
if (-not (Test-Path $Binary)) {
    Write-Host 'amigo-sofarr wird gebaut …'
    cargo build -p sofarr-server --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build fehlgeschlagen' }
}
foreach ($root in @($MoviesRoot, $SeriesRoot)) {
    if (-not (Test-Path $root)) {
        Write-Warning "Ordner nicht erreichbar: $root (compare-mediainfo meldet dann „Datei fehlt“)"
    }
}

$out = Join-Path 'target/acceptance' (Get-Date -Format 'yyyy-MM-dd_HH-mm-ss')
New-Item -ItemType Directory -Force -Path $out | Out-Null

$runs = @(
    @{ Name = 'mediainfo-radarr'; Spec = 'mediainfo AC-07';
       Args = @('compare-mediainfo', '--app', 'radarr', '--root', $MoviesRoot) },
    @{ Name = 'mediainfo-sonarr'; Spec = 'mediainfo AC-07';
       Args = @('compare-mediainfo', '--app', 'sonarr', '--root', $SeriesRoot) },
    @{ Name = 'rename-radarr'; Spec = 'web-ui-library AC-23';
       Args = @('compare-rename', '--app', 'radarr', '--limit', "$Limit") },
    @{ Name = 'rename-sonarr'; Spec = 'web-ui-library AC-23';
       Args = @('compare-rename', '--app', 'sonarr', '--limit', "$Limit") }
)

$results = foreach ($run in $runs) {
    $log = Join-Path $out "$($run.Name).log"
    Write-Host ''
    Write-Host "=== $($run.Name) ($($run.Spec)) ==="
    $started = Get-Date
    & $Binary --config $Config @($run.Args) 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log
    $code = $LASTEXITCODE
    [pscustomobject]@{
        Lauf     = $run.Name
        Spec     = $run.Spec
        Exitcode = $code
        Ergebnis = if ($code -eq 0) { 'gleich' } else { 'Abweichungen oder Fehler' }
        Dauer    = '{0:N0} s' -f ((Get-Date) - $started).TotalSeconds
        Log      = $log
    }
}

Write-Host ''
Write-Host '=== Zusammenfassung ==='
$results | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
$results | ConvertTo-Json | Set-Content -Encoding utf8NoBOM -Path (Join-Path $out 'summary.json')
Write-Host "Logs: $out"
