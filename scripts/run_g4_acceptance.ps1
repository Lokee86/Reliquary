$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$runId = [guid]::NewGuid().ToString('N')
$exportDir = Join-Path $repo ".test-tmp\g4-completed-$runId"
$target = Join-Path $repo 'target-codex-check'
$manifest = Join-Path $repo 'tests\fixtures\g4-native-rel21-v1.json'
$env:G4_EXPORT_DIR = $exportDir
$observations = [System.Collections.Generic.List[object]]::new()

function Invoke-Cargo([string[]] $Arguments) {
    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) { throw "cargo exited with code ${LASTEXITCODE}: $Arguments" }
}
function Invoke-Json([string] $Executable, [string[]] $Arguments) {
    $output = & $Executable @Arguments 2>&1
    $exit = $LASTEXITCODE
    $text = ($output | Out-String).Trim()
    if ($exit -ne 0) { throw "$Executable failed ($exit): $text" }
    $result = $text | ConvertFrom-Json
    $observations.Add(@{ arguments = $Arguments; exit_code = $exit; result = $result })
    return $result
}
function Expect-Failure([string] $Executable, [string[]] $Arguments) {
    $output = & $Executable @Arguments 2>&1
    if ($LASTEXITCODE -eq 0) { throw "Expected failure: $Executable $Arguments" }
    $result = ($output | Out-String) | ConvertFrom-Json
    if ($null -eq $result.error) { throw 'CLI failure did not return a structured JSON error.' }
    $observations.Add(@{ arguments = $Arguments; exit_code = $LASTEXITCODE; result = $result })
}

Push-Location $repo
try {
    Invoke-Cargo @('test', '-p', 'reliquary-runtime-gateway', '--locked', '--offline', '-j', '1', '--target-dir', $target, 'g4_t04_joined_vectors_context_retrieval_and_confirmed_use', '--', '--test-threads=1')
    $env:G4_EXPORT_DIR = "$exportDir-repeat"
    Invoke-Cargo @('test', '-p', 'reliquary-runtime-gateway', '--locked', '--offline', '-j', '1', '--target-dir', $target, 'g4_t04_joined_vectors_context_retrieval_and_confirmed_use', '--', '--test-threads=1')
    $first = Get-Content -LiteralPath (Join-Path $exportDir 'witness.json') -Raw | ConvertFrom-Json
    $second = Get-Content -LiteralPath (Join-Path $env:G4_EXPORT_DIR 'witness.json') -Raw | ConvertFrom-Json
    if (($first.semantics.sources | ConvertTo-Json -Depth 16 -Compress) -ne ($second.semantics.sources | ConvertTo-Json -Depth 16 -Compress)) { throw 'Regenerated authored semantic witnesses are not deterministic.' }
    $env:G4_EXPORT_DIR = $exportDir
    Invoke-Cargo @('build', '--manifest-path', 'cli/Cargo.toml', '--locked', '--offline', '-j', '1', '--target-dir', $target)
    Invoke-Cargo @('build', '--manifest-path', 'tests/external-consumer/Cargo.toml', '--locked', '--offline', '-j', '1', '--target-dir', $target)
    Invoke-Cargo @('test', '--manifest-path', 'tests/external-consumer/Cargo.toml', '--locked', '--offline', '-j', '1', '--target-dir', $target, '--', '--ignored', '--test-threads=1')

    $cli = Join-Path $target 'debug\reliquary.exe'
    $consumer = Join-Path $target 'debug\reliquary-external-consumer-check.exe'
    $sourceFiles = @('rel-lantern.rel', 'rel-cascade.rel', 'phy-reader-a.phy', 'phy-reader-b.phy') | ForEach-Object { Join-Path $exportDir $_ }
    $sourceHashes = $sourceFiles | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash }
    $public = Invoke-Json $consumer @($exportDir, $manifest)
    if (-not $public.source_bytes_unchanged -or $public.authored_turns_checked -lt 40) { throw 'Consumer witness is incomplete.' }

    foreach ($entry in @(
        @{ key = 'rel_lantern'; file = 'rel-lantern.rel' },
        @{ key = 'rel_cascade'; file = 'rel-cascade.rel' }
    )) {
        $ownerPath = Join-Path $exportDir $entry.file
        $info = Invoke-Json $cli @('rel', 'info', $ownerPath)
        if ($info.owner.owner_id -ne $public.($entry.key).owner_id) { throw "CLI and consumer owner IDs differ for $($entry.key)." }
        if ($info.owner.scope -ne $public.($entry.key).scope) { throw "CLI and consumer scopes differ for $($entry.key)." }
        $proof = Invoke-Json $cli @('rel', 'history', 'proof', $ownerPath)
        if ($entry.key -eq 'rel_lantern' -and $proof.proof.digest -ne $public.rel_lantern.history_digest) { throw 'CLI and consumer proof digests differ.' }
        $history = Invoke-Json $cli @('rel', 'history', 'archive', $ownerPath, '--limit', '128')
        if ($entry.key -eq 'rel_lantern' -and $history.page.total -ne $public.rel_lantern.conversations) { throw 'CLI history and consumer conversation totals differ.' }
        if ($entry.key -eq 'rel_cascade' -and $history.page.total -ne $public.rel_cascade.conversations) { throw 'CLI history and consumer conversation totals differ.' }
    }

    $negativeDir = Join-Path $repo ".test-tmp\g4-negative-$runId"
    New-Item -ItemType Directory -Force -Path $negativeDir | Out-Null
    $source = Join-Path $exportDir 'rel-lantern.rel'
    $witness = Get-Content -LiteralPath (Join-Path $exportDir 'witness.json') -Raw | ConvertFrom-Json
    foreach ($owner in $witness.owners) {
        $hash = (Get-FileHash -LiteralPath (Join-Path $exportDir $owner.file) -Algorithm SHA256).Hash
        if ($hash -ne $owner.sha256) { throw "Exported source hash differs for $($owner.file)." }
    }
    $null = Invoke-Json $cli @('rel', 'verify', $source)
    $memoryId = ($witness.semantics.sources | Where-Object { $_.source_id -eq 'src-release-label' }).memory_id
    $memoryHistory = Invoke-Json $cli @('rel', 'history', 'memory', $source, $memoryId)
    if ($memoryHistory.result.status -ne 'available') { throw 'Authored Memory history is unavailable in the CLI.' }
    $created = Join-Path $negativeDir 'cli-created.rel'
    $createdInfo = Invoke-Json $cli @('rel', 'create', $created, '--scope', 'project')
    $createdReopen = Invoke-Json $cli @('rel', 'verify', $created)
    if ($createdInfo.owner.owner_id -ne $createdReopen.owner.owner_id) { throw 'CLI create identity changed after reopen.' }
    $policy = Join-Path $negativeDir 'rewrite-policy.json'
    $witness.rewrite_policy | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $policy -Encoding utf8
    $dryRun = Join-Path $negativeDir 'dry-run.rel'
    $plan = Invoke-Json $cli @('rel', 'rewrite', $source, $dryRun, '--policy', $policy, '--dry-run')
    if ($plan.operation -ne 'rewrite_plan' -or (Test-Path -LiteralPath $dryRun)) { throw 'CLI dry-run published output or returned the wrong operation.' }
    $copy = Join-Path $negativeDir 'reconciled.rel'
    $null = Invoke-Json $cli @('rel', 'reconcile', $source, $source, $copy)
    $copyProof = Invoke-Json $cli @('rel', 'history', 'proof', $copy)
    if ($copyProof.proof.digest -ne $public.rel_lantern.history_digest) { throw 'CLI copy changed retained history.' }
    Expect-Failure $cli @('rel', 'reconcile', $source, $source, $copy)
    Expect-Failure $cli @('cva', 'info', $source)
    Expect-Failure $cli @('phy', 'info', (Join-Path $exportDir 'phy-reader-a.phy'))
    Expect-Failure $cli @('rel', 'info', $source, '--unknown', 'x', '--unknown', 'y')
    $wrongKind = Join-Path $exportDir 'phy-reader-a.phy'
    Expect-Failure $cli @('rel', 'info', $wrongKind)
    $legacyPath = Join-Path $negativeDir 'synthetic-2.0.rel'
    [System.IO.File]::WriteAllBytes($legacyPath, [byte[]](70, 82, 83, 48, 55, 0, 2, 0))
    $legacyHash = (Get-FileHash -LiteralPath $legacyPath -Algorithm SHA256).Hash
    Expect-Failure $cli @('rel', 'info', $legacyPath)
    if ((Get-FileHash -LiteralPath $legacyPath -Algorithm SHA256).Hash -ne $legacyHash) { throw 'Legacy rejection changed its source bytes.' }
    Expect-Failure $cli @('rel', 'info', (Join-Path $exportDir 'rel-lantern.rel'), '--unknown')
    $collision = Join-Path $negativeDir 'collision.rel'
    Copy-Item -LiteralPath (Join-Path $exportDir 'rel-lantern.rel') -Destination $collision
    Expect-Failure $cli @('rel', 'reconcile', (Join-Path $exportDir 'rel-lantern.rel'), $collision, $collision)
    $after = Invoke-Json $consumer @($exportDir, $manifest)
    if (-not $after.source_bytes_unchanged -or $after.rel_lantern.owner_id -ne $public.rel_lantern.owner_id) { throw 'Negative checks changed the completed source cohort.' }
    $afterHashes = $sourceFiles | ForEach-Object { (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash }
    if (($sourceHashes -join ',') -ne ($afterHashes -join ',')) { throw 'CLI reads or negative cases changed source owner bytes.' }
    $after | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath (Join-Path $exportDir 'process-witness.json') -Encoding utf8
    $observations | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath (Join-Path $exportDir 'cli-witness.json') -Encoding utf8
    Write-Output "Completed corpus evidence: $exportDir"
    Write-Output ('G4 fresh-process witness passed: ' + ($after | ConvertTo-Json -Compress))
}
finally {
    Pop-Location
}
