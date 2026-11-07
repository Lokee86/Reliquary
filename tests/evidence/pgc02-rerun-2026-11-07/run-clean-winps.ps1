$ErrorActionPreference = 'Stop'
$evidence = $PSScriptRoot
$repo = [System.IO.Path]::GetFullPath((Join-Path $evidence '..\..\..'))
$winps = 'C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe'
$runner = Join-Path $repo 'scripts\run_g4_acceptance.ps1'
$expectedHead = '26e84f45b54985d5aa1ceb3ab8e7fd7f183b8b0a'
$planSpec = '5d96db99'
$moduleRoots = @(
    'C:\Users\archa\OneDrive\Documents\WindowsPowerShell\Modules',
    'C:\Program Files\WindowsPowerShell\Modules',
    'C:\WINDOWS\system32\WindowsPowerShell\v1.0\Modules'
)
$existingModuleRoots = @($moduleRoots | Where-Object { [System.IO.Directory]::Exists($_) })
if(-not [System.IO.Directory]::Exists($moduleRoots[-1])){throw "Required system WindowsPowerShell module root is missing."}
$processModulePath = $existingModuleRoots -join ';'
$head = (& git -C $repo rev-parse HEAD).Trim()
if($head -ne $expectedHead){throw "Refusing to run at unexpected HEAD $head (expected $expectedHead)."}
function Get-Sha256([string]$FilePath){
    $stream = [System.IO.File]::OpenRead($FilePath)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try { return ([System.BitConverter]::ToString($algorithm.ComputeHash($stream))).Replace('-','') }
    finally { $stream.Dispose(); $algorithm.Dispose() }
}
function Save-Metadata([string]$FilePath,[object]$Value){
    $json = ConvertTo-Json -InputObject $Value -Depth 24
    [System.IO.File]::WriteAllText($FilePath,$json,[System.Text.UTF8Encoding]::new($false))
}
function Invoke-Logged([string]$Name,[string]$Kind,[string]$Arguments,[string]$VersionHint){
    $stdout = Join-Path $evidence ($Name + '.stdout.log')
    $stderr = Join-Path $evidence ($Name + '.stderr.log')
    $metaPath = Join-Path $evidence ($Name + '.meta.json')
    foreach($file in @($stdout,$stderr,$metaPath)){if([System.IO.File]::Exists($file)){throw "Refusing to overwrite evidence file: $file"}}
    $record = [pscustomobject][ordered]@{
        kind = $Kind
        shell_version = $VersionHint
        resolved_executable = $winps
        wrapper_shell_version = $PSVersionTable.PSVersion.ToString()
        working_directory = $repo
        process_local_PSModulePath = $env:PSModulePath
        arguments = $Arguments
        command = '"' + $winps + '" ' + $Arguments
        launched_utc = [DateTime]::UtcNow.ToString('o')
        planning_spec_revision = $planSpec
        tested_source_commit = $head
        runner_sha256 = Get-Sha256 $runner
        focused_harness_sha256 = Get-Sha256 (Join-Path $repo 'scripts\test_g4_acceptance_native_capture.ps1')
        target_cache = Join-Path $repo 'target-codex-check'
        stdout_log = [System.IO.Path]::GetFileName($stdout)
        stderr_log = [System.IO.Path]::GetFileName($stderr)
        exit_code = $null
        completed_utc = $null
        stdout_sha256 = $null
        stderr_sha256 = $null
    }
    Save-Metadata $metaPath $record
    $process = Start-Process -FilePath $winps -ArgumentList $Arguments -RedirectStandardOutput $stdout -RedirectStandardError $stderr -WorkingDirectory $repo -PassThru -Wait -NoNewWindow
    $record.exit_code = $process.ExitCode
    $record.completed_utc = [DateTime]::UtcNow.ToString('o')
    $record.stdout_sha256 = Get-Sha256 $stdout
    $record.stderr_sha256 = Get-Sha256 $stderr
    Save-Metadata $metaPath $record
    return $record
}
$oldModulePath = $env:PSModulePath
try {
    $env:PSModulePath = $processModulePath
    $probeCode = '$PSVersionTable.PSVersion.ToString(); (Get-Command Get-FileHash -ErrorAction Stop).Name; (Get-Command Get-FileHash -ErrorAction Stop).Source; (Get-Module Microsoft.PowerShell.Utility | Select-Object -First 1 -ExpandProperty Version).ToString(); (Get-Module Microsoft.PowerShell.Utility | Select-Object -First 1 -ExpandProperty Path)'
    $probeArgs = '-NoLogo -NoProfile -Command "' + $probeCode + '"'
    $preflight = Invoke-Logged 'clean-winps-preflight' 'module_preflight' $probeArgs '5.1.26100.9711'
    $probeOutput = [System.IO.File]::ReadAllText((Join-Path $evidence 'clean-winps-preflight.stdout.log'))
    $expectedUtilityPath = 'C:\WINDOWS\system32\WindowsPowerShell\v1.0\Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1'
    if($preflight.exit_code -ne 0 -or $probeOutput -notmatch '5\.1\.26100\.9711' -or $probeOutput -notmatch 'Get-FileHash' -or $probeOutput -notmatch '3\.1\.0\.0' -or $probeOutput -notmatch [regex]::Escape($expectedUtilityPath)){
        throw "PS5 module preflight failed; see clean-winps-preflight stdout/stderr logs."
    }
    $runnerArgs = '-NoLogo -NoProfile -File "' + $runner + '"'
    $full = Invoke-Logged 'clean-winps-full' 'full_acceptance' $runnerArgs '5.1.26100.9711'
    $full | Add-Member -NotePropertyName prior_unfiltered_attempt -NotePropertyValue ([pscustomobject]@{
        metadata = 'full-winps.meta.json'
        exit_code = 1
        exact_error = 'Get-FileHash was not recognized at scripts/run_g4_acceptance.ps1:153.'
        diagnosed_environment = 'PS5.1 loaded Microsoft.PowerShell.Utility 7.0.0.0 from the Codex PowerShell 7 module path; that module did not export Get-FileHash. The normal WindowsPowerShell Utility 3.1.0.0 module was later in PSModulePath.'
        change_for_this_attempt = 'Only this wrapper process local PSModulePath is restricted to the WindowsPowerShell module roots; no system or user setting is changed.'
    })
    $fullOutput = [System.IO.File]::ReadAllText((Join-Path $evidence 'clean-winps-full.stdout.log'))
    $completion = [regex]::Match($fullOutput,'(?m)^Completed corpus evidence: (.+)$')
    $evidenceChecks = [ordered]@{runner_exit_zero=($full.exit_code -eq 0);completion_path_found=$completion.Success;cli_observation_count=$null;negative_cli_count=$null;negative_structured_json_error_count=$null;source_bytes_unchanged=$false;source_file_hashes=@();repeat_semantics_equal=$false;artifacts=@()}
    if($completion.Success){
        $primary = $completion.Groups[1].Value.Trim()
        $repeat = $primary + '-repeat'
        $witnessPath = Join-Path $primary 'witness.json'
        $processWitnessPath = Join-Path $primary 'process-witness.json'
        $cliWitnessPath = Join-Path $primary 'cli-witness.json'
        $repeatWitnessPath = Join-Path $repeat 'witness.json'
        if(@($witnessPath,$processWitnessPath,$cliWitnessPath,$repeatWitnessPath | Where-Object { -not [System.IO.File]::Exists($_) }).Count -eq 0){
            $witness = [System.IO.File]::ReadAllText($witnessPath) | ConvertFrom-Json
            $processWitness = [System.IO.File]::ReadAllText($processWitnessPath) | ConvertFrom-Json
            $observations = @([System.IO.File]::ReadAllText($cliWitnessPath) | ConvertFrom-Json)
            $repeatWitness = [System.IO.File]::ReadAllText($repeatWitnessPath) | ConvertFrom-Json
            $negative = @($observations | Where-Object { $_.exit_code -ne 0 })
            $structured = @($negative | Where-Object { $null -ne $_.result.error })
            $evidenceChecks.cli_observation_count = $observations.Count
            $evidenceChecks.negative_cli_count = $negative.Count
            $evidenceChecks.negative_structured_json_error_count = $structured.Count
            $evidenceChecks.source_bytes_unchanged = [bool]$processWitness.source_bytes_unchanged
            $evidenceChecks.repeat_semantics_equal = ((ConvertTo-Json -InputObject $witness.semantics -Depth 64 -Compress) -ceq (ConvertTo-Json -InputObject $repeatWitness.semantics -Depth 64 -Compress))
            $sourceResults = @($witness.owners | ForEach-Object {
                $owner = $_
                $file = Join-Path $primary $owner.file
                $actual = Get-Sha256 $file
                [pscustomobject]@{path=$file;expected_sha256=$owner.sha256;actual_sha256=$actual;matches=($actual -ieq $owner.sha256)}
            })
            $evidenceChecks.source_file_hashes = $sourceResults
            $artifactPaths = @($witnessPath,$processWitnessPath,$cliWitnessPath,$repeatWitnessPath)
            $evidenceChecks.artifacts = @($artifactPaths | ForEach-Object { [pscustomobject]@{path=$_;sha256=(Get-Sha256 $_)} })
        }
    }
    $full | Add-Member -NotePropertyName evidence_checks -NotePropertyValue ([pscustomobject]$evidenceChecks)
    $full | Add-Member -NotePropertyName accepted -NotePropertyValue ([bool]($evidenceChecks.runner_exit_zero -and $evidenceChecks.completion_path_found -and $evidenceChecks.cli_observation_count -eq 25 -and $evidenceChecks.negative_cli_count -eq 8 -and $evidenceChecks.negative_structured_json_error_count -eq 8 -and $evidenceChecks.source_bytes_unchanged -and $evidenceChecks.repeat_semantics_equal -and @($evidenceChecks.source_file_hashes | Where-Object { -not $_.matches }).Count -eq 0))
    Save-Metadata (Join-Path $evidence 'clean-winps-full.meta.json') $full
    Write-Output (ConvertTo-Json -InputObject ([pscustomobject]@{preflight_exit=$preflight.exit_code;full_runner_exit=$full.exit_code;accepted=$full.accepted;metadata=(Join-Path $evidence 'clean-winps-full.meta.json');stdout=(Join-Path $evidence 'clean-winps-full.stdout.log');stderr=(Join-Path $evidence 'clean-winps-full.stderr.log');process_local_PSModulePath=$processModulePath}) -Depth 24)
    if(-not $full.accepted){exit 2}
}
finally {
    $env:PSModulePath = $oldModulePath
}
