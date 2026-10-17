# Freshness retrieval boundary — execution record

Date: 2026-10-15
Parent plan: [implementation specification](freshness-internal-retrieval-boundary-implementation-plan-2026-10-15.md)
Status: F1, F2, F4, and F5 evidence remains valid. F3's actual receiver/replay seam is implemented and tested in the Warlock `cf41c66` integration candidate, not deployed production. The Reliquary/host receiver evidence is recorded in [host handoff seam](agentic-memory-host-handoff-seam-2026-10-16.md); frozen-corpus and final disposition gates remain open.

## F2 — explicit passive-read contract

Existing primitives already satisfy the implementation boundary; no duplicate reader, scoring engine, flag or propagation was needed. Rust API documentation now explicitly marks REL/PHY Memory reads, lexical search and Freshness scores as passive, and distinguishes authorized gateway delivery from consumption. Receipt/API documentation assigns attestation to the trusted embedding host and disclaims authentication of arbitrary client claims. Existing return values, errors and numeric/persistence behavior are unchanged.

## Subsequent retrieval design (status clarification)

The later accepted [agentic Memory retrieval design](agentic-memory-retrieval-design-2026-10-15.md) uses successful host tool-result handoff and a single turn-wide `SelectedIDs` set. This document remains the historical F1/F2/F4/F5 **execution evidence**; its original stricter accepted-context/outbox assumptions are superseded where inconsistent. The current F3 receiver/replay seam passes focused Warlock host tests in candidate `cf41c66`, but final frozen-corpus/release verification remains outstanding.

## F3 — external integration disposition

The 2026-10-15 F1 finding remains an accurate historical baseline: no production caller was then identified. The later Warlock `cf41c66` integration candidate supplies receiver-owned acknowledgment and exact-manifest restart replay, with eight focused host Memory tests passing. It is not a deployed release; frozen-corpus and final production-disposition gates remain open.

## Purpose

Record caller classification, the explicit passive API contract, test evidence and the external host acceptance blocker.

## Overview

Existing production reads are passive. The implementation documents and regression-protects that contract; numeric policy and storage mechanics are unchanged. The external host must still supply confirmed accepted-content events.

## Notes

F4 was assigned to GPT-5.6 Luna; the primary agent reviewed and corrected its assertions and added omitted-candidate and PHY coverage. Initial compile attempts hit the delegated command timeout. The first collected test attempt exposed fixture expectations that confused durable anchors with projected decay; the corrected tests pass.

## Related docs

- [Architecture](architecture.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Host consumption contract](freshness-external-consumption-contract.md)

## F1 — caller and ownership map

The production `src/` and `cli/src/` trace finds one access-principal event producer: `Cva::record_accepted_memory_use` in `src/runtime_inference/freshness_access.rs`. Its production callers are absent; all actual invocations are tests. `src/lib.rs` exports the receipt and API to trusted embedding hosts. This in-process API is not an authenticated client endpoint and cannot verify that a host tells the truth about acceptance.

| Caller / concrete path | Read path / classification | Before | Required after |
| --- | --- | --- | --- |
| `facade/cva.rs::memory`, `memory_revision`, `facade/phylactery.rs::memory` | `Memories::memory` / content and revision inspection | No access receipt | Same |
| `retrieval/memory_search.rs::search_memories` (REL and PHY) | lexical candidates then `memory`; candidate delivery | No access receipt | Same |
| `retrieval/memory_retrieval_build.rs`, `retrieval/lexical_index/memory.rs` | storage reader; derived retrieval/index construction | No access receipt | Same |
| `runtime_inference/freshness_access.rs::ego_memory_freshness`, `select_ego_routine_context` | score/history evaluation and Ego preparation; internal processing | No access receipt | Same; selection policy deferred |
| `runtime_inference/dream_owner_candidates.rs`, `dream_temporal.rs`, `dream_community_naming.rs`, `dream_cooldown.rs`, `dream_freshness.rs`; `facade/phylactery_dream_candidates.rs` | content, temporal, lifecycle and graph inspection; internal processing | No access receipt | Same |
| `runtime_inference/insomnia/processor/application.rs` | existing-memory hydration around explicit publication; internal processing | Reads do not reinforce; publication has independent birth/admission semantics | Same |
| `perception/entity_candidates.rs`, `entity_principal.rs`, `entity_resolution_prepare.rs`, `entity_resolution_evidence.rs`, `entity_reconciliation_prepare.rs`, `entity_reconciliation_owner.rs` | owner-local evidence inspection; internal processing | No access receipt | Same |
| `runtime_inference/runtime_host_rel_dream.rs`, `runtime_host_phy_dream.rs`, `runtime_host_rel_perception.rs`, `runtime_host_phy_perception.rs`, `runtime_host_gateway_background.rs` | worker scheduling/candidate hydration; internal processing | No access receipt | Same |
| `runtime_inference/runtime_host_gateway_ambient.rs`, `runtime_host_gateway_ambient_extended.rs` | authorized Memory reads/retrieval; external candidate delivery | No access receipt | Same |
| `runtime_inference/runtime_host_gateway_semantic.rs`, `runtime_host_gateway_semantic_search.rs`, `runtime_host_gateway_semantic_provenance.rs` | authorized content/provenance and REL/PHY visible-search hydration; external candidate delivery | No access receipt | Same |
| `runtime_inference/workspace_management_knowledge.rs` | inventory, content, provenance and relation inspection through gateway; candidate/metadata delivery | No access receipt | Same |
| `runtime_inference/runtime_host_gateway_external.rs`, `external_source/publication.rs` | external-source Memory hydration; candidate delivery/internal publication | No access receipt | Same |
| `facade/cva_freshness.rs`, `runtime_inference/dream_lifecycle.rs`, `dream_canonical.rs`, `facade/phylactery_dream_lifecycle.rs`, `phylactery_dream_canonical.rs`, `runtime_host_gateway_semantic_mutation.rs`, `facade/phylactery_memory_source.rs`, reconciliation helpers | reads before explicit mutation | Reads passive; explicit mutation may have its own effects | Same |
| `record_accepted_memory_use` | trusted host accepted-content receipt, explicit mutation | Validates owner/cut/identity, syncs effect, applies existing propagation | Same |

Paths in the table are relative to `src/`. Graph/provenance readers do not call the accepted-use API; the only production `policy.access_principal` propagation construction is in `freshness_access.rs`. Policy validation and persisted-event verification also reference that constant without producing access events.

### Acceptance boundary finding

`runtime_host_gateway_ambient.rs::deliver_rel` only revalidates the instance/token and returns a value. `with_instance_delivery` similarly holds authorization leases. `runtime_host_gateway_semantic_search.rs::search_visible_memories` embeds, retrieves, hydrates, truncates, revalidates grants and returns lanes. None receives an acknowledgement that exact Memory content entered a consumer context. Their success is candidate delivery, not consumption. Conversation admission/finalization and stream/resync acknowledgements concern turns or stream positions, not accepted Memory IDs. There is no universal accepted-context producer in this repository (also recorded in `current-limitations.md`).

**F1 gate:** no trustworthy host-owned acceptance event exists here. Do not wire reinforcement into search, authorization delivery, or conversation admission. F3 must remain pending for the external host; document the contract and continue independent F4/F5 validation.

## F4 — regression coverage

`tests/freshness_retrieval_boundary.rs` adds three real-owner tests:

- Passive REL reads compose content/revision fetch, lexical search, real source provenance/Episode hydration used by Insomnia, graph neighbors/path traversal, actual Dream context/temporal reads, Ego selection, historical/current score projections, and invalid reads. Full file bytes, all affected records and REL/global/Archive/Memory/Graph clocks remain unchanged through reopen.
- An accepted exact source set with duplicate IDs creates one durable batch and existing propagation; a retrieved but omitted, disconnected candidate remains unchanged. Identical retries append nothing, including after reopen. Invalid owner, future turn, changed identity payload/cut, empty identity, missing Memory and empty-source cases create no phantom effects.
- PHY content, lexical and Dream reads preserve bytes and clocks through reopen without introducing a PHY activity source.

The tests use real Cva persistence and event recording, not private mocks. Episode hydration is representative Insomnia read coverage, not certification of every inference/publishing branch. No host acceptance/cancellation/outbox is fabricated: end-to-end delivery failure/retry proof remains part of pending F3. Existing owner/R2/recovery/oracle suites protect deeper engine recovery and propagation behavior.

## F5 — verification and closeout

| Check | Collected result |
| --- | --- |
| `cargo fmt --check` | Passed after final Rust changes |
| `cargo check -j 2` | Passed |
| Targeted boundary tests | 3 passed |
| `cargo test -j 2` | Passed: 1,132 library tests, 2 ignored; 141 integration tests across 23 targets; 0 doc tests; zero failures |
| Documentation structural and `--changed-from origin/main` checks | Only four findings remain, all missing sections (Purpose, Overview, Related docs, Notes) in the pre-existing dirty `ego-status-decision-inventory-2026-10-14.md`; task-owned documents are clear |
| Architecture source guards | 75 P4 fixtures passed, 161 public APIs classified; 11 external-ownership fixtures and production boundary check passed |
| `python scripts/check_architecture.py --refresh` | Timed out after 240 seconds during Lexicon Rust analysis, before Arcana/Pitlord enforcement; no refreshed enforcement pass claimed |

The behavior matrix is unchanged: all classified reads produce zero access reinforcement; only the existing explicit accepted-use API writes it. No runtime algorithm, public signature, numeric policy, propagation, storage format or persistence mechanics changed. The detachable CLI is unaffected; no CLI-specific change gate was triggered.

F1/F2/F4 are complete. F5 verification is recorded with the two exceptions above; it is not an all-gates-green claim. F3 receiver/replay wiring is implemented in Warlock candidate `cf41c66`; deployment and frozen-corpus verification remain pending. The separate Ego audit-selection refactor was not performed. Both pre-existing dirty Ego documents remain outside this change and must not be staged with it. The exact implementation commit is reported in the completion response.

### Documentation impact

- Inspected: required architecture, invariants, storage format, maintainer map, limitations and roadmap; shared engineering guidance and repository documentation policy.
- Updated: API comments, architecture, invariant 127, API reference, maintainer map, behavioral contracts, development, limitations, documentation index, execution plan, host contract and this verification record.
- Not affected: storage-format authority and the separate Ego design/status edits.
- Compliance check: both documentation checks leave only the four unrelated Ego-status findings listed above.
- Known documentation gaps: those pre-existing structural findings remain; missing external integration is explicitly documented as pending.

### Architecture impact

- Standards added or changed: no shared standard or repository policy change.
- Ownership or boundary impact: existing readers remain passive; Freshness retains the sole receipt/effect writer; the trusted external host must own proof of accepted context.
- Pitlord enforcement impact: policy unchanged; source guards passed; refreshed graph enforcement unavailable because analysis timed out.
- Other verification impact: complete Rust formatting/check/test evidence above; no new engine or storage implementation.
- Known architectural gaps: authoritative external accepted-context producer/durable receipt handoff remains absent (F3); no fresh graph enforcement result.
