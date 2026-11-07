# D01-T01 provisional tooling lane

Status: **provisional synthetic-only tooling**. This lane does not implement the FRS07 reader, 2.1 target writer, source traversal, candidate publication, or any authentic conversion. T01 is not complete.

This lane is rooted at product integration commit 26e84f45b54985d5aa1ceb3ab8e7fd7f183b8b0a in a dedicated worktree/branch. The root Cargo.toml has an explicit workspace member list; this tooling is under tools/d01-converter/, outside that list and the 23-package production DAG. No Cargo manifest is added here.

## Isolation contract

The eventual disposable converter must use separate processes and separate Rust workspaces:

- source-reader/ is pinned to the verified FRS07 source snapshot and has no target-codec dependency.
- target-writer/ links only accepted current 2.1 owner APIs and has no old-reader/reference dependency.
- a small coordinator owns custody, invocation and receipt assembly, not semantic policy.

Keep the original source read-only. Use synthetic fixtures only until source admission is independently frozen. Do not use a raw Container/store append as a restore API, add an old-format runtime fallback, or move migration semantics into the production DAG. If exact old values cannot be represented by current 2.1, prove the gap with a synthetic owner case and request the narrow value-based operation at its owning boundary.

## Pinned PG-C01 contract

pgc01-binding.provisional.json records the exact PG-C01 provisional commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87 (parent dd77dfa8f7d3c73e3e03cd07b2f4f0f7c9850fae), contract-validator hash, registry hashes, and bundle hash. pgc01_adapter.py checks the worktree revision, validator bytes, the official registry validator and all pinned hashes before using the contract. The D01 review record confirms that 0f824a09 changes only the independent test file and leaves the validator, registries, schemas, fixtures, and bundle hashes unchanged from dd77. The adapter verifies the exact commit, parent, diff paths, validator bytes, registry hashes, and bundle hash. It fails closed on any mismatch. This is D01's scoped review for the three-lane gate, not overall PG-C01 acceptance.

The pinned physical-carrier registry is unclassified-no-authenticated-inventory: it has no carrier entries, no unclassified-carrier list, and authentic admission is false. The CVAAFMT2 test below is only a caller-supplied marker string; it is not read from a source file or admitted. The PG-C01 positive PASS fixture is explicitly synthetic_fixture evidence and cannot authorize source admission.

admission.py has only two classifications: a valid D01SYN01 test envelope is synthetic_fixture_only; a supplied source-marker string is unclassified_source_marker. Both are non-admissible. require_source_admission refuses synthetic frames and refuses source markers while the pinned registry is closed. No source decoder, corpus traversal, or authentic-conversion function exists in this lane.

evidence.py builds an in-memory PG-C01-compatible INCOMPLETE report skeleton with zero owners, source hashes, and bindings. It calls the pinned PG-C01 validator. It cannot build PASS evidence, and the zero-filled binary hash is only the schema's absent-binary placeholder. The report indicates the converter was not executed.

## What this lane contains

- synthetic.py creates/parses only the deliberately separate D01SYN01 test envelope. It never reads files, recognizes no FRS07 marker, and emits no conversion receipt.
- tests/test_synthetic.py covers deterministic synthetic building and malformed/unknown refusal.
- pgc01_adapter.py, admission.py, and evidence.py bind typed synthetic-only classification and incomplete evidence generation to the exact provisional PG-C01 snapshot. pgc01-d01-review.provisional.json records the D01 review of 0f824a09 without claiming overall contract acceptance.
- tests/test_pgc01_admission.py checks pinned hashes, empty carrier admission, marker refusal, synthetic-only classification, and the official incomplete-report validator.
- codec-coverage.provisional.json records inspected target seams and explicit blockers. It has zero admitted source carriers and is not the contractual codec-coverage.json.

The planning repository's lexical marker inventory is a search hint only. Neither it nor the empty provisional PG-C01 carrier map is an observed-carrier allowlist. Full family coverage must wait for an authenticated source-frame inventory and a reviewed PG-C01 contract.

## Local synthetic checks

Set the contract root to the separate PG-C01 worktree, then run from the product repo root:

    $env:RELIQUARY_PG_C01_ROOT = 'C:\!bin\workspace\Reliquary-pg-c01-v1\tools\corpus-evidence-v1'
    python -B -m unittest discover -s tools/d01-converter/tests -p "test_*.py" -v

These are Python fixture/contract checks only. They do not run Cargo, read authentic source files, or create a target candidate. No owner-session API is changed. T01 remains incomplete pending F03's authenticated source-frame inventory, PG-C01 acceptance, owner-boundary gap review, process isolation, and final coverage evidence; authentic conversion remains gated by F03-V04 plus G4.