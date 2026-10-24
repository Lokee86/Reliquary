# Phase 5 runtime convergence execution

Date: 2026-10-23. Worktree: `Reliquary-phase5`; branch: `phase5-runtime-convergence`.

## Purpose

Execute the [45-task specification](dependency-convergence-phase5-runtime-orchestration-spec-2026-10-23.md), preserving existing runtime lifetimes and supported APIs.

## Overview

Implementation in progress. No final gate is accepted by this receipt. The original main worktree and its spec edit are preserved. Dedicated worktree starts at integrated Phase 4 commit `10cbe4b7`.

## B5 boundary and source baseline

The [Phase 4 execution receipt](dependency-convergence-phase4-execution-2026-10-23.md) and committed acceptance JSON establish integrated Project Environment scope APIs and passing Rust acceptance. The final prerequisite paragraph of the [handoff](dependency-convergence-phase3-phase4-handoff-2026-10-23.md) accepts bounded C-01–C-04 source-contract dispositions. Earlier text describing uncommitted Phase 4 is historical; it does not override the committed receipt. Complete symbol-level graph proof remains unavailable.

The [pinned baseline](../benchmarks/dependency-convergence-phase5-baseline-2026-10-23.json) reuses the accepted component map and cfg-aware source-import resolver directly, without an Arcana export. It reports 3,342 cross-component import rows and 96 uncertain imports. The strict import SCC contains 12 components including runtime and public composition, with no physical or owner-store component. Lower-to-runtime imports are exactly two approved immutable `InteractionRole` references in Archive stream records/codecs; no lower service imports Gateway/Instance implementation. Classification does not eliminate cycles.

The accepted deferred register assigns 167 exact tuples to Phase 5. Their source roles must be reviewed individually: immutable values and supported public adapters are distinct from private service callbacks. Initial Luna audits confirmed mounted worker provider calls run outside owner mutexes; deeper tuple review found mixed configuration/provider contracts, Insomnia extraction/evidence callbacks to Cva, and private Dream canonical reads through Cva. Those are the initial source cuts; aggregate module registries and graph policy have one owner (the parent agent).

## Contract and migration register

| Cut | Existing authority / final seam | Preservation tests |

| --- | --- | --- |

| P5.1.3 / P5.4.3 | Insomnia start/finish consume owner evidence; public offline composition supplies Archive/index/Container parts | Insomnia evidence, extraction, worker, backpressure and Gateway background suites |

| P5.1.4 / P5.4.4 | Dream canonical helper reads concrete owner stores; existing publication and runtime fences stay authoritative | Dream lifecycle/canonical/freshness/worker tests |

| P5.4.1–.2 | Configuration values/codecs/validation below provider routing; preserve root public exports and serialized tags | Switchboard, credentials/config and configured endpoint suites |

| G5 / Q5 | Gateway maps, instance generations, journal transitions and stream registries remain the single authorities | Existing instance/admission/conversation/host/snapshot/stream/reconnect suites |

| S5 | Current context evidence and accepted Phase 4 validated scope; post-I/O delivery reauthorization remains Gateway-owned | Context, compaction, project Git/Lore/revocation and external-source suites |

No new runtime, queue, worker registry, model call, compaction algorithm or format is introduced. No red build window is claimed accepted: coherent cuts compile at O5/E5 integration before broader verification.

## Durable transition owners

| Transition | Single controller / durable authority |
| --- | --- |
| Enqueue and cancel pending | Gateway conversation admission composes its owner-qualified multiplexing journal; cancellation is provisional until Archive finalization. |
| Dispatch / automatic branch | Conversation coordinator selects one native or negotiated host route, records stable operation identity, and preserves per-instance branch selection. |
| Unknown host outcome / restart | Existing host operation journal and reconciliation status; no blind native redispatch. |
| Finalize versus cancel | Existing conversation admission/journal plus idempotent Archive publication; the durable winner determines the receipt. |
| Owner mutation to delivery | Owner sequencer/publication gate commits first; Gateway independently reauthorizes the viewer at delivery. |
| Stream checkpoint / reconnect | Existing stream registry and durable checkpoint/cursor snapshot; no parallel registry or pre-checkpoint bytes. |

## Phase 6 supported public composition

Supported `Cva`/`Phylactery` APIs, `ConfiguredRuntime` offline methods, DreamProcessor/DreamCommunityNamer offline methods and root type exports remain available. New exact source tuples for split adapters are recorded in the existing deferred register with phase 6 and `C-06`, not added to the target DAG. The original Perception closure remains explicit. Source-role corrections do not mean those calls were eliminated. Non-runtime owner-read/value relationships inherited from the original 167-tuple inventory remain visible and are classified by actual use; they are not lower-to-Gateway/Instance callbacks.

## Task ledger

The denominator remains 45 tasks / eight gates. `pending` includes verification still required. B5 task records are pinned to the baseline above, not inferred from target DAG acyclicity.

| Task | State | Receipt / remaining work |

| --- | --- | --- |
| P5.0.1 | Verified (source) | Baseline receipt: `../benchmarks/dependency-convergence-phase5-baseline-2026-10-23.json`; source receipt `before_checkpoint`, `accepted_map_before`. |
| P5.0.2 | Verified (source) | Source receipt `tuple_reviews`, `source_proof_limits`; B5 boundary above. |
| P5.0.3 | Verified (source) | Contract and migration register above; source receipt `tuple_reviews`, `source_role_corrections`. |
| P5.1.1 | Verified (source) | No source cut: Gateway keyed maps and lease/writer lifetime; REL/PHY mount, detach and reopen tests in final Rust gate. `src/runtime_inference/runtime_host_gateway.rs`, `runtime_host_rel_execution.rs`, `runtime_host_phy_execution.rs`; source receipt `tuple_reviews`. |
| P5.1.2 | Verified (source) | Explicit DreamOwnerRead, EvidenceSearch and UserMemoryOwner ports; existing RelShared mutate/with_rel fences remain authoritative. `src/runtime_inference/runtime_host_rel_execution.rs`; source receipt `tuple_reviews`, `after`. |
| P5.1.3 | Verified (source) | Owner claim/result/config values split; extraction/evidence is facade-free; grouped publication moved unchanged to owner_lifecycle/insomnia_application.rs; prior import replay remains canonical. `src/runtime_inference/runtime_host_rel_insomnia.rs`; source receipt `tuple_reviews`, `source_role_corrections`. |
| P5.1.4 | Verified (source) | Dream canonical read port, pure provider evaluator, temporal policy and settlement evaluation separated from supported offline composition. `src/runtime_inference/runtime_host_rel_dream.rs`; source receipt `tuple_reviews`, `source_role_corrections`. |
| P5.1.5 | Verified (source) | No source cut: mounted Perception prepares/evaluates/commits with existing owner fences; exact public Cva extensions remain Phase6. `src/runtime_inference/runtime_host_rel_perception.rs`; source receipt `tuple_reviews`, `after`. |
| P5.1.6 | Verified (source) | No worker rewrite: vector batches freeze before embed and revalidate on commit; vector normalization is now a vector-owned immutable contract. `src/runtime_inference/runtime_host_rel_vectors.rs`; source receipt `tuple_reviews`, `after`. |
| P5.1.7 | Verified (source) | No source cut: shared PHY lease/worker lifetime remains Gateway-owned; Instance only has attachment/grants. `src/runtime_inference/runtime_host_phy_execution.rs`; source receipt `tuple_reviews`, `after`. |
| P5.1.8 | Verified (source) | No source cut: PHY Dream/Perception/Vectors worker loops use the keyed owner execution, not instance maps. PHY worker family sources named in spec; source receipt `tuple_reviews`, `after`. |
| P5.1.9 | Verified (source) | Old internal model_switchboard_codec removed; extraction tests use final contracts; unused canonical historical forwarding removed. Public offline methods retained. REL/PHY owner execution sources; source receipt `tuple_reviews`, `after`. |
| P5.2.1 | Verified (source) | `runtime_host_gateway.rs`, `runtime_host_gateway_mount.rs`, `runtime_host_gateway_admission.rs`; source receipt `tuple_reviews`, `after`. |
| P5.2.2 | Verified (source) | `runtime_host_instances.rs`, `runtime_host_gateway_conversation_selection.rs`; source receipt `tuple_reviews`, `after`. |
| P5.2.3 | Verified (source) | `runtime_host_gateway_semantic.rs`, `runtime_host_gateway_ambient.rs`; source receipt `tuple_reviews`, `after`. |
| P5.2.4 | Verified (source) | Gateway admission/instance authorization sources; source receipt `tuple_reviews`, `after`. |
| P5.2.5 | Verified (source) | Gateway publication/delivery sources; source receipt `tuple_reviews`, `after`. |
| P5.2.6 | Verified (source) | `runtime_host.rs` and Gateway sources; source receipt `source_role_corrections`, `after`, `source_proof_limits`. |
| P5.3.1 | Verified (source) | `multiplexing_journal.rs`, `runtime_host_gateway_conversation.rs`; source receipt `tuple_reviews`, `after`. |
| P5.3.2 | Verified (source) | Gateway conversation/finalization sources; source receipt `tuple_reviews`, `after`. |
| P5.3.3 | Verified (source) | Gateway conversation policy/branch sources; source receipt `tuple_reviews`, `after`. |
| P5.3.4 | Verified (source) | Gateway cancellation/journal/finalization sources; source receipt `tuple_reviews`, `after`. |
| P5.3.5 | Verified (source) | Gateway host/conversation generation sources; source receipt `tuple_reviews`, `after`. |
| P5.3.6 | Verified (source) | Gateway events/snapshots sources; source receipt `tuple_reviews`, `after`. |
| P5.3.7 | Verified (source) | Gateway streams/registry and `interaction_live.rs`; source receipt `tuple_reviews`, `after`. |
| P5.3.8 | Verified (source) | Gateway reconnect/snapshot reader sources; source receipt `tuple_reviews`, `after`. |
| P5.4.1 | Verified (source) | model_config/model_config_codec/retrieval_config/runtime_config are below switchboard routing/auth; validate_runtime no longer constructs a service. `configured_runtime.rs`, `configured_runtime_decision.rs`, `model_switchboard.rs`; source receipt `tuple_reviews`, `after`. |
| P5.4.2 | Verified (source) | VectorNormalization moved with unchanged 0/1 tags; Insomnia completion codec remains lifecycle-owned; process/worker values separated from facade implementation. Configuration/codec moves: source receipt `source_role_corrections`, `tuple_reviews`; contract register above. |
| P5.4.3 | Verified (source) | Insomnia stages resolve explicit evidence, release owner mutex for model I/O, then reacquire owner admission and canonical lifecycle publication. Insomnia runtime/processor/worker sources; source receipt `source_role_corrections`, `tuple_reviews`. |
| P5.4.4 | Verified (source) | Pure DreamProcessor evaluation has no Cva/PHY handle; existing snapshot/revision/graph checks and final publication order preserved. Dream runtime, `dream_settlement.rs`, `dream_temporal_policy.rs`; source receipt `source_role_corrections`, `tuple_reviews`. |
| P5.4.5 | Verified (source) | No source cut: Perception endpoint calls are outside prepare/commit owner scopes. Perception runtime consumer sources; source receipt `tuple_reviews`, `after`. |
| P5.4.6 | Verified (source) | No source cut: embeddings follow prepare→embed→fresh lease/profile/owner validation; no provider or store moved into ConfiguredRuntime. Vectors runtime consumer sources; source receipt `tuple_reviews`, `after`. |
| P5.4.7 | Verified (source) | Existing root exports and public offline methods preserved; offline Cva adapters are explicitly distinct from provider/domain services. Supported offline adapters: source receipt `source_role_corrections`; Phase 6 register above. |
| P5.5.1 | Verified (source) | Context Engine runtime input consumer sources; source receipt `tuple_reviews`, `after`. |
| P5.5.2 | Verified (source) | Compaction/checkpoint consumer sources; source receipt `tuple_reviews`, `after`. |
| P5.5.3 | Verified (source) | Project historical-read consumers; source receipt `tuple_reviews`, `after`; Phase 4 baseline above. |
| P5.5.4 | Verified (source) | `runtime_host_gateway_project_lifecycle.rs`; source receipt `tuple_reviews`; Phase 4 baseline above. |
| P5.5.5 | Verified (source) | `runtime_host_gateway_external.rs`; source receipt `tuple_reviews`, `after`. |
| P5.6.1 | Verified (source) | Deleted old codec forwarding; split canonical source owners once; no shadow execution, queue, compatibility adapter or old-owner test copy. Source receipt `tuple_reviews`, `source_role_corrections`, `after`. |
| P5.6.2 | Verified (source) | Current-source mode reuses accepted map/resolver and retained exact before-image; source-role changes are reported separately from code/edge cuts. Source receipt `after`: 822 mapped production files, zero unmapped paths and zero unclassified new edges. |
| P5.6.3 | Verified (source) | Focused source negatives reject facade handles at cut seams and lower Gateway/Instance imports; exact import graph retains Phase6 SCC witnesses and uncertainty. Source receipt `after.cycle_witnesses`, `source_proof_limits`; remaining facade SCC/debt stays explicit. |
| P5.6.4 | Verification in progress | Consolidated bounded subsystem test/source-policy gate remains; no acceptance claimed. |
| P5.7.1 | Verification in progress | Final version-pinned integration verification remains; no acceptance claimed. |
| P5.7.2 | Pending final closure | Canonical documentation and `ddocs` policy/link pass await P5.7.1. |
| P5.7.3 | Pending final closure | Worktree/commit disposition and Phase 6 handoff await P5.7.1–.2. |

## Related docs

- [Controlling specification](dependency-convergence-phase5-runtime-orchestration-spec-2026-10-23.md)

- [Architecture](architecture.md)

- [Runtime multiplexing](subsystems/runtime-multiplexing.md)

## Notes

B5 accepts only the inherited bounded map/source-contract baseline. D5/A5 require changed-edge enforcement and behavioral/documentation evidence; unavailable symbol-level proof must remain explicit.

## Documentation impact

- Inspected: required architecture/invariant/storage/maintainer/limitations/roadmap documents, controlling spec and prior receipts.
- Updated: architecture/API/model-runtime/runtime/context/configuration ownership, preservation contracts, current limitations, maintainer/coverage/roadmap/index and this execution/source receipt.
- Not affected: persistent schema, public signatures, queue/host/stream semantics, Project Environment authority and deferred Context Engine features.
- Compliance check: final documentation policy, changed-from and Archivist checks pending.
- Known documentation gaps: installed Pitlord docs command availability must be reported, not inferred from source guards.

## Architecture impact

- Standards added or changed: none; existing target DAG remains unchanged.
- Ownership or boundary impact: lower configuration/owner evidence/value contracts; one lifecycle-owned grouped publisher; pure Dream evaluation separated from supported public composition.
- Pitlord enforcement impact: focused runtime source guard is part of the existing architecture entrypoint; accepted component checker adds current-source-only incremental evidence.
- Other verification impact: one serial Rust/CLI acceptance run, source/physical/owner/history negatives, incremental imports and required architecture refresh.
- Known architectural gaps: inherited symbol-level graph completeness and explicit retained source debt remain disclosed; source-role correction is not edge elimination.
