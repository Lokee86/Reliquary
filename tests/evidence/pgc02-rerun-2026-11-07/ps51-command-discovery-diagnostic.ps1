$ErrorActionPreference = 'Continue'
$repo = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$runnerPath = Join-Path $repo 'scripts\run_g4_acceptance.ps1'
$tokens = $null; $parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($runnerPath, [ref]$tokens, [ref]$parseErrors)
$names = @('ConvertTo-NativeArgumentString','Invoke-NativeCommand','Get-NativeJsonText','Invoke-Json','Expect-Failure')
$defs = $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $names -contains $node.Name }, $true)
$defs | Sort-Object { $_.Extent.StartOffset } | ForEach-Object { Invoke-Expression $_.Extent.Text }
function Snapshot([string]$Label) {
    $autoload = Get-Variable -Name PSModuleAutoLoadingPreference -ErrorAction SilentlyContinue
    $autoloadText = if ($null -eq $autoload) { '<unset>' } else { [string]$autoload.Value }
    Write-Output ("SNAPSHOT=" + $Label)
    Write-Output ("PSVersion=" + $PSVersionTable.PSVersion.ToString())
    Write-Output ("PSModulePath=" + $env:PSModulePath)
    Write-Output ("PSModuleAutoLoadingPreference=" + $autoloadText)
    $available = @(Get-Module -ListAvailable -Name Microsoft.PowerShell.Utility)
    Write-Output ("UtilityAvailableCount=" + $available.Count)
    foreach($m in $available){Write-Output ("UTILITY_AVAILABLE="+$m.Version+";PATH="+$m.Path)}
    $loaded = @(Get-Module -Name Microsoft.PowerShell.Utility)
    foreach($m in $loaded){Write-Output ("UTILITY_LOADED="+$m.Version+";PATH="+$m.Path+";EXPORTS_FILEHASH="+($m.ExportedCommands.ContainsKey('Get-FileHash')))}
    $discovered = @(Get-Command -Name Get-FileHash -ListImported -ErrorAction SilentlyContinue)
    Write-Output ("GET_FILE_HASH_IMPORTED_COUNT=" + $discovered.Count)
}
$primary = Join-Path (Join-Path $repo '.test-tmp\g4-completed-56596a5f52ee42c5baf4b2c7fa103002') 'rel-lantern.rel'
Snapshot 'before_helper'
try { $h=(Get-FileHash -LiteralPath $primary -Algorithm SHA256).Hash; Write-Output ("HASH_BEFORE_HELPER=PASS;"+$h) } catch { Write-Output ("HASH_BEFORE_HELPER=FAIL;"+$_.Exception.Message) }
Snapshot 'after_pre_hash'
$child = (Get-Command powershell.exe -CommandType Application | Select-Object -First 1).Path
$probe = Invoke-NativeCommand $child @('-NoLogo','-NoProfile','-Command','[Console]::Out.WriteLine("probe-out"); [Console]::Error.WriteLine("probe-err"); exit 17')
Write-Output ("PROCESS_HELPER_EXIT=" + $probe.ExitCode + ";OUT_LENGTH=" + $probe.Stdout.Length + ";ERR_LENGTH=" + $probe.Stderr.Length)
Snapshot 'after_helper'
try { $h=(Get-FileHash -LiteralPath $primary -Algorithm SHA256).Hash; Write-Output ("HASH_AFTER_HELPER=PASS;"+$h) } catch { Write-Output ("HASH_AFTER_HELPER=FAIL;"+$_.Exception.Message) }
Snapshot 'after_post_hash'