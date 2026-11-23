function Get-NguPaths {
    $repo = Split-Path -Parent $PSScriptRoot
    $parent = Split-Path -Parent $repo
    $root = if ($env:NGUIDLE_HOME) {
        $env:NGUIDLE_HOME
    } elseif ((Split-Path -Leaf $parent) -eq 'repos' -or (Test-Path -LiteralPath (Join-Path $parent 'NGU IDLE/NGUIdle_Data'))) {
        if ((Split-Path -Leaf $parent) -eq 'repos') { Split-Path -Parent $parent } else { $parent }
    } else { $repo }
    $work = if ($env:NGU_WORK) { $env:NGU_WORK } else { Join-Path $root 'work' }
    $build = if ($env:NGU_BUILD) { $env:NGU_BUILD } else { Join-Path $root 'build' }
    $game = Join-Path $root 'game/NGU IDLE/NGUIdle_Data'
    if (!(Test-Path -LiteralPath $game)) {
        $legacy = Join-Path $root 'NGU IDLE/NGUIdle_Data'
        if (Test-Path -LiteralPath $legacy) { $game = $legacy }
    }
    if ($env:NGU_GAME_DIR) { $game = $env:NGU_GAME_DIR }
    @{
        Root = $root
        Repo = $repo
        Work = $work
        Build = $build
        Game = $game
        PrepPython = Join-Path $root 'venvs/prep-win/Scripts/python.exe'
    }
}
