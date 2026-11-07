param(
    [Parameter(Mandatory)][string]$ValidationRoot,
    [Parameter(Mandatory)][string]$FixtureRepo,
    [Parameter(Mandatory)][string]$CustodyCheck
)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$source = Split-Path $PSScriptRoot
$base = 'c6010fd30dd52acb81c667f7aed99b831307e49f'
$run = Join-Path $ValidationRoot ('.test-tmp\f03-archive-' + [Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($run)

# The older oracle materializes disposable prefix bytes under its checkout.
# Put the entire checkout, corpus, scratch, and logs beneath one private ACL.
$acl = Get-Acl -LiteralPath $run
$acl.SetAccessRuleProtection($true, $false)
foreach ($rule in @($acl.Access)) { $acl.RemoveAccessRuleAll($rule) }
$currentUser = [Security.Principal.WindowsIdentity]::GetCurrent().User
$allowed = @($currentUser.Value, 'S-1-5-18', 'S-1-5-32-544') | Select-Object -Unique
foreach ($identity in $allowed) {
    $sid = [Security.Principal.SecurityIdentifier]::new($identity)
    $rule = [Security.AccessControl.FileSystemAccessRule]::new(
        $sid, 'FullControl', 'ContainerInherit,ObjectInherit', 'None', 'Allow'
    )
    $acl.AddAccessRule($rule)
}
Set-Acl -LiteralPath $run -AclObject $acl
$applied = Get-Acl -LiteralPath $run
$access = $applied.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])
if (!$applied.AreAccessRulesProtected -or
    @(Compare-Object @($allowed | Sort-Object) @($access.IdentityReference.Value | Sort-Object)).Count -ne 0) {
    throw 'Private run ACL did not match expected principals'
}
Write-Output "Private archived-oracle run $run"

$bridge = Join-Path $run 'archive-custody-bridge.json'
python -B (Join-Path $PSScriptRoot 'f03_verify_archived_oracle.py') --historical-repo $source --fixture-repo $FixtureRepo --private-run-root $run > $bridge
if ($LASTEXITCODE -ne 0) { throw 'Archive custody bridge failed' }
$copies = Join-Path $run 'corpus-copies'
$originals = Join-Path $FixtureRepo 'local\authoritative'
python -B $CustodyCheck --source-root $originals --copy-root $copies > (Join-Path $run 'custody-before.json')
if ($LASTEXITCODE -ne 0) { throw 'Original/copy custody failed' }

$archive = Join-Path $run 'historical-source.tar'
git -C $source archive --format=tar -o $archive $base
if ($LASTEXITCODE -ne 0) { throw 'Historical archive failed' }
$snapshot = Join-Path $run 'reader'
[void][IO.Directory]::CreateDirectory($snapshot)
tar.exe -xf $archive -C $snapshot
if ($LASTEXITCODE -ne 0) { throw 'Historical archive extraction failed' }

$patches = @{
    'src/facade/historical_current_corpus_tests.rs' = @{
        'Cva::open(&path)' = 'Cva::open_read_only(&path)'
        'Phylactery::open(&path)' = 'Phylactery::open_read_only(&path)'
    }
    'src/facade/historical_whole_owner_fingerprint.rs' = @{
        'Cva::open(&path)' = 'Cva::open_read_only(&path)'
    }
    'src/facade/historical_whole_phy_fingerprint.rs' = @{
        'Phylactery::open(&path)' = 'Phylactery::open_read_only(&path)'
    }
}
$patchInputs = foreach ($relative in ($patches.Keys | Sort-Object)) {
    $file = Join-Path $snapshot $relative
    $before = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    $body = [IO.File]::ReadAllText($file)
    foreach ($old in $patches[$relative].Keys) {
        if (!$body.Contains($old)) { throw "Read-only instrumentation missing $relative" }
        $body = $body.Replace($old, $patches[$relative][$old])
    }
    [IO.File]::WriteAllText($file, $body, [Text.UTF8Encoding]::new($false))
    [ordered]@{
        relative_path = $relative
        before_sha256 = $before
        after_sha256 = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        replacements = $patches[$relative]
    }
}

$env:RELIQUARY_CURRENT_CORPUS_ROOT = $copies
$env:RELIQUARY_CURRENT_CORPUS_MIGRATION_RECEIPT = Join-Path $run 'archived-migration-receipt.json'
$env:CARGO_TARGET_DIR = Join-Path $ValidationRoot 'target'
$env:TEMP = Join-Path $run 'temp'
$env:TMP = $env:TEMP
[void][IO.Directory]::CreateDirectory($env:TEMP)
$arguments = @('test', '--lib', '--features', 'history-oracle-fixtures', '--locked', '--offline', '-j', '1',
    'canonical_current_corpus_matches_frozen_history_oracle', '--', '--ignored', '--nocapture', '--test-threads=1')
$inputs = [ordered]@{
    reader_commit = $base
    historical_archive_sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    cargo_lock_sha256 = (Get-FileHash -LiteralPath (Join-Path $snapshot 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
    runner_sha256 = (Get-FileHash -LiteralPath $PSCommandPath -Algorithm SHA256).Hash.ToLowerInvariant()
    instrumentation = @($patchInputs)
    rustc = @(rustc -vV)
    cargo = @(cargo -Vv)
    arguments = $arguments
    captured_utc = [DateTime]::UtcNow.ToString('o')
}
$inputs | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $run 'inputs.json') -Encoding utf8NoBOM
$stdout = Join-Path $run 'oracle.stdout.log'
$stderr = Join-Path $run 'oracle.stderr.log'
Push-Location -LiteralPath $snapshot
try {
    cargo @arguments > $stdout 2> $stderr
    $oracleExit = $LASTEXITCODE
} finally { Pop-Location }

python -B $CustodyCheck --source-root $originals --copy-root $copies > (Join-Path $run 'custody-after.json')
$custodyExit = $LASTEXITCODE
$owners = @(Select-String -LiteralPath $stdout -Pattern 'current-format frozen oracle verified (.+)$' |
    ForEach-Object { $_.Matches[0].Groups[1].Value })
$expectedOwners = @((Get-Content -Raw -LiteralPath $bridge | ConvertFrom-Json).fixtures.fixture)
$reportedAll = $owners.Count -eq 6 -and @(Compare-Object $expectedOwners $owners).Count -eq 0
$unchanged = (Get-FileHash -LiteralPath (Join-Path $run 'custody-before.json')).Hash -eq
    (Get-FileHash -LiteralPath (Join-Path $run 'custody-after.json')).Hash
$status = if ($oracleExit -eq 0 -and $custodyExit -eq 0 -and $reportedAll -and $unchanged) { 'PASS' } else { 'FAIL' }
[ordered]@{
    status = $status
    scope = 'fresh-repeat-of-archived-oracle-projection'
    oracle_exit = $oracleExit
    custody_exit = $custodyExit
    reported_all_six = $reportedAll
    reported_owners = $owners
    stdout_sha256 = (Get-FileHash -LiteralPath $stdout).Hash.ToLowerInvariant()
    stderr_sha256 = (Get-FileHash -LiteralPath $stderr).Hash.ToLowerInvariant()
    input_sha256 = (Get-FileHash -LiteralPath (Join-Path $run 'inputs.json')).Hash.ToLowerInvariant()
    captured_utc = [DateTime]::UtcNow.ToString('o')
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $run 'archived-oracle-repeat.json') -Encoding utf8NoBOM
Write-Output "Archived-oracle projection $status"
if ($status -ne 'PASS') { exit 1 }
