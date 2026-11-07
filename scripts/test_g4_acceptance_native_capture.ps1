#requires -Version 5.1
# Focused tests load only the native-capture helper functions from the acceptance runner.
# This script does not invoke Cargo or the full G4 acceptance flow.

$ErrorActionPreference = 'Stop'
$runnerPath = Join-Path $PSScriptRoot 'run_g4_acceptance.ps1'
$runnerSource = [System.IO.File]::ReadAllText($runnerPath)
$tokens = $null
$parseErrors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($runnerPath, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -gt 0) {
    throw (($parseErrors | ForEach-Object { $_.Message }) -join '; ')
}

$negativeCases = [System.Text.RegularExpressions.Regex]::Matches(
    $runnerSource,
    '(?m)^\s*Expect-Failure \$cli\b'
).Count
if ($negativeCases -ne 8) {
    throw "Expected 8 negative CLI invocations, found $negativeCases."
}

$functionNames = @(
    'ConvertTo-NativeArgumentString',
    'Invoke-NativeCommand',
    'Get-NativeJsonText',
    'Invoke-Json',
    'Expect-Failure'
)
$functionAsts = $ast.FindAll({
    param($node)
    $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and
        $functionNames -contains $node.Name
}, $true)
if ($functionAsts.Count -ne $functionNames.Count) {
    throw "Expected all native-capture helper definitions, found $($functionAsts.Count)."
}
$functionAsts |
    Sort-Object { $_.Extent.StartOffset } |
    ForEach-Object { Invoke-Expression $_.Extent.Text }

$observations = [System.Collections.Generic.List[object]]::new()
$nativeShell = if ($PSVersionTable.PSVersion.Major -lt 6) {
    (Get-Command powershell.exe -CommandType Application | Select-Object -First 1).Path
}
else {
    (Get-Command pwsh.exe -CommandType Application | Select-Object -First 1).Path
}
if (-not $nativeShell) { throw 'Could not resolve a child PowerShell executable.' }

$probePath = Join-Path ([System.IO.Path]::GetTempPath()) ("pgc02-argv-" + [guid]::NewGuid().ToString('N') + '.ps1')
if (Test-Path -LiteralPath $probePath) { throw "Refusing to overwrite existing probe: $probePath" }

try {
    $stdoutJson = '[Console]::Out.WriteLine(''{"ok":true}'')'
    $success = Invoke-Json $nativeShell @('-NoProfile', '-Command', $stdoutJson)
    if (-not $success.ok -or $observations[0].exit_code -ne 0) {
        throw 'stdout-only success JSON or zero exit was not preserved.'
    }

    $stderrJson = '[Console]::Error.WriteLine(''{"error":{"message":"probe"}}''); exit 23'
    Expect-Failure $nativeShell @('-NoProfile', '-Command', $stderrJson)
    if ($observations.Count -ne 2 -or
        $observations[1].exit_code -ne 23 -or
        $observations[1].result.error.message -ne 'probe') {
        throw 'stderr-only JSON, nonzero exit, or observation capture failed.'
    }

    $stdoutErrorJson = '[Console]::Out.WriteLine(''{"error":{"message":"probe"}}''); exit 25'
    Expect-Failure $nativeShell @('-NoProfile', '-Command', $stdoutErrorJson)
    if ($observations.Count -ne 3 -or
        $observations[2].exit_code -ne 25 -or
        $observations[2].result.error.message -ne 'probe') {
        throw 'stdout-only failure JSON or exit-code capture failed.'
    }

    $mixedOutput = '[Console]::Out.WriteLine(''{"error":{"message":"probe"}}''); [Console]::Error.WriteLine(''not json''); exit 24'
    $mixedOutputRejected = $false
    try {
        Expect-Failure $nativeShell @('-NoProfile', '-Command', $mixedOutput)
    }
    catch {
        if ($_.Exception.Message -match 'both stdout and stderr') { $mixedOutputRejected = $true }
        else { throw }
    }
    if (-not $mixedOutputRejected) { throw 'Mixed stdout JSON and stderr diagnostics were not rejected.' }
    if ($observations.Count -ne 3) { throw 'Mixed output unexpectedly added an observation.' }

    $nonzeroSuccess = '[Console]::Out.WriteLine(''{"ok":true}''); exit 26'
    $nonzeroSuccessRejected = $false
    try {
        $null = Invoke-Json $nativeShell @('-NoProfile', '-Command', $nonzeroSuccess)
    }
    catch {
        if ($_.Exception.Message -match 'failed \(26\)') { $nonzeroSuccessRejected = $true }
        else { throw }
    }
    if (-not $nonzeroSuccessRejected) { throw 'Invoke-Json accepted a nonzero native exit.' }
    if ($observations.Count -ne 3) { throw 'A nonzero success command unexpectedly added an observation.' }

    $probeSource = '[Console]::Out.WriteLine((ConvertTo-Json -InputObject $args -Compress))'
    [System.IO.File]::WriteAllText(
        $probePath,
        $probeSource + [Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    $expectedArgs = @('arg with spaces', '', 'embedded"quote', 'C:\tail path\')
    $processArgs = @('-NoProfile', '-File', $probePath) + $expectedArgs
    $quoted = Invoke-NativeCommand $nativeShell $processArgs
    if ($quoted.ExitCode -ne 0) {
        throw "Argument echo child failed with $($quoted.ExitCode): $($quoted.Stderr)"
    }
    $actualArgs = $quoted.Stdout.Trim() | ConvertFrom-Json
    if ($actualArgs.Count -ne $expectedArgs.Count) {
        throw "Argument count mismatch: got $($actualArgs.Count)."
    }
    for ($i = 0; $i -lt $expectedArgs.Count; $i++) {
        if ($actualArgs[$i] -cne $expectedArgs[$i]) {
            throw "Argument $i was not preserved: '$($actualArgs[$i])'."
        }
    }

    $largeOutput = '[Console]::Out.Write(''x''.PadRight(200000, ''x'')); [Console]::Error.Write(''y''.PadRight(200000, ''y'')); exit 17'
    $large = Invoke-NativeCommand $nativeShell @('-NoProfile', '-Command', $largeOutput)
    if ($large.ExitCode -ne 17 -or
        $large.Stdout.Length -ne 200000 -or
        $large.Stderr.Length -ne 200000) {
        throw "Concurrent stdout/stderr capture or exit preservation failed (exit=$($large.ExitCode), stdout=$($large.Stdout.Length), stderr=$($large.Stderr.Length))."
    }

    Write-Output "PASS: 8 negative-call preservation, JSON/exit validation, mixed-stream rejection, Windows argv quoting, and 200KB dual-stream capture (PowerShell $($PSVersionTable.PSVersion))."
}
finally {
    if (Test-Path -LiteralPath $probePath) { [System.IO.File]::Delete($probePath) }
}