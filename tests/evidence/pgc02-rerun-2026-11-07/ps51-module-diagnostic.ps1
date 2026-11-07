$ErrorActionPreference = 'Continue'
$repo = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..\..'))
$runnerPath = Join-Path $repo 'scripts\run_g4_acceptance.ps1'
$runnerSource = [System.IO.File]::ReadAllText($runnerPath)
$tokens = $null; $parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($runnerPath, [ref]$tokens, [ref]$parseErrors)
$names = @('ConvertTo-NativeArgumentString','Invoke-NativeCommand','Get-NativeJsonText','Invoke-Json','Expect-Failure')
$defs = $ast.FindAll({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $names -contains $node.Name }, $true)
$defs | Sort-Object { $_.Extent.StartOffset } | ForEach-Object { Invoke-Expression $_.Extent.Text }
function Snapshot([string]$Label) {
    $autoload = Get-Variable -Name PSModuleAutoLoadingPreference -ErrorAction SilentlyContinue
    $autoloadText = if ($null -eq $autoload) { '<unset>' } else { [string]$autoload.Value }
    $utility = @(Get-Module -Name Microsoft.PowerShell.Utility)
    $available = @(Get-Module -ListAvailable -Name Microsoft.PowerShell.Utility)
    Write-Output ("SNAPSHOT=" + $Label)
    Write-Output ("PSVersion=" + $PSVersionTable.PSVersion.ToString())
    Write-Output ("PSModulePath=" + $env:PSModulePath)
    Write-Output ("PSModuleAutoLoadingPreference=" + $autoloadText)
    Write-Output ("UtilityLoaded=" + ($utility.Count -gt 0))
    Write-Output ("UtilityAvailableCount=" + $available.Count)
}
Snapshot 'before_process_helper'
$child = (Get-Command powershell.exe -CommandType Application | Select-Object -First 1).Path
if (-not $child) { $child = (Get-Command pwsh.exe -CommandType Application | Select-Object -First 1).Path }
$probe = Invoke-NativeCommand $child @('-NoLogo','-NoProfile','-Command','[Console]::Out.WriteLine("probe-out"); [Console]::Error.WriteLine("probe-err"); exit 17')
Write-Output ("PROCESS_HELPER_EXIT=" + $probe.ExitCode + ";OUT_LENGTH=" + $probe.Stdout.Length + ";ERR_LENGTH=" + $probe.Stderr.Length)
Snapshot 'after_process_helper'
$sourceFiles = @('rel-lantern.rel','rel-cascade.rel','phy-reader-a.phy','phy-reader-b.phy') | ForEach-Object { Join-Path $repo (' .test-tmp' + $_) }
$sourceFiles = @('g4-completed-56596a5f52ee42c5baf4b2c7fa103002','g4-completed-b13db49b4b984d68b8d505838a5cbd94') | ForEach-Object { Join-Path (Join-Path $repo '.test-tmp') $_ }
foreach($dir in $sourceFiles){
    if(Test-Path -LiteralPath (Join-Path $dir 'witness.json')) {
        $witness = Get-Content -Raw -LiteralPath (Join-Path $dir 'witness.json') | ConvertFrom-Json
        $items = @($witness.owners | ForEach-Object { Join-Path $dir $_.file })
        try {
            $hashes = @($items | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash })
            Write-Output ("HASH_PIPELINE=PASS;COUNT=" + $hashes.Count + ";DIR=" + (Split-Path -Leaf $dir))
        } catch { Write-Output ("HASH_PIPELINE=FAIL;DIR=" + (Split-Path -Leaf $dir) + ";ERROR=" + $_.Exception.Message) }
    }
}
Snapshot 'after_hash_attempt'