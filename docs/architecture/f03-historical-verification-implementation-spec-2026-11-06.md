# F03 — complete historical source verification

Parent index: [Architecture](INDEX.md). Parent plan: [post-G4 execution](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation-ready continuation; F03-V01 copy-only current-head admission is accepted; F03-V02 has an isolated source-witness checkpoint in progress; full deep corpus proof remains open.** This does not reopen or rename the already completed original F03 capability, which permitted recording its precise FRS07 blocker. Tickets `F03-V01..V04` close the additional source assurance needed by D01/D02. No source conversion, mutation or activation is authorized here.

## Purpose

Prove the authentic semantics and retained history of all six old-format owners. A hash check proves custody; opening and matching heads proves admission; neither proves full preservation of historical records, Freshness authority, provenance, vector identities or private user state.

## Overview

Read authoritative files only as ordinary immutable bytes. Run historical semantic readers exclusively against verified disposable copies. Freeze independent source witnesses under the [shared evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md), including unavailable history. Compare those witnesses against the archived oracle where its semantics apply, and independently repeat them before D01 consumes authentic sources.

### Verified starting point

- F01 reference: `Reliquary-reference`, `f62b1fb4`, detached/locked. It cannot open the three promoted FRS07 RELs; do not repair it in place.
- Historical reader: `Reliquary-frs07-validation`, exported from `c6010fd3`. `tests/frs07_copy_proof.rs` previously opened all six copies and checked heads.
- Authoritative corpus: `reliquary-fixtures/local/authoritative`, three REL/PHY pairs. Exact file names/hashes/heads are in [the original F03 record](../rebuild-f03-identity-and-corpus-evidence-2026-11-03.md).
- Fresh custody script: `scripts/verify_f03_corpus_custody.py`; six sources and six copies matched, totaling 124,664,599 bytes. The 2026-11-07 F03-V01 replay passed against verified copies and matched all six current heads; it does not establish complete historical semantic coverage.
- Existing historical helpers include `src/facade/historical_current_corpus_tests.rs`, `historical_*_fingerprint.rs`, `historical_cut_enumeration.rs`, and `historical_global_cuts.rs` in the historical snapshot. Many are test-only, and the complete corpus test is gated by `history-oracle-fixtures`.

## Target architecture and ownership

The historical snapshot is a disposable validation dependency, not a dependency of `Reliquary-rebuild`. Use a test-only exporter in the isolated snapshot or a standalone source-reader executable with the same pinned decoder. Preserve a source snapshot digest and an explicit patch containing only validation/export instrumentation. Original FRS07 decoder semantics must not be changed to make a proof pass.

The source reader derives witnesses from accepted typed records and retained revision streams. It may reuse historical codec implementations and canonical fingerprint helpers after reviewing their coverage; it must not use D01's conversion traversal, transfer output or proposed target values. `source-coverage.json` lists actual physical carriers, typed families, historical cuts and sidecar discovery. Old test helpers that call repair, reconstruction, inferred settlement or synchronization are not valid read-only exporters.

Use access-controlled disposable outputs. A copied file's source hash must match before first open and after every read/reopen phase. Record byte length, hash and modification time; do not clear attributes or rewrite source headers. Source changes invalidate the entire run rather than being silently recaptured as new expected data.

## Implementation decisions

### Custody and historical reader provenance

Verify all six hashes against the committed F03 table, not the possibly older promotion JSON. Re-resolve `c6010fd3` to its full commit, inventory source deviations in the validation snapshot and record the exact toolchain/Cargo.lock. Reuse the existing validation build output; do not launch a second Cargo build into G4's target or create additional giant caches by default. If the decoder build needs more than one bounded run, keep an explicit build/result record and continue; a timeout is incomplete work, not a semantic verdict.

### Exhaustive semantic extraction

Export every family and history boundary in the shared contract. Traverse Archive branches/Episodes/attachments, all retained Memory/Entity/Relationship revisions, Graph/Community changes, Ego records, original Freshness event authority/FRS07 causal settlements, Dream/Insomnia/import journals, exact vectors/profiles and original provenance. Zero-row families have explicit evidence. Sidecar queue/host journals are either discovered and hash-bound or explicitly absent; unknown operational state is not equivalent to no pending operations.

Use streaming/incremental cut evaluation. Replace time-budget-based sampling in historical helpers with resumable complete batches for acceptance. Keep sampled diagnostics labeled as sampled and exclude them from the exhaustive gate. Preserve typed intra-settlement unavailability; do not reconstruct an unretained trajectory or grant new Freshness credit. Internal reads cannot reinforce Memory or invoke inference/embeddings.

### Archived-oracle reconciliation

`canonical-history-equivalence-2026-10-21.json` contains pre-FRS07 physical REL hashes. Do not patch them to today's hashes to bypass the old test's source guard. Supply an independently verified custody/migration link or compare a compatible semantic projection with an explicit family-by-family difference ledger. Each difference names old/new schema, exact affected coordinates and why semantic equivalence or an existing availability distinction holds. Unexplained changes remain failures. New expected values must come from authentic old bytes, not the rebuilt implementation.

### Independent repeat and negative evidence

Repeat the export in a separate process on fresh identical copies with no shared mutable projections. Compare owner IDs, complete family/cut counts and canonical digests, then rehash originals and copies. Negative copies cover altered FRS07 authority, unsupported format, truncated/ambiguous carriers, forged owner/source binding and an intermediate revision mutation that converges to the same head. The reader must fail or report the precise typed unavailable state; no automatic repair or new history.

## Tickets

F03-V01 is complete per the linked copy-only current-head evidence. F03-V02 has an isolated checkpoint only; its full witness acceptance is not recorded. F03-V03-V04 remain not started; earlier custody work does not complete the larger semantic/history tickets.

| Ticket | Hard predecessors | Deliverable and scope | Executable acceptance / evidence |
| --- | --- | --- | --- |
| F03-V01 — authenticate reader and source | None | Six-source custody inventory; pinned historical decoder provenance; fresh copy-only admission | Existing custody script and `frs07_copy_proof` both exit 0; exact six head tuples and unchanged before/after hashes; binary/source/toolchain digests; [accepted V01 evidence](f03-v01-current-head-admission-evidence-2026-11-07.md) |
| F03-V02 — complete source witness | PG-C01, F03-V01 | Test-only old-reader exporter and full source coverage, including unavailable cuts | Export each source in bounded batches; exhaustive family/cut census, no omissions/unknown carriers, no inference/writes; reviewed source witness manifest |
| F03-V03 — independently reconcile history | F03-V02 | Separate-process repeat, archived-oracle difference ledger and negative fixtures | Exact repeat equality; historical intermediate mutation is detected; all negative cases fail closed; no unexplained semantic or availability difference |
| F03-V04 — freeze source admission | F03-V03 | Reviewed `source-admission.json` binding six source/witness/schema/code digests | Independent review of coverage and authority, immutable private artifact custody, repeatable commands, evidence committed; no D01/D02/G5 completion claim |

## Testing decisions and commands

Existing checks, from their respective repositories:

```text
python scripts/verify_f03_corpus_custody.py
cargo test --test frs07_copy_proof --locked --offline -j 1 -- --nocapture --test-threads=1
```

The second command is run only in the isolated historical validation tree. V02 must add one documented exporter entry point with `--source-root`, `--expected-custody`, `--output`, `--resume` and `--verify-only`; concrete naming belongs to the isolated tooling, not Root. It must emit nonzero for mismatch/incomplete coverage and a final machine-readable report. The historical feature-gated oracle may be used only with a validated source-hash bridge; an ignored test or old-reader reject is not a pass.

Tests include all six identity goldens as a cross-check but distinguish golden construction from corpus-observed identities. A normal open must not rewrite or normalize the file. Capture command exits, full logs, execution source revisions and batch continuity; do not accept partial log tails as proof that all six files ran.

## Completion gate and handoff

F03 deep source assurance closes only with six authentic independently repeated source witnesses, complete supported-history coverage, explained archived-oracle differences, and explicit negative results. D01 may then convert verified copies; D02 receives the immutable expected bundle independently. Update the original F03 `corpus_status` and link the new evidence without changing its historical original completion rationale.

## Out of scope

Converting source bytes, modifying the pinned reference, changing FRS07 semantics, repairing old data, accepting new 2.1 outputs, activation, live model quality or public disclosure of corpus content.

## Related docs

- [Shared evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md)
- [Current continuation evidence](../f03-independent-verification-continuation-2026-11-06.md)
- [D01 converter](d01-offline-converter-implementation-spec-2026-11-06.md)
- [D02 parity](d02-independent-parity-verifier-implementation-spec-2026-11-06.md)

## Notes

The historical export is temporary assurance tooling. Frozen source evidence and rollback bytes remain after converter retirement; no old decoder ships in the production 2.1 dependency graph. Missing proof blocks acceptance/conversion of that source, not continued implementation on synthetic fixtures.
