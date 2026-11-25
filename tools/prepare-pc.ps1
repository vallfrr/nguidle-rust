param(
    [string]$GameData,
    [string]$Python,
    [string]$Output,
    [string]$TargetDir,
    [switch]$ExtractOnly,
    [switch]$SkipMilestoneIcons
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'environment.ps1')
$paths = Get-NguPaths
$repo = $paths.Repo
if (!$GameData) { $GameData = $paths.Game }
if (!$Python) { $Python = if (Test-Path -LiteralPath $paths.PrepPython) { $paths.PrepPython } else { 'python' } }
$game = (Resolve-Path -LiteralPath $GameData).Path
$out = [IO.Path]::GetFullPath($(if ($Output) { $Output } else { Join-Path $paths.Work 'prepared/pc' }))
$target = [IO.Path]::GetFullPath($(if ($TargetDir) { $TargetDir } else { Join-Path $paths.Build 'win' }))
$gamePrefix = $game.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
foreach ($destination in $out, $target) {
    if ($destination -eq $game -or $destination.StartsWith($gamePrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Choose output and build folders outside the original game data.'
    }
}

& $Python (Join-Path $PSScriptRoot 'verify_game_data.py') $game
if ($LASTEXITCODE -ne 0) { throw 'Game data verification failed.' }
& $Python (Join-Path $PSScriptRoot 'nguprep.py') --game-data $game --out $out
if ($LASTEXITCODE -ne 0) { throw 'Game data extraction failed.' }
if (!$SkipMilestoneIcons) {
    & $Python (Join-Path $PSScriptRoot 'fetch-achievement-icons.py') $out
    if ($LASTEXITCODE -ne 0) { Write-Warning 'Steam achievement icons are temporarily unavailable. The game will show text in their place.' }
}
if ($ExtractOnly) { return }

$env:CARGO_TARGET_DIR = $target
$dll = Join-Path $game 'Managed/Assembly-CSharp.dll'
Push-Location -LiteralPath $repo
try {
    cargo build --locked --release --package ngu-desktop --bin ngu-window
    if ($LASTEXITCODE -ne 0) { throw 'The desktop engine build failed.' }
    Write-Output ("& '{0}' '{1}' '{2}'" -f (Join-Path $target 'release/ngu-window.exe'), $dll, $out)
} finally {
    Pop-Location
}
