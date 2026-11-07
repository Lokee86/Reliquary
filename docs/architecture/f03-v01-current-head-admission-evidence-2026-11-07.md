# F03-V01 current-head admission evidence — 2026-11-07

## Purpose

Record the accepted F03-V01 copy-only current-head admission. This is limited to custody and successful fresh opens with exact current head tuples; it is not a complete historical semantic witness.

## Overview

- Planning/specification revision: 5d96db991a130d84bf76a720b445295f95ec0204.
- Pinned historical reader baseline: c6010fd30dd52acb81c667f7aed99b831307e49f.
- Independent reviewer verdict: accepted for F03-V01 copy-only current-head admission.
- Validation workspace: C:\!bin\workspace\Reliquary-frs07-validation. Its target directory is a junction to D:\!bin\rust-builds\Reliquary-frs07-validation; the C: target alias and D: physical paths below refer to the same output.

## Verified execution

The custody command python scripts/verify_f03_corpus_custody.py ran in C:\!bin\workspace\Reliquary-crate-first and exited 0 with PASS, physical-custody-only scope. Six authoritative sources and six copies matched in hash, byte length, and UTC mtime before/after; the independent current check rehashed all 12 files and matched their recorded hashes, sizes, and fractional-second mtimes.

The fresh replay command cargo test --test frs07_copy_proof --locked --offline -j 1 -- --nocapture --test-threads=1 ran only against verified copies in the isolated validation worktree. Final raw-log audit reports Cargo exit 0, one test passed, no failures or ignores, all six expected/observed head tuples matched, and source, copies, copy inventory, validation inputs, and toolchain unchanged.

Pinned current head tuples recorded by the passing replay:

- chatgpt-first14d/project.prj.rel: [18938, 7526, 3348, 150, 0, 7913, 7913, 1, 5857]
- chatgpt-first14d/user.phy: [1088, 421, 0, 0, 667, 667]
- chatgpt-first28d/project.prj.rel: [52804, 21068, 5727, 671, 0, 25337, 25337, 1, 16465]
- chatgpt-first28d/user.phy: [2215, 490, 35, 0, 1690, 1637]
- rhelm-david-r-ellis-pre-relationship/project.prj.rel: [11725, 3763, 1452, 199, 0, 6309, 6309, 2, 2972]
- rhelm-david-r-ellis-pre-relationship/user.phy: [5221, 976, 86, 0, 4159, 3889]

The scoped source-only inventory is PASS_SCOPED_BASE_BYTES_MATCH_PLUS_TWO_TEST_ADDITIONS: 1,511 baseline files and 1,511 exact common paths, zero changed or missing paths, plus only tests/frs07_copy_proof.rs and tests/frs07_semantic_snapshot.rs in the 1,513-file snapshot. The inventory baseline commit is c6010fd30dd52acb81c667f7aed99b831307e49f, tree f6a4fa1d3e2cc97a17672d00ad14ed00061da091, aggregate inventory SHA-256 79c1f274147ec263cf7c97b7e62f0947b8861d4639d1c7ac19aa7a7356a0839e.

## Retained evidence paths and hashes

Raw execution logs and receipts are under C:\!bin\workspace\Reliquary-frs07-validation\target. Source-only inventory, patch, receipt, and path addendum are under C:\!bin\workspace\Reliquary-frs07-validation\target\evidence. The addendum confirms target is a junction to D:\!bin\rust-builds\Reliquary-frs07-validation and that the C: alias and D: physical paths resolve to the same output.

- Replay raw log: frs07_copy_proof_warm_replay_20261107T093808Z.log, 58,074 bytes, SHA-256 a1dabfc7e8b902bf2fd33d8281ecce3ffce034957577b0b34c3eebe284a91f7a.
- Replay final receipt: frs07_copy_proof_warm_replay_20261107T093808Z.log.final-receipt.json, 19,462 bytes, SHA-256 debf7e3adb3fceaf64aba3b966698c1f4fac19bf1931fe87505cd0708dbccac6.
- Custody raw log: verify_f03_corpus_custody_20261107T100142Z.log, 2,067 bytes, SHA-256 41cecf6ff8bfcdf6c0809aed139e3b3a970d55594d24910d4a8d6c53fd6e04a5.
- Custody receipt: verify_f03_corpus_custody_20261107T100142Z.receipt.json, 8,411 bytes, SHA-256 bd2c6bee9966a3e62bdd1deb6a0e29e34f74cc13bad622f7898c151084153c5d.
- Source inventory: f03-v01-source-only-inventory-20261107T103350869864Z.json, 895,111 bytes, SHA-256 d2029122f541468ff006518a8c61fe73acd77f856216f404a4110ea5eb04d164.
- Inventory receipt: f03-v01-source-only-receipt-20261107T103350869864Z.json, 2,161 bytes, SHA-256 bfeedea409e2ee0a8bc0bbd59d2660fb6a626cfe7fa0df5cf2ec69a9858c30bc.
- Test-only instrumentation patch: f03-v01-test-instrumentation-20261107T103350869864Z.patch, 8,332 bytes, SHA-256 2b62ee609b2ae53782339e2e0d35c95861f8ffb327dee2306041cd76e1edf79b.
- Path reconciliation addendum: f03-v01-source-only-path-reconciliation-addendum-20261107T105134324Z.json, 3,941 bytes, SHA-256 307d27e8eb315a5d4d2db47be15592a7ec6b2e0be756db8490733d7eb511a797.

## Acceptance boundary

F03-V01 is complete for copy-only current-head admission. The original F03 capability remains completed under its limited historical contract; its corpus_status is updated to this new V01 evidence. V02-V04 remain unstarted. No complete semantic/history fingerprint, independent historical reconciliation, source-admission bundle, conversion, D02 parity, D03 staging, or G5 release is accepted by this result.

## Related docs

- [F03 historical verification specification](f03-historical-verification-implementation-spec-2026-11-06.md)
- [Post-G4 parallel execution plan](post-g4-parallel-execution-plan-2026-11-06.md)
- [Canonical F03 capability record](implementation-rebuild-capabilities-2026-11-03.json)

## Notes

The first automated parser receipt labelled the copy replay as FAIL because its parser missed the first head tuple and expected the wrong test-result punctuation. The raw test log was not changed; the final raw-log audit corrected those parser defects and verified all six tuples plus the passing Cargo result. Both the earlier receipt and final audit remain preserved; this record uses only the final audit.

No authoritative source bytes or corpus copies were written during custody, inventory, or replay. The accepted boundary is opening unchanged disposable copies at their current FRS07 heads, not proving all intermediate history.
