# Runtime and workflow source disposition crosswalk — 2026-11-03

Parent index: [Documentation index](INDEX.md)

**Status:** Stage 3 source-grounded design crosswalk; source baseline `ea18a13a`, `refactor/crate-first-restart`. This is a *family-level* destination map, not a claim that the listed files are individually extractable or that new Rust interfaces have compiled.

## Purpose

Prevent another partial Gateway extraction from being mistaken for completed runtime architecture. Every major Root execution and orchestration family must have a canonical future owner or a narrowly stated reason to remain in a public-product integration layer.

## Overview

The runtime implementation is primarily under `src/runtime_inference/` and includes actual Gateway, REL/PHY workers, journals, host integration, conversation scheduling, streaming, source adapters and semantic workflows. The `reliquary-runtime-gateway` crate exports no working implementation. Stage 3 assigns each responsibility below; Stage 4 will map the final Rust package/physical DAG and Stage 6 will enumerate exact functions/call sites and move-once slices.

## 1. Source-to-authority map

| Current source family | Present behavior | Target owner / mandatory seam |
| --- | --- | --- |
| `runtime_host_gateway.rs` | `GatewayRuntime`, mounted owner/Instance registries, admission leases, epoch, shutdown, host routes | **Canonical Gateway runtime.** Replace Root `InteractionRuntime`/`Cva` dependencies with Stage 1 typed session and explicit Stage 2 workflows, not a second registry |
| `runtime_host_instances.rs` | `InstanceRuntime`, grants, attachment incarnation, principal/PHY, managed selections | **Instance component inside Gateway.** Public observation without direct grant mutations; Gateway-only controller authority after crate cut |
| `runtime_host_gateway_admission.rs`, `runtime_host_gateway_mount.rs`, `runtime_host_gateway_phy_lifecycle.rs`, `runtime_host_gateway_project_lifecycle.rs` | Trusted host authentication, physical mount claims, owner topology/lifecycle, selection | **Gateway owner admission and host trust boundary**. No implicit cross-PHY authority, independent physical lease enforcement |
| `runtime_host_rel_execution.rs`, `runtime_host_phy_execution.rs` | One owner-local worker set; REL holds `Arc<Mutex<InteractionRuntime>>`, PHY holds `Arc<Mutex<Phylactery>>` | **Gateway owner execution** with one `RelSession`/`PhySession` handle and separate runtime-only scheduling state; no copy of mutable owner stores |
| `runtime_host_rel_insomnia.rs`, `runtime_host_rel_dream.rs`, `runtime_host_rel_perception.rs`, `runtime_host_rel_vectors.rs`, PHY counterparts | Bounded work dispatch and source/evaluation orchestration | **Worker runtime** handles lifetime/wake/leases; **named semantic workflows** own evidence/evaluation/admission; no model call under owner mutex |
| `runtime_host_gateway_conversation.rs`, `runtime_host_gateway_conversation_*.rs` | `ConversationRegistry`, submission/cancel, per-branch scheduling, finalization, Archive receipts | **Gateway conversation coordinator** + **native conversation finalization workflow**; keep one journal and one writer-admission authority |
| `multiplexing_journal.rs`, `multiplexing_journal_tests.rs` | REL-qualified durable provisional queue, host phase and generation/checkpoint state, `RQMUXJ01/S01/H01` | **Gateway operational journal**, exact format/replay; separate from Archive semantic history and Insomnia work store |
| `runtime_host_owner_operation_journal.rs`, `runtime_host_owner_publication*.rs` | `RQOP0001` source attribution/episode plan/result/held receipt, publication/host effect coordination | **Gateway operational owner-effect journal** + focused publication workflow, with exact idempotent owner-commit receipts; never create parallel semantic Memory authority |
| `runtime_host_gateway_host.rs`, `runtime_host_gateway_host_*.rs` | Host capabilities/queue guarantees, `CapabilityRouter`, contract, status, handoff/cancel/reconcile | **Gateway host protocol boundary**, canonical single handoff state; Switchboard separately owns model transports |
| `runtime_host_gateway_streams.rs`, `runtime_host_gateway_stream_*.rs`, `runtime_host_gateway_convergence*.rs` | Operation/branch-scoped generation streams, sequenced snapshot+watermark, committed deltas and resync | **Gateway distribution/stream subsystem**. Publish only durably committed checkpoints, bound to mount and grant generations |
| `runtime_host_gateway_observation*.rs`, `runtime_host_gateway_events*.rs`, `runtime_host_gateway_mutation_events*.rs` | Current observations, invalidation notifications, sequenced owner changes | **Gateway readonly projection and distribution**; facts derived from canonical REL/PHY owner, not a second semantic event store |
| `interaction_live.rs`, `interaction_live_resume.rs`, `interaction_stream*.rs` | `LiveSession`/adapter, viewer cursors, checkpoint projections and evidence | **Host-facing view/recipient adapter** over Gateway; Archive/operational journals are authority; no duplicate worker queue |
| `interaction_runtime.rs`, `interaction_session.rs`, `interaction_background.rs` | `InteractionRuntime` owns `cva: Cva`, session map, turn acceptance and Episode scheduling | **Split:** owner state to `RelSession`; runtime-only session selection to Gateway; accepted turn/Episode admission to **conversation ingest workflow** |
| `src/runtime_inference/insomnia/**` | Extractor adapters, proof hydration, grouped output, work lease and worker runtime | **Split:** Insomnia semantic algorithm remains service; owner proof/commit workflow; worker in Gateway; durable work in `InsomniaStore` |
| `src/runtime_inference/dream_*.rs` | Dream evaluation, source hydration, Graph/Memory lifecycle publication, first pass | **Dream evaluator and named Dream publication workflow**. Journal/Graph/Freshness durability stays owner-local |
| `src/perception/**`, `runtime_host_*perception*.rs` | Entity/mention resolution, scheduling, Graph association and owner commits | **Perception workflow** and owner-scoped worker; future Observations add one typed workflow rather than an owner bag |
| `agentic_memory_retrieval*.rs`, `runtime_host_gateway_agentic_memory*.rs` | Request controller, selection decision, hydration, confirmed host handoff | **Agentic selection controller**, **Gateway delivery** and **separate Freshness accepted-use workflow**; no score on internal read |
| `configured_runtime*.rs`, `model_switchboard*.rs` | Capabilities/routes and provider endpoint construction | **Switchboard/host composition**; typed service endpoint injection. Provider routing is not Gateway durable owner state |
| `src/retrieval/**`, `runtime_host_gateway_semantic*.rs` | Scoped candidate collection, ranking, source/PHY hydration | **Retrieval read workflow** plus pure service ranking; authorization at Gateway and before hydration |
| `runtime_host_gateway_external*.rs`, `src/external_source/**` | Host-owned reference-only source hydration/publication | **External source integration** plus Gateway auth/host adapter; cannot fake Archive-native source history |
| `workspace_management*.rs`, `src/project_environment*` | Owner management, project source correlation, Git/Lore file/revision operations | **Gateway authorized management workflow** plus independent Project Environment; physical VCS remains its own authority |
| `src/imports/**`, `src/facade/historical_*.rs`, `src/rewrite/**` | Import/reconciliation/history/rewrite and bounded source/provider operations | **Exclusive offline application workflows** over Stage 1 owner reads/commits; history pure proof, no inference on ordinary replay |
| `crates/reliquary-runtime-gateway/src/lib.rs` | Placeholder only | **Target physical integration candidate for Stage 4**; not complete until all real Gateway execution authority is physically owned and Root no longer imports its internals |

**Negative acceptance:** No generic `RootRuntime`, all-service registry, arbitrary closure over mutable REL stores, duplicate `InstanceRuntime` grant cache, second `RelJournal` queue, or callback into Root higher service from lower Gateway module.

## 2. Concrete design/verification seams

| Existing code witness | Stage 3 decision | Later acceptance test |
| --- | --- | --- |
| `GatewayRuntime::authorize` / `with_rel` | Owner token is scoped to attachment, grant generation, mount epoch, fence; short owner access only | Revocation during model work, stale mount, wrong PHY grant, no provider call within owner lock |
| `InstanceRuntime::replace_grants` etc. | Grant/select/detach remains Gateway-private through a real controller capability | External consumer holding `Arc<InstanceRuntime>` cannot mutate grants; foreign controller rejected |
| `RelJournal::open` and `submit_conversation` | Durable provisional queue admitted before user ack | Restart after enqueue, cancel/dequeue race, no premature Archive turn/activity |
| `process_reserved_conversation_submission` | FinalizeWon then Archive and post-commit receipt reconciliation | Crash before/after Archive commit, same submission ID, no duplicate history |
| `GatewayHostAdapter::handoff` / `CapabilityRouter` | Stable ID/digest, host queue guarantee negotiated separately per capability | Unknown/transport error stays Held, host cancel after dequeue, reconnect status reconciliation |
| `RelShared::mutate`, `PhyRuntime` | One shared per-owner worker control with bounded short owner commits | Multiple users/Instances, worker cancellation, no cross-owner deadlock or leaked PHY state |
| `join_stream_bound`, `with_conversation_distribution` | Snapshot+watermark, epoch-bound stream cursor, synced prefix visibility | Late join, gap, revocation, remount, slow subscriber, crash before sync |
| `InteractionRuntime::accept_turn` | Typed REL Archive acceptance and source receipt, session scheduling independent | Exactly one accepted activity tick/Archive turn, Episode scheduling after durable sync |
| Insomnia REL→PHY workflow | Durable idempotent destination-first routing with separate source completion | Crash between files, exact deterministic retry, authorization loss |
| Dream first cycle + Freshness | Owner-local first pass and accepted event proof | No reinforcement on duplicate rewire, zero-link admission, retry/no-op recovery |
| Future Observations/Ego | New worker strategy + typed workflow, not new Gateway mutable owner fields | Compile-level extension experiment; PHY/REL authorized projections; no new global scope |

## 3. Next-stage boundary

**Stage 4** selects physical crate/package seams and the actual acyclic dependency DAG, including whether `GatewayRuntime` and workflow coordinators are in one library, neighboring libraries or a small public product composition package. **Stage 6** enumerates every remaining function, type, test and caller and requires compiler/behavior proof. This document settles responsibility but does not pretend to settle compile-time type paths.

## Related docs

- [Stage 3 runtime design](runtime-orchestration-architecture-2026-11-03.md)
- [ADR 0046](decisions/0046-gateway-runtime-and-named-workflow-ownership.md)
- [Stage 1 storage](rel-phy-storage-architecture-2026-11-03.md)
- [Stage 2 services](semantic-service-architecture-2026-11-03.md)
- [Current implementation inventory](current-system-inventory-2026-11-03.md)
- [Old partial Gateway extraction spec](runtime-gateway-service-crate-extraction-spec-2026-10-30.md)

## Notes

This crosswalk is grounded in current Root source and the existing Gateway operational contracts. It is not a new code implementation, a complete Rust method-level migration or host integration release acceptance.
