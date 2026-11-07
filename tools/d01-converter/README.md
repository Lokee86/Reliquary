# D01 provisional synthetic tooling

Status: **T01 remains incomplete**. This lane contains synthetic fixtures and test command surfaces, including native 2.1 writers for ordered synthetic REL Archive and ungrouped PHY Memory history. Source traversal and authentic conversion are pending.

Converter tools live under tools/d01-converter/, outside the explicit 23-package production DAG. The native writer has its own Cargo workspace and depends on current 2.1 codecs and owner-session.

## Accepted contract and admission boundary

coverage_map.py loads and verifies the exact-byte-pinned provisional coverage map used by synthetic classification and reports. The map is versioned as provisional-incomplete, requires the accepted PG-C01 synthetic snapshot, and explicitly marks D01-T01 not accepted. Missing or changed map bytes fail closed; this pin does not establish source inventory or T01 acceptance.

PG-C01 synthetic v1 was accepted at planning record b6332a8ff5396a7725c73cbe7643c32950e2de7e for implementation commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87 and bundle digest 7a1abda477874b2a1ff78f75520e075f909750d45a17ce3d6528110334f33688. The accepted scope is synthetic schemas, representative fixtures, independent goldens, and strict validation. Authentic admission remains false; the physical-carrier registry is intentionally empty.

pgc01_adapter.py verifies the exact contract revision, parent, reviewed test-only delta, validator hash, registry hashes, and contract blobs. pgc01-binding.provisional.json and pgc01-d01-review.provisional.json record the accepted synthetic scope and D01's exact-version review. Neither file grants source admission.

admission.py maps only two hand-built D01 fixture families to representative PG-C01 fixture schemas. Its typed grant permits fixture testing only. Caller-supplied markers such as CVAAFMT2 remain unclassified; the lexical marker inventory is discovery material, not a carrier allowlist. Authentic source admission always refuses pending F03-V04.

## Synthetic command surfaces

cli.py provides separate-process synthetic-only inspect, convert, and verify-structure commands. Every subcommand requires --synthetic-fixture. Inputs, outputs, and receipts must be under the system temporary directory. Set TEMP and TMP to a directory inside the active worktree when running these commands on Windows. The command prints one structured JSON result to stdout, structured nonzero diagnostics to stderr, and never prints fixture payloads. Every report and receipt binds the exact byte SHA-256 and schema ID of codec-coverage.provisional.json and labels D01-T01 not accepted. coverage_map.py validates the byte pin, accepted PG-C01 synthetic snapshot, and closed fixture-only map; a missing or changed map fails with a structured diagnostic before input is processed.

Example invocations from the product repo root:

    $env:RELIQUARY_PG_C01_ROOT = 'C:\!bin\workspace\Reliquary-pg-c01-v1\tools\corpus-evidence-v1'
    python tools/d01-converter/cli.py inspect --synthetic-fixture --input <temp-fixture>
    python tools/d01-converter/cli.py convert --synthetic-fixture --input <temp-fixture> --expected-sha256 <hash> --output <new-temp-file>
    python tools/d01-converter/cli.py verify-structure --synthetic-fixture --candidate <temp-file> --receipt <saved-convert-json>

The convert command only materializes the same D01SYN01 test envelope as a temporary synthetic artifact. It returns an INCOMPLETE synthetic receipt and never creates a 2.1 owner candidate. verify-structure checks the synthetic envelope, hash, contract binding, and receipt; it does not perform normal current-only owner reopen. The resume command is explicitly unsupported and returns a structured resume_unsupported failure; no checkpoint or resumable conversion state exists. These invocations do not establish the future separate old-reader and target-writer workspaces or processes.

synthetic_transaction.py uses a same-directory hard link for atomic no-replace output under a system temporary directory. Tests cover existing-path collisions, concurrent writers, malformed inputs, and refusing an unsupported no-replace operation. This helper does not sync the containing directory, checkpoint/resume work, inject crash faults, issue owner receipts, or satisfy D01-T04.

## Native 2.1 synthetic Archive writer

The separate Rust package under `target-writer/` writes an attachment-free
synthetic REL Archive history. Its private transfer format orders accepted
turns and branch revisions independently, carries exact local versions and
nanosecond timestamps as decimal strings, and binds owner identity, scope,
genesis generation and content IDs. The checked-in synthetic transfer has no
corpus inputs. The target process accepts the fixture SHA-256 explicitly.

The writer uses the current Archive codec through narrow owner-session
operations. It captures a proof at each revision, releases the writer, and
reopens the staged REL through ordinary current-only admission. Each cut must
produce the same proof after reopen. A same-filesystem hard link publishes the
file only after proof and source checks pass and fails on a collision. The
synthetic receipt includes the hashes of the transfer, writer and candidate,
and the per-cut proof digests.

Run from the D01 worktree:

    cargo test --manifest-path tools/d01-converter/target-writer/Cargo.toml --locked --offline -j 1 -- --test-threads=1
    $fixture = 'tools/d01-converter/target-writer/tests/ordered-archive-transfer.json'
    New-Item -Force -ItemType Directory '.test-tmp' | Out-Null
    cargo run --manifest-path tools/d01-converter/target-writer/Cargo.toml --locked --offline -j 1 -- --synthetic-fixture $fixture (Get-FileHash $fixture -Algorithm SHA256).Hash.ToLowerInvariant() '.test-tmp/synthetic-archive.rel'

The native test verifies exact node, parent, content and clock identity, checks
current-only reopening and detects histories with the same final branch but
different intermediate cuts. Its negative cases cover conflicting IDs,
noncanonical clocks, stale versions, dangling leaves, unknown families, input
hash mismatch and output collisions.

Native synthetic scope is limited to attachment-free Archive turns and branch
revisions with genesis lineage. Source FRS07 traversal, other retained
families, global clock reservations, grouped writes, retained physical
reference relocation and authentic source admission remain open. The
create-only fixture publication does not establish the D01-T04 crash matrix.

FRS07 Archive publications carry local and global coordinates together.
`Container::append_bound_publication_envelope` can bind a current physical
version, but the current Archive reader only decodes top-level Archive records.
The product-level probe binds the same typed turn to two distinct reservations.
Both physical envelopes reopen at their respective global versions, while
neither binding enters the Archive history. The converter therefore rejects
synthetic transfers that supply a global binding, even when their local
history matches a supported fixture. Exact authentic replay needs an Archive
representation and normal read-path support for the binding, plus review of
the source publication families and repeats.

## Native 2.1 synthetic PHY Memory writer

The same isolated writer accepts a separate fixture containing ungrouped
private Memory creation and delta publications. It carries the original stable
Memory and body IDs, per-Memory revisions, owner-local Memory versions, dense
global versions and exact transaction times. The writer checks each accepted
record against the current Memory codec before appending it. Ordinary current
PHY admission must reproduce every typed revision and the physical
publication scan must reproduce every local/global binding and timestamp
before a candidate is published.

    $fixture = 'tools/d01-converter/target-writer/tests/phy-memory-transfer.json'
    New-Item -Force -ItemType Directory '.test-tmp' | Out-Null
    cargo run --manifest-path tools/d01-converter/target-writer/Cargo.toml --locked --offline -j 1 -- --synthetic-memory-fixture $fixture (Get-FileHash $fixture -Algorithm SHA256).Hash.ToLowerInvariant() '.test-tmp/synthetic-memory.phy'

The receipt is synthetic and provisional. This fixture exercises an available
immutable body, two retained revisions, changed metadata, and transaction
times above 2^53. Pruned or unavailable bodies, sparse global clocks,
reserved/bound publications, cross-owner provenance and grouped work need
additional typed conversion paths. This path does not resolve the Archive
global binding gap.

## Coverage and target gaps

codec-coverage.provisional.json lists inspected target seams and clearly records zero observed/admitted source carriers. It is not the required codec-coverage.json. F03-V02..V04 must provide and independently reconcile the actual source-frame inventory before a full physical/nested family map can be produced.

target-primitive-gaps.provisional.json records unresolved REL/PHY owner import needs, grouped settlement/version semantics, reference relocation, current-only reopen, and create-only crash-safety work. The D02-owned PHY profile-prune authority fix is a blocker for later synthetic PHY mapping; this lane does not edit owner-session.

## Local checks

Run from the product repo root after setting the PG-C01 contract root:

    $env:RELIQUARY_PG_C01_ROOT = 'C:\!bin\workspace\Reliquary-pg-c01-v1\tools\corpus-evidence-v1'
    python -B -m unittest discover -s tools/d01-converter/tests -p "test_*.py" -v

These Python tests use synthetic data only. They do not run Cargo, read authentic source files, publish target owners, or activate anything. T01 remains incomplete pending F03-V02..V04 source inventory, full carrier/family coverage, the target gaps being resolved at lawful owner boundaries, separate reader/writer processes, and exact code/schema/map hash evidence. Authentic T05 remains gated by D01-T04, F03-V04, and G4.