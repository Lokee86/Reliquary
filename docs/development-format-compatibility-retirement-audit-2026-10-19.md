# Development-format compatibility retirement audit (2026-10-19)

Parent: [Documentation index](INDEX.md).

## Purpose

Enforce the development persistence policy: after a format change, migrate the controlled development corpus to the canonical format immediately, verify it, commit the format and corpus, and remove all temporary migration machinery. Older formats have no permanent runtime-read compatibility by default; exceptions require a documented reason.

## Overview

Status: **Audit performed; retirement and migration not yet complete.** Do not classify Step 65's compatibility matrix as ongoing acceptance for supported production formats.

The current canonical Freshness import settlement writer emits `CVAFRS06`. Runtime compatibility still accepts `CVAFRS03`, `CVAFRS04`, `CVAFRS05` via `src/freshness/freshness_import_compatibility.rs`, and `CVAILEG1` via `src/imports/legacy_epoch.rs` and `src/imports/epoch_store.rs`. The public `Cva::normalize_import_settlements()` and repair normalization path provide retained compatibility. `src/facade/cva_historical_import.rs` also contains legacy import decoding/prefix evidence. Retiring the decoder alone is insufficient.

An independent binary-signature scan (ASCII markers in the complete six files) of `C:\!bin\workspace\reliquary-fixtures\local\authoritative` found exactly one `CVAFRS03` and one `CVAILEG1` marker in each of the `chatgpt-first14d`, `chatgpt-first28d`, and `rhelm-david-r-ellis-pre-relationship` REL files. No `CVAFRS04`, `CVAFRS05`, or `CVAFRS06` markers occur in those six files, and the three paired PHY files contain none of these five markers. The counts are occurrence evidence, not parser-level validation or a full inventory of other development files.

The Reliquary main checkout currently has 130 modified or untracked paths across several concurrent features. Neither authoritative fixtures nor runtime compatibility files were modified by this audit. No migration, source cleanup, test verification, or commit has been completed.

### Required retirement gates

1. Inventory all retained development REL/PHY corpora, not only the six authoritative files; identify every prior format by parsed payload type and capture immutable original SHA-256 and logical state baselines.
2. On disposable copies, migrate all retained fixtures to canonical records, including semantic import settlement and epoch provenance. Avoid simply appending a V6 successor while leaving V3/V4/V5/CVAILEG1 bytes that would still require obsolete readers. If current rewrite machinery cannot preserve required historical cuts, extend the **temporary offline converter**, not production readers.
3. Verify reopen, import completion/retry, full historical-state reconstruction, Owner/Memory/Entity/Relationship/Graph/vector data, and Step 61 Ellis retrieval parity. Ensure old/current file histories match for intended preserved snapshots and record any intentionally rewritten history explicitly.
4. Publish and hash-verify canonical fixture replacements, commit the fixture corpus in its own repository, and only then remove legacy settlement/epoch readers, normalization APIs, orphaned guards, old-format tests, and obsolete documentation from Reliquary. Preserve tests asserting that unsupported older formats fail closed.
5. Run root formatting/build/tests, CLI tests, rewrite/REL/PHY prune contracts, authoritative fixture parity, and structural/policy checks. Commit only cohesive related source changes; do not stage unrelated dirty main changes.
6. Document every genuine compatibility exception with consumer, supported version, sunset condition, and owner. Absent such justification, the runtime accepts current format only.

## Related docs

- [Storage format](storage-format.md)
- [Semantic import and history rewrite evidence](semantic-import-pruning-history-rewrite-evidence-2026-10-18.md)
- [Semantic maintenance](subsystems/semantic-maintenance.md)

## Notes

The legacy corpus is a concrete migration dependency. Deleting readers immediately would make the known authoritative REL files inaccessible. Migration must precede deletion. There is no evidence here of a justified publicly supported legacy reader. This audit is not a claim of completed remediation.
