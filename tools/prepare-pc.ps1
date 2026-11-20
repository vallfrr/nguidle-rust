param(
    [Parameter(Mandatory=$true)][string]$GameData,
    [string]$Python = 'python',
    [string]$Output,
    [string]$TargetDir,
    [switch]$ExtractOnly
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$game = (Resolve-Path -LiteralPath $GameData).Path
$out = [IO.Path]::GetFullPath($(if ($Output) { $Output } else { Join-Path $repo 'out/pc' }))
$target = [IO.Path]::GetFullPath($(if ($TargetDir) { $TargetDir } else { Join-Path $repo 'build/pc' }))
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
if ($ExtractOnly) { return }

$env:CARGO_TARGET_DIR = $target
$env:RUST_MIN_STACK = '536870912'
$env:CARGO_BUILD_JOBS = '1'
$dll = Join-Path $game 'Managed/Assembly-CSharp.dll'
Push-Location -LiteralPath $repo
try {
    cargo build --locked --release --package ngu-ilgen
    if ($LASTEXITCODE -ne 0) { throw 'The converter build failed.' }
    & (Join-Path $target 'release/ngu-ilgen.exe') $dll (Join-Path $repo 'crates/ngu-gen/src') ALL NumberOutput::suffixFormat
    if ($LASTEXITCODE -ne 0) { throw 'The game conversion failed.' }
    & (Join-Path $PSScriptRoot 'capbuild.ps1') -GB 14 -Cmd @('cargo','build','--locked','--profile','game','--package','ngu-gen')
    if ($LASTEXITCODE -ne 0) { throw 'The game build failed.' }
    Write-Output ("& '{0}' window '{1}' '{2}'" -f (Join-Path $target 'game/ngu-gen.exe'), $dll, $out)
} finally {
    Pop-Location
}
