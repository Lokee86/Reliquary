# Documentation Semantic Audit — 2026-10-21

Parent index: [Documentation index](INDEX.md)

## Overview

Reliquary's documentation enforcement and planning cleanup are now in place, but the canonical reference layer still has semantic drift. This audit records the remaining implementation-to-documentation gaps and separates them into independent repair streams so completeness work can be closed deterministically rather than by prose volume.

## Purpose

This audit evaluates whether Reliquary's current documentation accurately and completely describes the implementation. It is intentionally separate from the planning-document cleanup completed in `a57a823`.

The audit treats the current implementation as the source of truth and checks distinct documentation failure modes independently:

1. public Rust API completeness;
2. persistent/storage-format correctness;
3. architecture and ownership clarity;
4. CLI/configuration/operator-surface completeness;
5. behavioral-contract and protecting-test coverage;
6. current-limitations accuracy;
7. historical/evidence-document taxonomy and user navigation.

The audit does **not** use Arcana. No source code was changed as part of the audit.

## Baseline

Audit results below were confirmed on `main` at `a57a823` after the documentation-enforcement and planning-cleanup work was merged.

At this point:

- documentation enforcement through engineering-standards checks and Archivist is installed;
- the planning cleanup is complete;
- 21 completed/superseded planning documents have been removed;
- current-state subsystem documentation exists under `docs/subsystems/`;
- the remaining problem is semantic completeness and current-state accuracy.

The repository currently contains 168 Markdown files under `docs/`. Sixty-four are date-stamped. Thirty-six date-stamped verification/baseline/acceptance/evidence-style documents remain, totaling 2,241 nonblank lines.

## Executive result

The documentation is structurally stronger than before the cleanup, but the canonical reference layer is still not release-quality.

| Audit | Result | Primary issue |
| --- | --- | --- |
| Public API completeness | **RED** | Public modules and exported runtime surfaces are not exhaustively represented in `api.md`. |
| Storage/persistent-format correctness | **RED — highest priority** | Canonical docs contain mutually contradictory claims about retired legacy migration/runtime-format behavior. |
| Architecture/ownership | **YELLOW/RED** | Macro ownership is present, but current-state documents still contain large amounts of execution-history and obsolete implementation routing. |
| CLI/configuration/operator surface | **RED** | Real CLI/configuration commands and operator behavior are absent or contradicted by manuals. |
| Behavioral contracts/tests | **RED/YELLOW** | 13 of 25 integration-test files are not represented in `behavioral-contracts.md`; some require documentation, others require classification/retirement. |
| Current limitations | **RED** | The file mixes present limitations with completed phase history, old gate results and retired behavior. |
| Evidence taxonomy/navigation | **YELLOW/RED** | Historical evidence remains useful but is tightly coupled into current docs; README/manual navigation contains stale instructions. |

The most important distinction is that these are **different classes of documentation debt**. They should not be repaired as one broad prose rewrite.

---

## Audit 1 — Public Rust API completeness

### Result

**RED**

### Source-derived inventory

`src/lib.rs` currently contains 162 top-level `pub use` or `pub mod` declarations.

There are 12 public root modules. A direct module-name check against `docs/api.md` finds three public modules not represented as public modules in the API reference:

- `owner_lifecycle`
- `external_source`
- `staleness`

The broader export inventory also shows that many public symbol families are described only indirectly or not at all. High-volume examples include:

- Insomnia public contracts and defaults;
- Chronos model types;
- runtime-host and management types;
- live runtime types;
- Community types;
- Context Engine compaction types;
- Entity/Relationship candidate and resolution types.

The problem is not that every re-export must receive one prose paragraph. The problem is that the API reference cannot currently be treated as an exhaustive map of the supported public surface.

### Required repair

Build a deterministic source-to-reference inventory from `src/lib.rs` and classify each public export into one of:

1. documented directly in `api.md`;
2. documented by an explicitly linked canonical subsystem/reference section;
3. intentionally low-level but still part of the supported API;
4. accidental/unwanted public surface that should instead be made private.

The audit should close only when every public export has an explicit disposition.

---

## Audit 2 — Storage and persistent-format correctness

### Result

**RED — highest priority**

### Exact wire inventory

The current production source contains 84 production wire/discriminator constants matching the REL/PHY/CVA record families inspected by the audit.

At least three current production constants are not named in `docs/storage-format.md`:

- `CVACPRT1`
- `CVAERL01`
- `CVAERS02`

These omissions alone do not prove that the complete behavior is undocumented, but they demonstrate that the storage reference is not an exhaustive record-family inventory.

### Hard-cut contradiction

More seriously, the canonical documentation currently disagrees with itself about the development-format hard cut.

`docs/storage-format.md` states near the beginning that:

- `src/storage_upgrade/` has been deleted;
- no runtime upgrade API remains;
- no CLI migration command remains.

Later in the same file, it still states that legacy typed/16-byte inputs remain readable and that `migration::migrate_file` repacks them.

`docs/manual/migration-and-reconciliation.md` correctly states that the temporary `migrate` CLI command and `migrate_file` API are retired and that unsupported old headers fail before writable access.

However:

- `docs/current-limitations.md` still says `migration::migrate_file` and the repo-local `migrate` command are implemented;
- `docs/manual/troubleshooting.md` still instructs operators to run `reliquary migrate <source> <output>`.

The implementation has already removed the old migration/upgrade machinery. This is therefore not a future documentation improvement; it is current factual contradiction.

### Required repair

Rebuild the storage reference around the post-hard-cut implementation:

1. current writable/readable formats only;
2. current record-family inventory;
3. current replay/admission/failure behavior;
4. current maintenance/rewrite/reclamation behavior;
5. historical conversion machinery moved to dated evidence only;
6. no normal-runtime compatibility promise unless the implementation actually retains one.

Operator documentation and current limitations must use the same hard-cut statement.

---

## Audit 3 — Architecture and ownership

### Result

**YELLOW/RED**

### Positive finding

The earlier ownership gap has largely been solved.

All 18 top-level `src/` domains inspected by the audit have a documentation owner across the architecture, maintainer-map and subsystem documentation. The problem is no longer primarily “this subsystem has no owner.”

### Remaining problem: wrong abstraction level

Canonical current-state documents still contain substantial implementation-history language.

A conservative search for phase/step/checkpoint/acceptance/verification terminology currently finds matching lines in:

- `docs/architecture.md`: 25
- `docs/maintainer-map.md`: 55
- `docs/documentation-coverage.md`: 38
- `docs/current-limitations.md`: 48

This is not automatically wrong in every instance, but the concentration is high enough to show that these documents are still carrying execution diaries.

Examples of the wrong abstraction include:

- phase/P-step completion histories in architecture;
- test counts and timed-out historical gates in current limitations;
- completed implementation routing in the maintainer map;
- coverage rows describing how a feature was implemented instead of simply mapping current code to current documentation and protecting tests;
- references to implementation paths that were deleted during the current-format hard cut.

### Required repair

Normalize each canonical document to one job:

- **Architecture:** current owners, boundaries, data flow, lifecycle, failure/recovery, concurrency, invariants.
- **Maintainer Map:** “if changing X, change/check Y,” with current paths only.
- **Documentation Coverage:** current implementation surface → canonical docs → protecting verification.
- **Current Limitations:** only limitations that exist now.

Historical gate results belong in dated evidence documents, not in the canonical current-state layer.

---

## Audit 4 — CLI, configuration and operator surface

### Result

**RED**

### Missing configuration commands

Current `cli/src/config_args.rs` exposes model-route commands that are not represented in `docs/cli.md`:

- `ClearInsomnia`
- `ClearInsomniaMetadata`
- `SetEntityExtraction`
- `ClearEntityExtraction`
- `ClearDream`
- `ClearEmbedding`

The top-level CLI documentation is closer to current code than it was before the cleanup, but detailed route/configuration coverage is not complete.

### Operator contradictions

The more severe problem is stale operational guidance:

- the migration manual says migration is retired;
- troubleshooting still instructs the user to invoke the removed `migrate` command;
- current limitations still claims that command/API exists.

### Required repair

Inventory the Clap command hierarchy and every user-facing configuration route directly from current code, then make:

- `docs/cli.md`
- `docs/configuration.md`
- `docs/manual/**`
- README operator statements

agree with that exact surface.

---

## Audit 5 — Behavioral contracts and protecting tests

### Result

**RED/YELLOW**

There are currently 25 integration-test files under `tests/`.

Thirteen are not represented by filename or stem in `docs/behavioral-contracts.md`:

- `current_format_boundary.rs`
- `external_source_contract.rs`
- `freshness_r2_contract.rs`
- `freshness_reconcile_contract.rs`
- `live_runtime_adapter.rs`
- `multiplexing_baseline_contract.rs`
- `multiplexing_phase0b_conversation_contract.rs`
- `multiplexing_phase0b_events_auth_contract.rs`
- `multiplexing_phase0b_host_contract.rs`
- `multiplexing_phase0b_journal_contract.rs`
- `multiplexing_phase0b_lifecycle_contract.rs`
- `multiplexing_phase0b_personal_contract.rs`
- `workspace_management.rs`

These should not all be blindly added to the behavioral-contract document.

The first group contains clearly current contract surfaces such as current-format admission, external-source behavior, Freshness reconciliation, live runtime and workspace management.

The Phase-0B/baseline multiplexing files require classification:

- if they still protect current behavior, rename/reframe them as current contracts and document them;
- if they only preserve an obsolete implementation phase, either retire them or explicitly retain them as historical regression coverage.

### Required repair

For every integration-test file, record:

1. the invariant/contract it protects;
2. the current owner;
3. whether it is canonical release protection, narrow regression protection, or historical/obsolete coverage.

`behavioral-contracts.md` should then reference the tests that actually protect current behavior without becoming a raw test-file catalog.

---

## Audit 6 — Current limitations

### Result

**RED**

`docs/current-limitations.md` is not currently a reliable “what is limited now?” document.

It contains:

- completed P2/P3/P4/P5 execution history;
- C-step completion/disposition records;
- historical test counts;
- old timeout and build-failure reports;
- superseded release-gate caveats;
- statements describing retired migration behavior as implemented;
- genuine current product limitations mixed into the same chronology.

This makes it difficult to distinguish an actual current limitation from historical implementation context.

### Required repair

Rebuild the document by subsystem and retain only limitations that are true at HEAD.

A limitation entry should state:

- the unavailable/unsupported behavior;
- the current bounded behavior instead;
- failure/refusal semantics when relevant;
- the canonical owner;
- the future plan only if one is still active.

Completed implementation history should move to evidence or disappear if Git history is sufficient.

---

## Audit 7 — Evidence taxonomy and user navigation

### Result

**YELLOW/RED**

The planning cleanup intentionally did **not** bulk-delete evidence.

The repository still contains:

- 64 date-stamped Markdown documents;
- 36 verification/baseline/acceptance/evidence/checkpoint/calibration/disposition/review documents;
- 2,241 nonblank lines in that evidence-like set.

That is not intrinsically excessive. Many of these records contain useful measurements, fixture hashes, acceptance matrices and historical decisions.

The problem is coupling: current canonical docs often depend on dated execution evidence to explain current behavior. This makes old evidence difficult to retire and encourages execution chronology to leak back into Architecture, Coverage and Current Limitations.

### Required repair

Perform a second, evidence-specific audit with three dispositions:

1. **Retain as evidence** — contains measurements, hashes, acceptance artifacts or decision context that remains independently useful.
2. **Graduate and delete** — only useful facts belong in canonical docs; the dated record adds no durable evidence value.
3. **Historical archive/reference** — useful history, but remove it from normal current-state navigation and canonical ownership.

This audit must be conservative. Inbound links are not proof that an evidence file is necessary; they may simply indicate that canonical docs need to stop depending on execution diaries.

---

## Remediation order

The audits should be repaired in this order because later repairs depend on earlier truth being stable.

### 1. Storage hard-cut/current-format truth repair

Fix the current-format/legacy contradictions first across:

- `storage-format.md`
- `api.md`
- `current-limitations.md`
- README
- operator manuals/troubleshooting.

Then inventory all current persistent record families.

### 2. Public API and live runtime/management completeness

Build the source-derived public export inventory and close every public-surface disposition.

### 3. CLI/configuration/manual/README completeness

Make the actual command/configuration tree authoritative and remove stale operator instructions.

### 4. Behavioral-contract/test classification

Classify all integration tests and map current protecting contracts.

### 5. Architecture, Maintainer Map and Documentation Coverage normalization

Remove execution-history contamination and stale implementation routing after API/storage/operator truth is stable.

### 6. Current Limitations reconstruction

Rewrite from current implementation constraints rather than editing the existing chronology in place.

### 7. Evidence-retention audit

Only after canonical docs can stand alone, classify the remaining dated evidence and perform the second pruning pass.

---

## Completion criteria

The documentation-semantic overhaul is complete when all of the following are true:

- every supported public Rust export has an explicit documentation disposition;
- every current persistent record family and format boundary is represented in the storage reference;
- README/manual/API/storage/current-limitations agree on supported format and migration behavior;
- every CLI/configuration command is represented in the appropriate reference;
- every current integration contract has an identified invariant and documentation owner;
- Architecture contains current architecture rather than implementation chronology;
- Maintainer Map contains current paths only;
- Documentation Coverage maps current implementation to canonical docs and protecting tests;
- Current Limitations contains only present limitations;
- retained evidence is intentionally classified;
- Archivist index/link checks pass;
- engineering-standards documentation-policy checks pass.

Passing deterministic structure/index checks alone is not sufficient evidence of semantic completeness.

## Notes

Counts in this audit are source-derived observations at the recorded baseline. Heuristic inventories are used only to identify audit targets; completion criteria require explicit human-readable disposition rather than assuming every source symbol deserves one standalone paragraph.

No Arcana scan was used or required because this audit concerns documentation truth and coverage, not source-architecture graph changes.

## Related docs

- [Documentation policy](documentation-policy.md)
- [Documentation procedure](documentation-procedure.md)
- [Documentation coverage](documentation-coverage.md)
- [Maintainer map](maintainer-map.md)
- [Architecture](architecture.md)
- [Rust API](api.md)
- [Storage format](storage-format.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Current limitations](current-limitations.md)
- [Subsystem documentation](subsystems/INDEX.md)

## Audit disposition

This audit records the remaining documentation work after the planning cleanup. It is not itself a claim that the listed gaps have been repaired.

The next implementation workstream should begin with **Storage hard-cut/current-format truth repair**.
