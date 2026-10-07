# P5-B capability and worker ledger
Parent: [P5-B implementation plan](multiplexing-phase5b-background-host-2026-10-06.md)

## Purpose
Record the source-backed worker migration, executable capability boundaries, and convergence handoffs for lane B.

## Overview
Base: `c24b06273abbab383eb74780bf82cb5c0ceef0b5`. Lane: `feat/multiplexing-p5-background`, independent worktree `Reliquary-p5b-background`. Main and A's worktree are preserved. This ledger is an implementation record for B; it does not establish P5/P6 or Warlock acceptance.

## Worker ownership

| Legacy responsibility | Canonical owner | Receipt and recovery | Wake and lifecycle |
| --- | --- | --- | --- |
| Insomnia extraction, evidence, metadata, ownership and Chronos | One RelRuntime; configured Insomnia worker count per REL | Existing Insomnia lease/operational state; frozen exact-principal EpisodePlan in REL owner journal; verified REL receipt; observed marker | REL signal; owner fence; stop/join before reopen |
| Personal publication from REL extraction | PublicationCoordinator; exact mounted PhyRuntime destination | Frozen principal/PHY binding; private publication key; offline/revoked/quiesced destination Held; no retarget; receipt verification before observation | REL retry and exact PHY wake; no nested REL/PHY store locks |
| Dream project work | RelRuntime Dream worker | Owner-local Memory/Graph revisions and Dream epoch/cooldown; stale evaluations discarded | REL signal and timed retry; owner-local fence |
| Dream personal work | PhyRuntime Dream worker | PHY-only Memory/Graph revisions and Dream epoch/cooldown | PHY signal; timed retry; owner-local fence |
| Perception project work | RelRuntime Perception worker | Derived deduplicated mention queue; startup scan of durable Dream-processed Memories; owner revision checks | REL signal; requeue fenced work; stop/join and startup reconstruction |
| Perception personal work | PhyRuntime Perception worker | Derived PHY-only mention queue; durable startup scan and staged reconciliation | PHY signal; requeue fenced work; seed before start |
| Memory embedding | Separate REL and PHY vector workers | Existing compatibility profile, MemoryBodyId binding and prepared-batch validation; missing vectors reconstruct work | Exact owner signal; no inference under owner mutex |
| Archive embedding | Existing Archive runtime vector generation operation | Archive vector generation and Fragment bindings remain canonical | No new background queue or worker introduced |

## Executable capability matrix

| Capability | Executable path | Authority | External payload/result boundary |
| --- | --- | --- | --- |
| ConversationExecution | Existing gateway conversation coordinator and REL operation journal | Host only for complete negotiated/reconciled contract; otherwise Native or Held as existing recovery requires | Existing immutable HostOperation, digest, durable HandoffReceipt; exact status/cancel; coordinator owns journal and Archive receipt |
| Insomnia | Existing GeneralEndpoint extraction stages in RelRuntime | Native | GeneralEndpoint::complete_json; native extractor/application/frozen personal publication; no host result/receipt path exists |
| Dream | Existing GeneralEndpoint pair classifier/verifier in each owner worker | Native | Native Dream candidates/evaluation/revision-checked publication; no typed host result path exists |
| Embedding | Existing EmbeddingEndpoint in each owner worker and authorized query callers | Native | Existing dimensions/normalization/embed; profile probe and vectors; no durable host result path exists |
| ProjectEnvironment | Existing owner-qualified gateway project operations | Native | Existing project adapter/filesystem/Git contracts; no host operation result/receipt path exists |

Advertising an enum member or full queue guarantees does not make the last four stages executable host integrations. They retain Native authority, including when an unrelated configured host connection fails. Recovered ownership debt is still Held until exact operation terminal proof and reconciliation release it; it cannot be erased by this filter. A missing native endpoint leaves its native work pending; it is not evidence that Host execution is supported.

## Cancellation and lock boundaries
The existing REL journal remains the only durable native pending authority. Host intent is persisted before handoff; stable identity and immutable digest bind every retry. Unknown/lost acknowledgement is Held; status/cancel remain queryable during downgrade. Pending cancellation and host dequeue serialize at the actual adapter. Running cancellation requests cooperative interruption and cannot promise rollback.

Native model calls are synchronous endpoint calls without a per-call abort contract. Quiesce closes the owner fence, drains bounded commit leases and joins workers; an in-flight call may delay the join but may not publish through the closed fence. Reopen follows stop/join/reconciliation and invalidates old access tokens. Processing switches stop new work, not roll back an already admitted commit.

## Consumer handoff

| Consumer need | Method | Grant / owner | Durability / visibility |
| --- | --- | --- | --- |
| Replace inference routes | GatewayRuntime::set_routes | Trusted gateway configuration; one canonical route owner | Ephemeral; wakes each mounted owner, never changes instance selection |
| Exact REL background counters | GatewayRuntime::background_status_for | ViewRel on explicit REL | Read-only; no PHY fields or data; attachment/grant revalidation |
| Exact private counters | GatewayRuntime::phy_background_status_for | UseOwnPhy on explicit attached PHY | Read-only; no REL feed |
| Wake exact REL / PHY | wake_rel_background_for / wake_phy_background_for | ManageRel / UseOwnPhy | Ephemeral; exact owner only |
| Enable background stages | set_rel_background_enabled_for / set_phy_background_enabled_for | ManageRel / UseOwnPhy | Ephemeral; owner-local switch; REL Insomnia has its existing separate switch |
| Worker diagnostics | background_worker_state_for | ViewRel / UseOwnPhy on explicit owner | Live worker count plus background enabled flag; read-only; no provider I/O |

No new public type exports are required for these inherent methods. Root export changes remain D's lane. Existing `set_rel_processing_enabled_for` controls Insomnia; the background switch controls the remaining three REL workers. PHY has no Insomnia.

## Deletion manifest for D
Delete old-only worker authority after C's real callers migrate: `runtime_host_owner_execution.rs`, `runtime_host_insomnia.rs`, `runtime_host_dream.rs`, `runtime_host_dream_owner.rs`, `runtime_host_vectors.rs`, `runtime_host_perception.rs`, `runtime_host_perception_api.rs`, `runtime_host_perception_flow.rs`, `runtime_host_perception_owner.rs`, `runtime_host_perception_queue.rs`, `runtime_host_perception_reconciliation.rs`, `runtime_host_lifecycle.rs`. Retire or migrate old-only characterization in runtime_host_perception_tests.rs, runtime_host_tests.rs and runtime_host_vector_tests.rs; runtime_host_perception_test_support.rs currently serves only the old host/perception tests. Retain or split runtime_host_test_support.rs: canonical gateway tests (including B) still use its test_path helper. Run the same reference inventory before removing module registrations. Split legacy host implementation from retained data definitions in `runtime_host_status.rs`; `RuntimeBackgroundStatus` remains an existing public data contract. Retain `runtime_host_routes.rs` as the one finite endpoint configuration type, moving its module registration only during D's root convergence. Retain endpoint traits, worker algorithms, and canonical REL/PHY/publication modules. D must run reverse source inventory before deletion; this is a responsibility manifest, not permission to remove shared helpers blindly.

## Related docs
- [Parallel plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [P5-B plan](multiplexing-phase5b-background-host-2026-10-06.md)

## Notes
Actual verification is recorded in [lane acceptance](multiplexing-p5b-acceptance-2026-10-07.md). Fake adapters prove gateway mechanics, not real Warlock provider conformance.
