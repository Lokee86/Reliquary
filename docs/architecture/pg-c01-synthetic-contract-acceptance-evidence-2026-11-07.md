# PG-C01 synthetic contract acceptance evidence — 2026-11-07

## Purpose

Record completion of the PG-C01 synthetic evidence-contract deliverable and its boundary. This ticket freezes the schema/golden tooling version; it does not claim authentic-source admission.

## Overview

- Planning/specification revision: 5d96db991a130d84bf76a720b445295f95ec0204.
- Product-side worktree: C:\!bin\workspace\Reliquary-pg-c01-v1.
- Contract/test commit: 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87.
- Exact v1 contract bundle digest: 7a1abda477874b2a1ff78f75520e075f909750d45a17ce3d6528110334f33688.
- Test command documented in the product worktree: python -m unittest discover -s tools/corpus-evidence-v1/tests -p "test_*.py" -v.
- The reported test run passed 26 tests. The committed suite has 26 test functions. This planning update did not rerun Python or Cargo tests.

The test suite includes a distinct JCS UTF-16 key-order golden, an independent reference encoder comparison against family, owner, and corpus goldens, a second literal golden-vector/alternate-encoding comparison, and strict negative cases for ambiguous JSON, numeric portability, duplicates, unknown/extra fields, empty evidence, malformed bindings, and inconsistent comparator results.

F03, D01, and D02 reviewers gave positive exact-version feedback for the synthetic handoff at the bundle digest above. The product worktree README continues to state the contract's synthetic scope.

## Acceptance boundary

PG-C01 is completed for the synthetic v1 contract, schemas, fixtures, reference encoder goldens, and strict validation cases. Authentic admission remains false. The physical-carrier registry is deliberately empty, the fixture domain IDs are synthetic, and only representative fixture families are defined. No real corpus files were read or converted by this tooling.

F03-V02-V04 remain required for complete authentic source witness, independent historic reconciliation, and frozen source admission. D01 conversion, D02 parity, D03 staging, and G5 release remain separate future gates; this status does not accept or authorize any of them.

## Related docs

- [Post-G4 parallel execution plan](post-g4-parallel-execution-plan-2026-11-06.md)
- [Shared corpus evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md)
- Product-side source: Reliquary-pg-c01-v1@0f824a09fe01f5ff327ab56f3296f36d8f4d7c87:tools/corpus-evidence-v1/README.md

## Notes

The accepted PG-C01 scope is the tested synthetic handoff contract, not the separate future authentic-source registry or D01/D02 production evidence.
