param(
    [Parameter(Mandatory = $true)][string]$GameCopy,
    [Parameter(Mandatory = $true)][string]$BepInEx
)
$ErrorActionPreference = 'Stop'
$copy = (Resolve-Path -LiteralPath $GameCopy).Path
$bep = (Resolve-Path -LiteralPath $BepInEx).Path
$managed = Join-Path $copy 'NGUIdle_Data/Managed'
$core = Join-Path $bep 'BepInEx/core'
$compiler = Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319/csc.exe'
$plugins = Join-Path $copy 'BepInEx/plugins'
$patchers = Join-Path $copy 'BepInEx/patchers'
New-Item -ItemType Directory -Force $plugins, $patchers | Out-Null

function Build-Probe([string]$Source, [string]$Output, [string[]]$References) {
    $arguments = @('/nologo', '/noconfig', '/nostdlib', '/target:library', "/out:$Output")
    $arguments += $References | ForEach-Object { "/reference:$_" }
    $arguments += (Join-Path $PSScriptRoot $Source)
    & $compiler @arguments
    if ($LASTEXITCODE -ne 0) { throw "Failed to build $Source" }
}

$system = @((Join-Path $managed 'mscorlib.dll'), (Join-Path $managed 'System.dll'))
$unity = $system + @(
    (Join-Path $managed 'UnityEngine.dll'),
    (Join-Path $managed 'UnityEngine.CoreModule.dll'),
    (Join-Path $core 'BepInEx.dll')
)
Build-Probe 'UnityRngProbe.cs' (Join-Path $plugins 'UnityRngProbe.dll') $unity
Build-Probe 'ProgressProbe.cs' (Join-Path $plugins 'ProgressProbe.dll') ($unity + (Join-Path $core '0Harmony.dll'))
Build-Probe 'IsolatedProbePatcher.cs' (Join-Path $patchers 'IsolatedProbePatcher.dll') ($system + (Join-Path $core 'Mono.Cecil.dll'))
Write-Output 'Probes built in the supplied isolated game copy.'
