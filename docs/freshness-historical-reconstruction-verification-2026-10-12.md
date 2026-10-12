# Historical reconstruction partial verification — 2026-10-12

## Purpose

Record actually executed Step 25 primitive/model and selected regression evidence, with integrated native acceptance and retained-corpus measurements still missing.

## Overview

**Status: partial primitive/model and selected regression evidence, not integrated H1–H8 closure.** The [Step 26 audit](freshness-combined-production-review-2026-10-12.md) withdraws the earlier completed disposition. It combines the focused historical fixtures, the coordinator/recovery path, core Freshness regressions, and repository documentation/architecture gates while keeping production activation deferred to Step 26.

## Scope

These commands exercise pure planners/math/models and independent native checkpoint/transform fixtures. The model coordinator does not invoke the native checkpoint or importer, and its inputs do not establish canonical manifest/proof/authorization validation. Native byte crash recovery, worker-barrier/importer wiring and integrated historical-to-live acceptance remain open. Normal live semantic reinforcement remains local and non-propagating.

## Acceptance commands and results

| Gate | Command | Result |
| --- | --- | --- |
| Historical primitive/model fixtures | `cargo test --locked --lib freshness::historical -j 2` | 35 passed, 0 failed |
| Model composition and independent native fixtures | `cargo test --locked --lib historical_import` | 5 passed, 0 failed |
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

- Implemented primitives: pure timeline/semantic/Dream/propagation/scoring and the separately verified narrow native checkpoint. No connected proof-validated importer-to-checkpoint path is established.
- Native evidence: Step 21/23 retry/reopen/live-continuation and physical-preservation fixtures. Step 22 model tests do not prove actual Container crash-prefix recovery.
- Historical production blocked: native proof-validated coordination/recovery and retained-corpus performance/coverage remain open. PHY freshness, arbitrary reordered-history conversion and implicit embedding inference remain unsupported.
- The Step 25 aggregate attempts were environment-limited; the later [Step 26 review](freshness-combined-production-review-2026-10-12.md) completes a fresh full root/CLI run with two jobs and debug information disabled. That later regression pass does not close missing integrated H coordination or retained-corpus measurements.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Historical one-shot import coordinator verification](freshness-historical-import-coordinator-verification-2026-10-12.md)
- [Historical manifest and timeline policy](freshness-historical-manifest-policy.md)
- [Historical import reconstruction specification](freshness-historical-import-reconstruction-spec-2026-10-03.md)
- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)

## Notes

This record preserves executed synthetic/selected results but does not close Step 25. Step 26 blocks historical production/default activation pending actual native coordination, recovery, integrated acceptance and retained-corpus measurement. Live F8 remains independently released as prospective explicit opt-in.
