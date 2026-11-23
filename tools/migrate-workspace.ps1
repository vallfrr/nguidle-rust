# Move one Windows development workspace. Dry run by default.
param([Parameter(Mandatory=$true)][string]$Workspace, [switch]$Apply)
$ErrorActionPreference = 'Stop'
$root = (Get-Item -LiteralPath $Workspace).FullName.TrimEnd('\')
$prefix = $root + '\'
$relocations = @(
    @('nguvita/nguidle-web', 'repos/nguidle-web'),
    @('nguvita/nguidle-vita', 'repos/nguidle-vita'),
    @('NGU IDLE', 'game/NGU IDLE'),
    @('build-ngu', 'build/wsl'),
    @('build-ngu-win', 'build/win'),
    @('nguvita/.venv', 'venvs/prep-win'),
    @('nguvita/out/achievement-qa/prep-venv', 'venvs/achievement-qa'),
    @('nguvita/out/steam-tab-qa/linux-prep-deps', 'venvs/linux-prep-deps'),
    @('nguvita/out/steam-tab-qa/web-test-deps', 'venvs/web-test-deps'),
    @('nguvita/out/ngu', 'work/prepared/ngu'),
    @('nguvita/out/pc', 'work/prepared/pc'),
    @('nguvita/re', 'work/decomp'),
    @('nguvita/local/save-backup', 'work/saves/imported-backup'),
    @('nguvita/local/docs', 'work/notes/legacy'),
    @('nguvita/local/jmods', 'refs/jmods'),
    @('nguvita/local/skills-src', 'refs/skills-src'),
    @('nguvita/local', 'work/notes/legacy-local'),
    @('rea-main', 'refs/rea-main'),
    @('rust-anything-main', 'refs/rust-anything-main'),
    @('Captures-16x10-16x9', 'work/captures/16x10-16x9'),
    @('nguvita', 'repos/nguidle-rust')
)

function Get-WorkspacePath([string]$Relative) {
    $path = [IO.Path]::GetFullPath((Join-Path $root $Relative))
    if (!$path.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Outside workspace: $Relative"
    }
    return $path
}

function Assert-WorkspaceParents([string]$Path) {
    $parent = Split-Path -Parent $Path
    while ($parent.Length -gt $root.Length) {
        $item = Get-Item -Force -LiteralPath $parent -ErrorAction SilentlyContinue
        if ($item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            if (!$expectedAliases.ContainsKey($parent) -or $item.LinkType -ne 'Junction' -or
                $item.Target -ne $expectedAliases[$parent]) {
                throw "Unexpected linked parent: $parent"
            }
        }
        $parent = Split-Path -Parent $parent
    }
}

$expectedAliases = @{}
foreach ($entry in $relocations) {
    $expectedAliases[(Get-WorkspacePath $entry[0])] = Get-WorkspacePath $entry[1]
}
$expectedAliases[(Get-WorkspacePath 'repos/nguidle-rust/local')] = Get-WorkspacePath 'work/notes/legacy-local'

$pending = @()
foreach ($entry in $relocations) {
    $source = Get-WorkspacePath $entry[0]
    $target = Get-WorkspacePath $entry[1]
    if ($entry[0] -eq 'nguvita/local' -and
        (Test-Path -LiteralPath (Get-WorkspacePath 'repos/nguidle-rust/.git/HEAD')) -and
        !(Test-Path -LiteralPath $source)) {
        $source = Get-WorkspacePath 'repos/nguidle-rust/local'
    }
    Assert-WorkspaceParents $source
    Assert-WorkspaceParents $target
    $old = Get-Item -Force -LiteralPath $source -ErrorAction SilentlyContinue
    $new = Get-Item -Force -LiteralPath $target -ErrorAction SilentlyContinue
    if ($old -and $old.LinkType -eq 'Junction' -and $old.Target -eq $target -and $new) {
        continue
    }
    if ($new -and !$old -and !($new.Attributes -band [IO.FileAttributes]::ReparsePoint)) { continue }
    if ($entry[0] -eq 'nguvita' -and $new -and (Test-Path -LiteralPath "$target/.git/HEAD")) {
        $remaining = @(Get-ChildItem -Force -LiteralPath $source | Where-Object { $_.Name -notin '.git','out' })
        $outAlias = Get-Item -Force -LiteralPath "$target/out" -ErrorAction SilentlyContinue
        if (!$remaining.Count -and $outAlias.LinkType -eq 'Junction' -and $outAlias.Target -eq "$source\out") {
            'Engine migrated. Its historical output folder is kept at the legacy path.'
            continue
        }
    }
    if ($new) { throw "Existing destination: $target" }
    if (!$old) { continue }
    if (!$old.PSIsContainer -or ($old.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Expected an ordinary source directory: $source"
    }
    $pending += ,@($source, $target)
}
foreach ($entry in $pending) {
    $source, $target = $entry
    "MOVE $source -> $target"
    if (!$Apply) { continue }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
    # An atomic same-volume rename either completes or leaves the source intact.
    [IO.Directory]::Move($source, $target)
    try {
        New-Item -ItemType Junction -Path $source -Target $target | Out-Null
    } catch {
        [IO.Directory]::Move($target, $source)
        throw
    }
}
if ($Apply) {
    foreach ($dir in 'repos','game','work/notes','work/prepared','work/saves','work/traces','work/captures','work/session','work/decomp',
        'build/wsl','build/win','build/vita','build/wasm','toolchains','venvs','refs','vita-agent') {
        New-Item -ItemType Directory -Force -Path (Get-WorkspacePath $dir) | Out-Null
    }
}
"Pending moves: $($pending.Count)"
