# F03 independent verification continuation — 2026-11-06

**Status: IN PROGRESS; conversion and activation not authorized.** This record extends, and does not replace, [F03 identity and corpus evidence](rebuild-f03-identity-and-corpus-evidence-2026-11-03.md). The canonical F03 capability is already marked completed for its limited original contract; this work is independent *deeper corpus assurance* needed before D01/D02. G4 remains a separate gate.

## Baseline and isolation

- Authoritative six owner files: `C:\!bin\workspace\reliquary-fixtures\local\authoritative`.
- Disposable historical validation copies: `C:\!bin\workspace\Reliquary-frs07-validation\corpus-copies`.
- Pinned source reference: `Reliquary-reference` at `f62b1fb4`. Historical FRS07-capable source snapshot: `c6010fd3` in `Reliquary-frs07-validation`.
- The prior `frs07_copy_proof` test opened all six copies and matched each expected owner-head tuple using the historical decoder (2026-11-04 evidence). The pinned F01 reader still rejects the three REL files due to its older Freshness schema.
- Never open or modify the authoritative owner files as writable stores. No 2.0 reader may be linked into the new 2.1 runtime. G4 is not affected by this work.

## Fresh physical-custody verification

A read-only Python SHA-256 check of **all six authoritative files and all six validation copies** completed on 2026-11-06 (local) with **exit code 0**. Every source digest and copy digest equalled the exact pinned F03 values; sizes matched and modification times remained unchanged during inspection. Aggregate corpus size: **124,664,599 bytes**. File-level SHA-256 values remain in the authoritative [F03 table](rebuild-f03-identity-and-corpus-evidence-2026-11-03.md).

Reproducible check: `python scripts/verify_f03_corpus_custody.py` from `Reliquary-crate-first`; it reads paths without writing owner bytes. Scope is **physical custody only**; it does not establish historical equivalence.

## New historical-decoder rerun

Attempted `cargo test --test frs07_copy_proof --locked --offline -j 1 -- --nocapture` in the isolated validation checkout. The command timed out after 600 seconds **while compiling third-party dependencies** in a cold Cargo target directory. No test executable was launched, and **no fresh FRS07 replay PASS is claimed**. The previous successful six-file historical proof remains the only executed admission/head check.

Do not run concurrent Cargo builds in the active G4 target directory or discard its cache; resume this check only with an isolated validated cache/build target and enough disk capacity. Warnings or compilation progress are not test evidence.

## What still needs independent proof

Execution now follows the [F03 historical verification specification](architecture/f03-historical-verification-implementation-spec-2026-11-06.md) and its F03-V01–V04 tickets. This link adds an execution contract; it does not upgrade the evidence below.

1. Re-run the six-file historical decoder test on copies and retain the exact test exit result and owner-head observations.
2. Produce a versioned **FRS07 source-state semantic witness** for each owner: owner identity, canonical per-domain history and cuts, Archive source bytes/branches/Episodes, Memory body/provenance and revisions, Entity/Relationship/Graph/Ego state, Freshness events and import settlements, Dream/Insomnia work receipts, vector/profile IDs, source references and typed unavailable history. Hash stable semantic rows; exclude physical offsets and potentially sensitive raw content from the report.
3. Compare the FRS07 witness with the prior frozen `canonical-history-equivalence-2026-10-21.json` oracle where semantics are unchanged. **The old oracle describes the pre-FRS07 canonical physical files** (its REL SHA values differ from the promoted FRS07 SHA values): differences in FRS07 settlement encoding and retained-history availability require explicit accepted explanations, not blanket equivalence or silent normalization.
4. Independently reopen each copy and compare the witness again. Test corrupted/unsupported historical states on *separate disposable negative copies*; sources remain unchanged.
5. Freeze the reviewed source and expected semantic witness for D01 (offline 2.0→2.1 conversion) and D02 comparison. **Do not** start conversion/activation until the G4 dependency and historical source proof both pass.

## Acceptance boundary

The current PASS is six-file *hash custody*. The historical decoder admission was proven earlier and awaits a fresh rerun. Full historical semantic fingerprints, FRS07-specific source authority and independent conversion comparison are **not yet accepted**. Do not upgrade D01/D02/G5 status from this document.
