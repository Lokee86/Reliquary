# Historical reconstruction end-to-end verification — 2026-10-12

## Purpose

Record Step 25 H1–H8 acceptance and measurement evidence for the supported REL-local historical import shape.

## Overview

This is the final synthetic H1–H8 acceptance record for the historical reconstruction branch. It combines the focused historical fixtures, the coordinator/recovery path, core Freshness regressions, and repository documentation/architecture gates while keeping production activation deferred to Step 26.

## Scope

This gate covers the shipped historical planner, Archive-clock compatibility, semantic and first-cycle Dream planning, import-only propagation, sparse chronological scoring, checkpoint publication/recovery, physical-transform handling, and the one-shot coordinator. Normal live semantic reinforcement remains local and non-propagating.

## Acceptance commands and results

| Gate | Command | Result |
| --- | --- | --- |
| Historical H1–H8 fixtures | `cargo test --locked --lib freshness::historical -j 2` | 35 passed, 0 failed |
| One-shot coordinator | `cargo test --locked --lib historical_import` | 5 passed, 0 failed |
| Full library regression | `cargo test --locked --lib -j 2` | 1,161 passed, 1 ignored, 0 failed; test execution 105.97s |
| Core integration contracts | `cargo test --locked --test freshness_r2_contract --test freshness_f7_acceptance --test freshness_f7_recovery --test freshness_reconcile_contract --test freshness_recovery_contract --test freshness_propagation_oracle -j 2` | 16 passed, 0 failed |
| Documentation policy | `python ../engineering-standards/tools/docs_policy/check.py --repo .` | passed |
| Changed documentation policy | `python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main` | passed |
| Architecture guard | `python scripts/check_architecture.py` | passed |
| Formatting/diff hygiene | `cargo fmt -- --check`; `git diff --check` | passed |

The earlier unrestricted `cargo test --locked` aggregate could not complete because the Windows paging file was exhausted while compiling examples/integration targets. The later `cargo test --locked --tests -j 2` aggregate reached its two-minute command timeout during compilation. Neither failure reported a historical assertion failure; the bounded library and selected integration gates above are the acceptance evidence for this step.

## Coverage and measurement

The repository currently contains no authoritative retained historical REL corpus with provenance-timed admission positions, vector generations, Dream proof, and final graph snapshots. The H1–H8 fixtures therefore use deterministic synthetic inputs. They prove chronology, eligibility, bridge traversal, event identity, propagation, scoring, checkpoint, recovery, and transform behavior, but they do not provide production retained-corpus CPU, memory, vector-work, edge-work, or checkpoint-size measurements.

The bounded coordinator fixture reports a complete projection and idempotent retry; no staged event queue is retained. Coverage is explicit at the planner/semantic/Dream input boundaries, and unsupported or missing provenance is rejected or reported rather than inferred. A retained-corpus measurement run remains required before any default activation decision.

## Supported shape and explicit limitations

- Supported: one complete, frozen, REL-local import with verified Archive chronology, authorized graph/community snapshot, precomputed semantic/Dream plans, deterministic import-only propagation, sparse scoring, and atomic checkpoint publication.
- Supported: repeat import identity, checkpoint reopen/recovery, post-checkpoint live continuation, copy/repack/reclamation preservation, and explicit manifest/policy conflict rejection.
- Not yet production-default: retained-corpus performance/coverage evidence, PHY freshness/import semantics, arbitrary reordered-history conversion, and unbounded or implicit embedding inference.
- The full unrestricted repository gate remains environment-limited by Windows memory/paging capacity; the full library and selected contract gates are green.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical one-shot import coordinator verification](freshness-historical-import-coordinator-verification-2026-10-12.md)
- [Historical manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical import reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)

## Notes

This record closes the Step 25 acceptance gate for the supported synthetic REL-local shape. Step 26 still owns the combined production-readiness and roadmap decision, including whether retained-corpus evidence is sufficient for optional or default activation.
