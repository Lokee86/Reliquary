# D01-T01 provisional synthetic tooling

Status: **T01 remains incomplete**. This tools-only lane contains synthetic fixtures and test command surfaces. It does not implement the FRS07 reader, accepted 2.1 owner writer, source traversal, real candidate creation, or authentic conversion.

The D01 worktree is isolated from the product workspace. These files are under tools/d01-converter/, outside the explicit 23-package production DAG; there is no Cargo manifest here.

## Accepted contract and admission boundary

coverage_map.py loads and verifies the exact-byte-pinned provisional coverage map used by synthetic classification and reports. The map is versioned as provisional-incomplete, requires the accepted PG-C01 synthetic snapshot, and explicitly marks D01-T01 not accepted. Missing or changed map bytes fail closed; this pin does not establish source inventory or T01 acceptance.

PG-C01 synthetic v1 was accepted at planning record b6332a8ff5396a7725c73cbe7643c32950e2de7e for implementation commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87 and bundle digest 7a1abda477874b2a1ff78f75520e075f909750d45a17ce3d6528110334f33688. The accepted scope is synthetic schemas, representative fixtures, independent goldens, and strict validation. Authentic admission remains false; the physical-carrier registry is intentionally empty.

pgc01_adapter.py verifies the exact contract revision, parent, reviewed test-only delta, validator hash, registry hashes, and contract blobs. pgc01-binding.provisional.json and pgc01-d01-review.provisional.json record the accepted synthetic scope and D01's exact-version review. Neither file grants source admission.

admission.py maps only two hand-built D01 fixture families to representative PG-C01 fixture schemas. Its typed grant permits fixture testing only. Caller-supplied markers such as CVAAFMT2 remain unclassified; the lexical marker inventory is discovery material, not a carrier allowlist. Authentic source admission always refuses pending F03-V04.

## Synthetic command surfaces

cli.py provides separate-process synthetic-only inspect, convert, and verify-structure commands. Every subcommand requires --synthetic-fixture. Inputs, outputs, and receipts must be under the system temporary directory. The command prints one structured JSON result to stdout, structured nonzero diagnostics to stderr, and never prints fixture payloads. Every report and receipt binds the exact byte SHA-256 and schema ID of codec-coverage.provisional.json and labels D01-T01 not accepted. coverage_map.py validates the byte pin, accepted PG-C01 synthetic snapshot, and closed fixture-only map; a missing or changed map fails with a structured diagnostic before input is processed.

Example invocations from the product repo root:

    $env:RELIQUARY_PG_C01_ROOT = 'C:\!bin\workspace\Reliquary-pg-c01-v1\tools\corpus-evidence-v1'
    python tools/d01-converter/cli.py inspect --synthetic-fixture --input <temp-fixture>
    python tools/d01-converter/cli.py convert --synthetic-fixture --input <temp-fixture> --expected-sha256 <hash> --output <new-temp-file>
    python tools/d01-converter/cli.py verify-structure --synthetic-fixture --candidate <temp-file> --receipt <saved-convert-json>

The convert command only materializes the same D01SYN01 test envelope as a temporary synthetic artifact. It returns an INCOMPLETE synthetic receipt and never creates a 2.1 owner candidate. verify-structure checks the synthetic envelope, hash, contract binding, and receipt; it does not perform normal current-only owner reopen. The resume command is explicitly unsupported and returns a structured resume_unsupported failure; no checkpoint or resumable conversion state exists. These invocations do not establish the future separate old-reader and target-writer workspaces or processes.

synthetic_transaction.py uses a same-directory hard link for atomic no-replace output under a system temporary directory. Tests cover existing-path collisions, concurrent writers, malformed inputs, and refusing an unsupported no-replace operation. This helper does not sync the containing directory, checkpoint/resume work, inject crash faults, issue owner receipts, or satisfy D01-T04.

## Coverage and target gaps

codec-coverage.provisional.json lists inspected target seams and clearly records zero observed/admitted source carriers. It is not the required codec-coverage.json. F03-V02..V04 must provide and independently reconcile the actual source-frame inventory before a full physical/nested family map can be produced.

target-primitive-gaps.provisional.json records unresolved REL/PHY owner import needs, grouped settlement/version semantics, reference relocation, current-only reopen, and create-only crash-safety work. The D02-owned PHY profile-prune authority fix is a blocker for later synthetic PHY mapping; this lane does not edit owner-session.

## Local checks

Run from the product repo root after setting the PG-C01 contract root:

    $env:RELIQUARY_PG_C01_ROOT = 'C:\!bin\workspace\Reliquary-pg-c01-v1\tools\corpus-evidence-v1'
    python -B -m unittest discover -s tools/d01-converter/tests -p "test_*.py" -v

These Python tests use synthetic data only. They do not run Cargo, read authentic source files, publish target owners, or activate anything. T01 remains incomplete pending F03-V02..V04 source inventory, full carrier/family coverage, the target gaps being resolved at lawful owner boundaries, separate reader/writer processes, and exact code/schema/map hash evidence. Authentic T05 remains gated by D01-T04, F03-V04, and G4.