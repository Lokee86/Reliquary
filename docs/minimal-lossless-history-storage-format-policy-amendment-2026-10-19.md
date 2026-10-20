# Minimal lossless history storage — compatibility-policy interruption and revised remaining execution (2026-10-19)

Parent: [Documentation index](INDEX.md).
Specification: [Development format hard cut](development-format-hard-cut-interruption-spec-2026-10-19.md).
Prior plan: [Minimal lossless history storage](minimal-lossless-history-storage-linear-plan-2026-10-19.md).

Status: **I1–I12 complete; original Step 87 accepted; Step 88 implementation committed, acceptance not yet recorded. Runtime retirement R1–R22 not implemented.** Original source freeze: `39eea4a`; current rebased operational checkpoint: `bafdaae` (see [handoff](minimal-lossless-history-storage-rebased-freeze-2026-10-19.md)). Steps 1–87 of the original plan are accepted; Step 88 is the next acceptance gate. This amendment overrides any V1 compatibility-retention language in the original plan/spec. See [interruption execution evidence](development-format-interruption-execution-2026-10-19.md).

## Purpose

Enforce current-format-only REL/PHY runtime while preserving historical semantics, without prematurely deleting decoders required for the still-incomplete canonical V2 migration. Preserve the small-step structure of the existing storage plan, interpose a policy correction now, and hard-gate final completion on full corpus migration and retirement.

## Overview

### Immediate interruption: I1–I12

Each item is an independently verifiable gate. Complete these **before** continuing original Step 87.

| Step | Small contained action | Acceptance |
| --- | --- | --- |
| I1 | Confirm source HEAD and index/working-tree cleanliness against the freeze. | Exact hash/dirty inventory saved; no incidental edits. |
| I2 | Inventory authoritative six files and identify other consciously retained development REL/PHY corpora. | Path/hash manifest; no corpus mutation. |
| I3 | Search container V1 dispatch and writer gates. | Exact caller map and classification. |
| I4 | Search CVAFRS03/04/05 decode, normalize and repair paths. | Exact caller map and classification. |
| I5 | Search CVAILEG1/2 frame transport, prefix and epoch readers. | Exact caller map and classification. |
| I6 | Enumerate all temporary V1 upgrade and fixture-migration commands. | Offline-only vs runtime entry points classified. |
| I7 | Identify remaining compact owner codecs/upgrader gaps that block V2-only corpus. | Dependency matrix tied to original remaining steps. |
| I8 | Add policy language to canonical architecture/development docs. | Explicit migrate-commit-remove lifecycle; no accidental release compatibility claims. |
| I9 | Add design-only runtime-version acceptance contract for canonical V2 and fail-closed old formats. | Positive/negative test names and boundaries recorded; no premature reader removal. |
| I10 | Tag every intended temporary migration seam in the owner/caller map for later deletion. | Removal checklist with tests/owner. |
| I11 | Map old plan Steps 87 onward to canonical-writer, temporary-upgrader, and retirement gates without renumbering completed steps. | No disguised multi-step implementation, no lost original work. |
| I12 | Validate documentation/index links and freeze resumed-start state. | Plan and spec coherent, implementation ready. |

No compatibility reader deletion is authorized by I1–I12. The code's V2 owner families are unfinished. This is a **bounded policy correction** ahead of existing Step 87, not a second full rewrite.

### I1–I12 acceptance record

Resumed from clean source `9cf643e`; exact frozen/runtime/fixture hashes and preexisting fixture dirt are in the [inventory manifest](../benchmarks/development-format-interruption-inventory-2026-10-19.json). The [execution record](development-format-interruption-execution-2026-10-19.md) supplies each gate's evidence:

| Item | Accepted evidence |
| --- | --- |
| I1 | Exact clean resume/freeze hashes; runtime diff from freeze is empty. |
| I2 | Six retained V1 containers hash-match accepted baseline; recursive fixture inventory and absent historical CVA candidate recorded; no corpus writes. |
| I3 | T1/T2 container, facade, CLI/host caller and write-barrier map. |
| I4 | T3/T4 legacy Freshness decode, normalization, repair and semantic consumers. |
| I5 | T5/T6 legacy epoch decode, prefix validation and copy/maintenance transport. |
| I6 | T7/T8 distinguish private format upgrade, identity bootstrap, content backfill and destructive rewrite. |
| I7 | Owner dependency matrix ties every known canonical-conversion gap to original steps. |
| I8 | Architecture, development, storage-format and ADR 0041 state migrate/verify/commit/remove policy without claiming retirement. |
| I9 | Six named future runtime acceptance/negative-test contracts; no premature reader removal. |
| I10 | T1–T8 removal tags name owner, source seams, final gate and retained tests. |
| I11 | Exhaustive original 87–266 mapping; three stale future dependency references corrected; completed steps unchanged. |
| I12 | Structural and change-impact documentation checks pass; original steps 1–266 and accepted records 1–86 intact; post-edit fixture hashes and runtime diff unchanged. |

Validation logs: `.test-tmp/format-interruption-docs-structural.log` and `.test-tmp/format-interruption-docs-impact.log`. These are documentation/inventory acceptances only. R1–R22 and corpus/runtime retirement are not accepted.

### Resume the original plan

- Original Step 87: measure raw compact Memory-history bytes before/after. Preserve the existing byte-size acceptance gate.
- Original Steps 88 onward: retain owner-specific compact codec, replay, writing and temporary migration tasks, with their existing test requirements. Revise wording that suggests the old codecs will be permanent.
- At each subsequent owner migration gate: record obsolete reader/codec dependencies explicitly, and keep temporary old-format translator isolated from the normal V2 code path.
- The original final acceptance is **not complete** while any obsolete development format remains a supported runtime read path.

### Mandatory final hard-cut phases (after all V2 owner converters exist)

| Step | Small contained action | Acceptance |
| --- | --- | --- |
| R1 | Record full pre-migration source fixture hash and historical-cut oracle. | Immutable baseline captured. |
| R2 | Migrate the first-14-day REL on a disposable copy. | Current canonical V2 payloads emitted. |
| R3 | Verify first-14-day REL historical cuts and reopen. | Exact parity, no unreported loss. |
| R4 | Migrate the first-28-day REL on a disposable copy. | Current canonical V2 payloads emitted. |
| R5 | Verify first-28-day REL historical cuts and reopen. | Exact parity. |
| R6 | Migrate the Ellis REL on a disposable copy. | Current canonical V2 payloads emitted. |
| R7 | Verify Ellis cut/retrieval parity. | Exact retrieval, profiles, vector and history fidelity. |
| R8 | Migrate three paired PHY files on disposable copies. | All canonical V2. |
| R9 | Verify PHY history, owner identity, vector/profile and relationships. | Exact parity. |
| R10 | Migrate each additional deliberately retained corpus file, one at a time. | Inventoried corpus fully migrated, or typed documented blocker. |
| R11 | Verify migrated corpus has no legacy settlement/epoch/owner records. | Parsed record inventory, not signature-only counts. |
| R12 | Commit canonical fixture corpus in fixture repository. | New hashes/manifest verified, source backups retained until accepted. |
| R13 | Remove V1 normal-runtime container reader/dispatch. | V1 opens reject with typed unsupported error. |
| R14 | Remove CVAFRS03/04/05 runtime settlement decode/normalization. | No old-format runtime decoder reachable. |
| R15 | Remove CVAILEG1/2 runtime epoch/prefix transport. | No legacy runtime dependency reachable. |
| R16 | Remove temporary upgrader APIs/CLI and associated obsolete tests. | Dead-symbol search clean. |
| R17 | Add/retain explicit old-format negative tests. | Fail-closed and current V2 positive tests pass. |
| R18 | Run root and CLI formatting/compilation/test suites. | Pass, or classified unrelated failure with independent rerun. |
| R19 | Run historical, rewrite, prune, reconciliation and Ellis equality gates. | No semantic divergence. |
| R20 | Run structural, docs and cross-repository fixture hash checks. | All required gates pass. |
| R21 | Update canonical docs/ADR to state V2-only reader support. | No residual V1-support promises. |
| R22 | Commit isolated runtime cleanup and final verification evidence. | No accidental unrelated files, explicit hashes. |

R1–R22 are **future mandatory gates**. They cannot be used to claim compatibility-retirement completion before migration. If an owner codec cannot yet translate a record, return to the exact original domain migration step that supplies it rather than silently weakening history equivalence.

## Related docs

- [Development-format hard-cut specification](development-format-hard-cut-interruption-spec-2026-10-19.md)
- [Original lossless-history plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Development-format audit](development-format-compatibility-retirement-audit-2026-10-19.md)

## Notes

This amendment corrects the storage-format *lifecycle*, not the compact-history design. Temporary V1 readers may remain while needed for migration but are not accepted in the final runtime. The planned retirement requires separate code and corpus commits, then resumed original-plan acceptance.
