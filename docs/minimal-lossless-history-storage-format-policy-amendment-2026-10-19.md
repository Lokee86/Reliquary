# Minimal lossless history storage — compatibility-policy interruption and revised remaining execution (2026-10-19)

Parent: [Documentation index](INDEX.md).
Specification: [Development format hard cut](development-format-hard-cut-interruption-spec-2026-10-19.md).
Prior plan: [Minimal lossless history storage](minimal-lossless-history-storage-linear-plan-2026-10-19.md).

Status: **I1–I12 complete; original Steps 1–90 accepted. Runtime retirement not implemented; R1–R22 superseded by integrated linear closeout.** Original source freeze: `39eea4a`; rebased operational checkpoint: `bafdaae` (see [handoff](minimal-lossless-history-storage-rebased-freeze-2026-10-19.md)). Step 91 is next. The shared-mechanics gate was reconciled with passing Memory/build/CLI/doc evidence and refreshed architecture enforcement (23 rules, 20 areas, 91,596 source nodes). This amendment overrides any V1 compatibility-retention language in the original plan/spec. See [interruption execution evidence](development-format-interruption-execution-2026-10-19.md).

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

Validation logs: `.test-tmp/format-interruption-docs-structural.log` and `.test-tmp/format-interruption-docs-impact.log`. These are documentation/inventory acceptances only. Corpus migration and runtime retirement are not accepted.

### Resume the original plan

The remaining implementation plan has been consolidated to **271 total steps**. Accepted Steps 1–90 and their execution records remain intact. The Entity dependency additions discovered after I11 remain in the future sequence; four redundant future reconciliation/benchmark rows were removed or merged, and the old final audits were converted to concrete migration promotion, legacy-reader removal and post-retirement validation gates. Pending row numbers have been resequenced; historical I11 number mappings are evidence only. **R1–R22 no longer exist as a second execution workstream.** That consolidation ended at Step 271. Subsequent execution discovery added eight explicit Relationship consumer/pruning gates, making the current integrated plan 279 steps; final acceptance is Step 279. The consolidation count remains historical evidence.

- Original Step 87: measure raw compact Memory-history bytes before/after. Preserve the existing byte-size acceptance gate.
- Original Steps 88 onward: retain owner-specific compact codec, replay, writing and temporary migration tasks, with their existing test requirements. Revise wording that suggests the old codecs will be permanent.
- At each subsequent owner migration gate: record obsolete reader/codec dependencies explicitly, and keep temporary old-format translator isolated from the normal V2 code path.
- The original final acceptance is **not complete** while any obsolete development format remains a supported runtime read path.

### Integrated final format retirement (supersedes R1–R22)

**R1–R22 are retired as separate execution items.** Their unique requirements are incorporated in the original linear plan's retained fixture migration, exhaustive historical equivalence, canonical fixture promotion, old-reader/upgrader removal and final post-removal verification gates. The current plan has **279 total original-sequence steps**, of which Steps 1–90 remain the last accepted checkpoint in this amendment; consult the current linear plan and execution evidence for newer acceptance.

- The retained six REL/PHY fixtures are converted as part of the existing authoritative fixture migration phase, without parallel duplicate R2–R9 experiments.
- Historical proof, Ellis retrieval and owner-state equality are checked in the existing equivalence and integration gates rather than another R19.
- The six canonical files are promoted with checksum manifests and a fixture commit only after equivalence passes.
- Production V1 readers, CVAFRS03/04/05, CVAILEG1/2, and temporary developer migration tools are removed at the integrated closeout; only format-rejection negative tests remain.
- All root/CLI/structural and historical acceptance is rerun *after* the removal, and a final commit/cleanliness gate closes the plan.

Some V1 conversion code remains necessary **temporarily** for lossless migration and is not an optional feature or permanent compatibility support. This is not a second implementation stream.

## Related docs

- [Development-format hard-cut specification](development-format-hard-cut-interruption-spec-2026-10-19.md)
- [Original lossless-history plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Development-format audit](development-format-compatibility-retirement-audit-2026-10-19.md)

## Notes

This amendment corrects the storage-format *lifecycle*, not the compact-history design. Temporary V1 readers may remain while needed for migration but are not accepted in the final runtime. The planned retirement requires separate code and corpus commits, then resumed original-plan acceptance.
