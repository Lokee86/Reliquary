# Combined Freshness production review — 2026-10-12

## Purpose

Record Step 26's F8 + historical reconstruction audit against ADR 0039, the frozen H1 contract, owner/authorization boundaries and actual importer/recovery evidence.

## Overview

**Disposition: retain live F8 as prospective explicit owner opt-in v1. Historical reconstruction remains experimental development primitives, with production activation blocked and no automatic/default importer activation. PHY Freshness remains deferred.** This decision does not enable a lane, change scoring constants or alter persisted records.

The review at branch input `28923dc` found that the Step 22/24/25 completion claims exceeded their evidence. Native `CVAFRS03` acceptance and the H7 preservation matrix are implemented. The new `HistoricalCheckpointStore` is a separate in-memory model, and `execute_historical_import` composes supplied plans into that model. Neither is connected to the durable Cva checkpoint or an importer. Thus a final production review can close with a blocked disposition, but cannot claim Steps 1–25 achieved integrated historical reconstruction.

## Audit results

| Boundary | Source/evidence | Finding |
| --- | --- | --- |
| Live F8 ownership and clock | `cva_semantic_live.rs`, `cva_semantic_source.rs`, `freshness_semantic_activation.rs`; Step 12 release record | Prospective explicit activation, immutable accepted source identity, current owner-local body/vector bindings and direct effects use existing Freshness durability. No historical backfill/default enablement is added. |
| Live privacy and authorization | Current Cva source/target reads and existing runtime owner admission | Eligible candidates come from the selected owner's Memory/vector stores; real owner authorization remains the host/gateway's responsibility. Pure planning APIs do not grant access. Provider work retains existing outside-lock boundaries. |
| Live graph/use semantics | Direct semantic commit, R2/F8 tests | Semantic relevance is local, not graph propagation or fabricated +25 use. Genuine independent accepted-use and actual first-cycle Dream retain their existing principals. |
| Historical math/topology | Timeline, semantic/Dream planners, recipient-only propagation, sparse scorer | Pure primitives have useful deterministic synthetic evidence. Caller-supplied graph/authorized sets and plans alone do not prove owner, source, topology, vector or cohort authority. |
| Native checkpoint/maintenance | `cva_historical_import.rs`, `historical_import_input.rs`, `freshness_import_checkpoint.rs`, H7 fixtures | Real `CVAFRS03` verifies the narrow native cohort/source/clock/input cut, preserves receipt/Dream authority and retains later live effects. The full supplied manifest/contribution proof remains a caller obligation. Divergent/source-identity conversion rejects. |
| Step 22 recovery model | `historical_checkpoint.rs` and three model tests | No codec, owner UUID, Container I/O or actual REL reopening. Clearing a map is not a byte crash-prefix test. This is partial model evidence, not closure of native checkpoint recovery. |
| Step 24 orchestration | `historical_import.rs`, two empty-event model tests, repository call-site search | No Cva/importer/runtime call sites; no durable checkpoint commit. Timeline/provenance are checked only for nonempty collections, and IDs/digests only for nonzero bytes. Full manifest, Archive clock, owner/graph/Community/vector/cohort proof and worker barrier validation are absent. |
| Historical identity/exclusion | `historical_semantic.rs::event_id`, `historical_import.rs::contribution_id` and `from_semantic(..., BTreeSet::new())` | Planner semantic IDs depend on effects and omit canonical H1 import identity; the model rehashes strings and derives recipient-specific IDs. Canonical source-event ordering/identity and full self/cohort credit exclusions are not wired. |
| Import/embedding contract | `cli/src/import_cmd.rs`, runtime call-site search | The importer ingests supplied line order and does not invoke historical reconstruction. No bounded opt-in embedding-only orchestration with cancellation/coverage is implemented; accepting a precomputed plan does not implement that operation. |
| Acceptance and measurement | Step 25 record | Synthetic and selected regression passes remain valid evidence. They do not prove an integrated native H1–H8 coordinator. Retained-corpus CPU/memory/vector/edge/checkpoint measurements and production relevance evidence are absent; its unrestricted full gate did not complete. |

The in-memory model also mutates its original checkpoint during post-event application, so replaying the original import after a live contribution conflicts rather than preserving that live state. Its global event-ID set can suppress a second recipient of one event, and contributions lack real accepted-use/REL-cut validation. These are reasons to keep it a development model and use the existing native Freshness owner for future integration, not to install it as another scoring ledger. Source documentation now identifies this boundary explicitly; no behavior or public signature changes here.

## Required work before historical release

1. Finish Step 24 through the existing Cva/Freshness owner: construct and verify canonical H1 identity/manifest, complete native generation barrier, original Archive chronology, full owner-authorized graph/Community/vector/body/cohort proofs, source/cohort exclusions and canonical event IDs. Feed verified contributions into the frozen native prepare/commit handle; no model checkpoint may substitute for acceptance.
2. Close Step 22 with actual Container byte crash prefixes, passive reopen, due-index reconstruction, exact retries after live growth and multi-recipient real accepted-use receipts. Keep the native checkpoint/receipt owner as sole authority.
3. Implement or explicitly defer the bounded embedding-only operation with true opt-in limits, worker/cancellation boundaries and coverage reporting. Add a real importer/embedding-to-native-checkpoint fixture; preserve already-enrolled owners and reject incompatible original acceptance order.
4. Repeat Step 25 against that integrated path, including F8/local-vs-import divergence, authorization, physical preservation and unsupported conversion fixtures, full root/CLI/docs/architecture gates, and hash-verified retained-corpus measurements. Do not infer inference/vector cost or production SLOs from empty-event model tests.
5. Revisit default activation only after the above evidence and semantic-quality acceptance. A separate PHY owner/activity-clock design and review are required before any PHY Freshness work; no REL counter may be borrowed.

## Verification and status correction

Step 26 is complete as a production audit and disposition. Steps 22 and 24 are partial model implementations; Step 25 is partial synthetic/selected regression evidence. Their dated records retain actual commands/results but now explicitly withdraw integrated completion claims. Steps 14–21 and 23 remain scoped primitive/native-checkpoint/transformation evidence; they do not imply a shipped historical importer.

Review verification uses existing live semantic, historical planning/model and native checkpoint/transform fixtures, plus compile/format/CLI/docs/architecture checks. Passing these existing tests cannot close missing integration or measurement gates. This review changes documentation and source API comments only; numeric policy, owner admission, durable formats and activation defaults remain unchanged.

Executed Step 26 gates:

| Gate | Result |
| --- | --- |
| Root formatting and locked compile | Passed |
| `cargo test --locked -j 2 --target-dir target/step25-lib` | 1,161 unit tests passed, one existing ignored; all integration/doc targets passed; unit execution 90.80s |
| CLI formatting, locked check/test with two jobs | Passed; all 19 CLI tests passed |
| Documentation structural and committed change-impact checks | Passed |
| Refreshed architecture enforcement | Six rules passed; 66,519 source nodes, zero relationships |
| Patch hygiene | `git diff --check` passed |

Compilation used process-local `CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0`, two jobs, and the worktree-local `target/step25-lib` target; TMP/TEMP were under `target/test-tmp`. The fresh full root pass resolves the previous Step 25 aggregate-build gap for this review, not its missing integration/measurement gates. Compile attempts, fixture duration and node counts are not production latency or relevance measurements.

## Documentation impact

- Inspected: ADR 0039, canonical owner/API/storage/invariant/limitations/roadmap docs, frozen F8/H1 contracts, Steps 12/21–25 evidence and actual source/call sites.
- Updated: production disposition, corrected plan/evidence/index statuses, ADR/policy/API/current-state routing and experimental model source comments.
- Not affected: numeric policy, native formats, actual owner admission, live activation defaults, importer behavior and PHY implementation.
- Compliance check: both required documentation modes and patch hygiene passed after committing the review.
- Known documentation gaps: missing native coordination/recovery and retained-corpus/quality measurements are explicitly tracked, not presented as completed evidence.

## Architecture impact

- Standards added or changed: shared standards and accepted numeric policy unchanged; production/model authority distinction clarified.
- Ownership or boundary impact: existing Cva/FreshnessStore remains sole accepted score/receipt owner; no new runtime integration or mutable owner installed.
- Pitlord enforcement impact: refreshed existing six-rule policy passed; zero extracted relationships limits dependency claims.
- Other verification impact: full root/CLI regression passed without enabling historical/default semantic activation.
- Known architectural gaps: verified native H coordinator/recovery/importer/embedding operation, retained-corpus acceptance, universal delivered-use/Ego consumers, and approved PHY activity design remain open.

## Related docs

- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [Linear plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Frozen H1 policy](freshness-historical-manifest-policy.md)
- [Live F8 release](freshness-semantic-release-verification-2026-10-12.md)
- [Native checkpoint](freshness-historical-checkpoint-verification-2026-10-12.md)
- [H7 transformation matrix](freshness-historical-transform-verification-2026-10-12.md)
- [Step 24 evidence](freshness-historical-import-coordinator-verification-2026-10-12.md)
- [Step 25 evidence](freshness-historical-reconstruction-verification-2026-10-12.md)

## Notes

Static enforcement with zero extracted relationships is limited dependency evidence. Source inspection and behavioral fixtures support the reviewed existing boundaries; they do not certify unimplemented coordinator, native crash recovery, host consumption or production measurements. Labeled genuine-turn relevance/default activation and universal accepted-context/Ego consumer integration remain separate live-F8/core work.
