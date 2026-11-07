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

pgc01-binding.provisional.json records the exact PG-C01 synthetic-v1 accepted commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87 (parent dd77dfa8f7d3c73e3e03cd07b2f4f0f7c9850fae), contract-validator hash, registry hashes, and bundle hash. pgc01_adapter.py checks the worktree revision, validator bytes, the official registry validator and all pinned hashes before using the contract. The D01 review record confirms that 0f824a09 changes only the independent test file and leaves the validator, registries, schemas, fixtures, and bundle hashes unchanged from dd77. The adapter verifies the exact commit, parent, diff paths, validator bytes, registry hashes, and bundle hash. It fails closed on any mismatch. This is D01's review for the three-lane gate; it does not accept an authentic source-carrier map or authorize authentic conversion.

The accepted PG-C01 physical-carrier registry remains unclassified-no-authenticated-inventory: it has no carrier entries, no unclassified-carrier list, and authentic admission is false. The CVAAFMT2 test below is only a caller-supplied marker string; it is not read from a source file or admitted. The PG-C01 positive PASS fixture is explicitly synthetic_fixture evidence and cannot authorize source admission.

admission.py maps only the two hand-built D01 fixture families to representative PG-C01 fixture schemas. Its typed synthetic grant permits test-fixture use only and never authentic-source admission. Caller-supplied source markers stay unclassified and cannot enter either fixture or source scope. No physical source-carrier mapping is asserted because F03-V02..V04 have not supplied a reviewed source inventory.

evidence.py builds an in-memory PG-C01-compatible INCOMPLETE report skeleton with zero owners, source hashes, and bindings. It calls the accepted synthetic PG-C01 validator. It cannot build PASS evidence, and the zero-filled binary hash is only the schema's absent-binary placeholder. The report indicates the converter was not executed.

## What this lane contains

- synthetic.py creates/parses only the deliberately separate D01SYN01 test envelope. It never reads files, recognizes no FRS07 marker, and emits no conversion receipt.
- tests/test_synthetic.py covers deterministic synthetic building and malformed/unknown refusal.
- pgc01_adapter.py, admission.py, and evidence.py bind typed synthetic-only classification and incomplete evidence generation to the exact accepted synthetic PG-C01 snapshot.
- synthetic_transaction.py exercises a single-file no-replace link only inside a system TemporaryDirectory. It is a test seam, not a converter writer and not D01-T04 crash-safety evidence.
- pgc01-d01-review.provisional.json records the D01 review of 0f824a09 and the accepted planning record.
- tests/test_pgc01_admission.py checks the accepted pin, synthetic family mappings, empty carrier admission, marker refusal, synthetic-only grants, and the official incomplete-report validator.
- tests/test_synthetic_transaction.py checks temp-directory-only create-only writes, collision preservation, race behavior and failure without unsafe fallback.
- codec-coverage.provisional.json records inspected target seams and explicit blockers. It has zero admitted source carriers and is not the contractual codec-coverage.json.

PG-C01 synthetic v1 was accepted at planning commit b6332a8ff5396a7725c73cbe7643c32950e2de7e. The planning repository's lexical marker inventory is still a search hint only. Neither it nor the empty PG-C01 physical-carrier map is an observed-carrier allowlist. Full physical-carrier coverage must wait for F03-V02..V04's reviewed source-frame inventory; any additional PG-C01 physical-carrier contract revision needs its own review and acceptance.

## Local synthetic checks

Set the contract root to the separate PG-C01 worktree, then run from the product repo root:

    $env:RELIQUARY_PG_C01_ROOT = 'C:\!bin\workspace\Reliquary-pg-c01-v1\tools\corpus-evidence-v1'
    python -B -m unittest discover -s tools/d01-converter/tests -p "test_*.py" -v

These are Python fixture/contract checks only. They do not run Cargo or read authentic source files. The create-only seam writes only under a temporary directory and is not crash-safe or D01-T04 acceptance. No owner-session API is changed. T01 remains incomplete pending F03's complete authenticated source-frame inventory, the full physical/nested family map, owner-boundary gap review, separate-process tooling, and exact final coverage evidence; authentic conversion remains gated by D01-T04, F03-V04, and G4.