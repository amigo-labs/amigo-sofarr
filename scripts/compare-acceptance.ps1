#Requires -Version 7
<#
.SYNOPSIS
  Runs the four comparisons against the owner's Radarr and Sonarr that the manual
  acceptance criteria need: mediainfo AC-07 (compare-mediainfo) and web-ui-library AC-23
  (compare-rename), each for movies (Radarr) and series (Sonarr).

.DESCRIPTION
  Without a sofarr configuration (config/sofarr.toml or -Config) the script prepares one:
  it takes the [radarr] and [sonarr] blocks of fixtures/private/parity.toml, writes
  target/acceptance/sofarr.toml (shadow mode, its own database next to it; target/ is
  gitignored) and imports the library fresh from the running Radarr and Sonarr.

  Every run writes its full output to a log file under target/acceptance/<time>/ and the
  script ends with a summary. A comparison that finds differences exits non-zero; that is
  expected and not an error of the script. The script never prints API keys.

.EXAMPLE
  pwsh scripts/compare-acceptance.ps1
  pwsh scripts/compare-acceptance.ps1 -MoviesRoot 'Z:\Filme' -SeriesRoot 'Z:\Serien' -Limit 5000
#>
param(
    # The sofarr configuration with [radarr], [sonarr] and the database; empty: the
    # project's config/sofarr.toml, else one prepared from fixtures/private/parity.toml.
    [string] $Config = '',
    # Where the [radarr] and [sonarr] blocks come from when there is no configuration.
    [string] $Parity = 'fixtures/private/parity.toml',
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

if (-not (Test-Path $Binary)) {
    Write-Host 'amigo-sofarr wird gebaut …'
    cargo build -p sofarr-server --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build fehlgeschlagen' }
}

# The `key = "value"` lines of one [section] of a TOML file (no nested tables needed).
function Read-TomlSection([string] $Path, [string] $Section) {
    $values = @{}
    $inside = $false
    foreach ($line in Get-Content -Encoding utf8 $Path) {
        $text = $line.Trim()
        if ($text -match '^\[+([^\]]+)\]+$') { $inside = ($Matches[1] -eq $Section); continue }
        if ($inside -and $text -match '^([A-Za-z_]+)\s*=\s*["''](.*)["'']\s*(#.*)?$') {
            $values[$Matches[1]] = $Matches[2]
        }
    }
    $values
}

# A TOML literal string (single quotes keep Windows backslashes as they are).
function ConvertTo-TomlLiteral([string] $Value) {
    if ($Value.Contains("'")) { throw "Wert mit einfachem Anführungszeichen wird nicht unterstützt" }
    "'$Value'"
}

$acceptance = 'target/acceptance'
New-Item -ItemType Directory -Force -Path $acceptance | Out-Null
$prepared = $false
if ($Config -eq '') {
    if (Test-Path 'config/sofarr.toml') {
        $Config = 'config/sofarr.toml'
    } else {
        if (-not (Test-Path $Parity)) {
            throw "Weder config/sofarr.toml noch $Parity gefunden – bitte -Config angeben"
        }
        $lines = @(
            '# Prepared by scripts/compare-acceptance.ps1 for the manual comparisons only.',
            '[automation]', 'mode = "shadow"',
            '[database]', "path = $(ConvertTo-TomlLiteral ((Join-Path (Resolve-Path $acceptance) 'sofarr.db') -replace '\\', '/'))",
            '[imdb]', 'enabled = false',
            '[trailers]', 'enabled = false'
        )
        foreach ($app in @('radarr', 'sonarr')) {
            $block = Read-TomlSection $Parity $app
            if (-not $block.ContainsKey('url') -or -not $block.ContainsKey('api_key')) {
                throw "[$app] mit url und api_key fehlt in $Parity"
            }
            $lines += "[$app]", "url = $(ConvertTo-TomlLiteral $block['url'])", "api_key = $(ConvertTo-TomlLiteral $block['api_key'])"
        }
        $Config = Join-Path $acceptance 'sofarr.toml'
        Set-Content -Encoding utf8NoBOM -Path $Config -Value ($lines -join "`n")
        Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $acceptance 'sofarr.db*')
        $prepared = $true
        Write-Host "Konfiguration aus $Parity vorbereitet: $Config (Radarr und Sonarr aus parity.toml)"
    }
} elseif (-not (Test-Path $Config)) {
    throw "Konfiguration nicht gefunden: $Config"
}
foreach ($root in @($MoviesRoot, $SeriesRoot)) {
    if (-not (Test-Path $root)) {
        Write-Warning "Ordner nicht erreichbar: $root (compare-mediainfo meldet dann „Datei fehlt“)"
    }
}

$out = Join-Path $acceptance (Get-Date -Format 'yyyy-MM-dd_HH-mm-ss')
New-Item -ItemType Directory -Force -Path $out | Out-Null

if ($prepared) {
    foreach ($import in @('import-radarr', 'import-sonarr')) {
        $log = Join-Path $out "$import.log"
        Write-Host ''
        Write-Host "=== $import (Bibliothek aus der laufenden Instanz) ==="
        & $Binary --config $Config $import 2>&1 | ForEach-Object { "$_" } | Tee-Object -FilePath $log
        if ($LASTEXITCODE -ne 0) { throw "$import fehlgeschlagen (Exitcode $LASTEXITCODE), siehe $log" }
    }
}

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
