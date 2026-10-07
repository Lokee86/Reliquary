# Multiplexing P5/P6 — parallel execution and convergence plan

Parent: [Multiplexing implementation plan](multiplexing-implementation-plan.md)  
Baseline: [Accepted P3/P4 main integration](multiplexing-p34-main-integration-2026-10-06.md)  
Authority: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)  
Consumer evidence: [Phase 0B G6](multiplexing-phase0b-consumer-verification.md)  
Status: **Execution-ready plan, no P5/P6 implementation claimed.**

## Purpose

Replace the remaining legacy host consumers with the established GatewayRuntime/InstanceRuntime authority in parallel, then remove the obsolete host in one controlled hard cut. Build release/stress verification concurrently instead of waiting for migration completion. Ownership extraction is the objective, not a redesign of semantic stores, inference, or transport.

## Overview

Planning baseline is clean Reliquary main e1c8f88 (2026-10-06). P3/P4 are integrated: conversation/journal/branch/generation, exact-REL/PHY ownership, authorized REL feed, conversation streams, and reconnect. The P3/P4 integration record reports 1,085 library, 128 integration, 19 CLI tests and serial all-target compilation passing; architecture indexing timed out, not green. These are **historical** acceptance numbers, not rerun results.

Old ReliquaryRuntimeHost with ambient active REL/PHY, mixed owner worker state, legacy route/session helpers and public methods remains in the library. Warlock still embeds it, pins old Reliquary/Arcana revisions, and is not yet a GatewayRuntime consumer. Its present main checkout has unrelated dirty UI/graph work; do not overwrite it.

## Parallel branches and handoffs

| Track | Source ownership | Independent scope | Blocked item |
| --- | --- | --- | --- |
| [P5-A REL/runtime consumers](multiplexing-phase5a-rel-consumers-2026-10-06.md) | gateway ambient, semantic, retrieval, project, owner-qualified tests | exact REL/PHY operations, Archive/Memory/Entity/Graph/repository/status | common public exports/root wiring |
| [P5-B background and host capabilities](multiplexing-phase5b-background-host-2026-10-06.md) | REL/PHY runtime workers, configured routes, gateway host adapter/worker tests | Dream/Insomnia/Perception/vector worker parity, capability/recovery/cancellation | common public exports/root wiring |
| [P5-C Warlock and CLI](multiplexing-phase5c-external-consumers-2026-10-06.md) | independent Warlock worktree, Warlock Rust/frontend boundaries; scoped CLI | caller migration, provider adapter conformance, UX/reconnect | final Reliquary API revision and A/B/E/F acceptance for upgraded build |
| [P5-E live runtime/public adapter](multiplexing-phase5e-live-runtime-surface-2026-10-06.md) | InteractionRuntime/session/adapter event-status surface | reconnect/resume, normalized events, controls, diagnostics | A/B primitives; consumed by C/G |
| [P5-F workspace management](multiplexing-phase5f-workspace-management-2026-10-06.md) | management facade and DTO/tests | conversation/files/Memory/health/import-export composition | A primitives and E runtime status |
| [P5-G interoperability](multiplexing-phase5g-interoperability-2026-10-06.md) | import/adapters/external source contracts | provider imports, ACP, external-derived transcript mode | E adapter seam; external source repos for host-specific adapters |
| [P6-H harness and measurement](multiplexing-phase6-parallel-verification-plan-2026-10-06.md) | new tests, stress/fault tooling, report fixtures | multi-instance matrix, host fakes, failure injection, profiling | final verdict only |
| [P5-D deletion/convergence](multiplexing-phase5d-legacy-deletion-2026-10-06.md) | Reliquary root module/public exports, old-host sources, final docs | remove obsolete host after callers migrate | A+B+C+E+F source gates; G core compatibility accounted |

Branch directly from the exact committed P5 plan base. Suggested Reliquary branches: feat/multiplexing-p5-rel-consumers, feat/multiplexing-p5-background, feat/multiplexing-p5-live-runtime, feat/multiplexing-p5-management, feat/multiplexing-p5-interoperability, feat/multiplexing-p6-harness. Suggested independent Warlock branch: feat/multiplexing-gateway-consumer (create a **new clean worktree**). Integrate on feat/multiplexing-p5-convergence after accepted source gates. Do not create speculative P5-D implementation on top of an unaccepted consumer branch.

## Scope coverage audit

| Requirement source | Required work | Owning track(s) |
| --- | --- | --- |
| Original Phase 5.1 | Archive, Memory, Graph/Entity, Perception, Dream, Insomnia, vectors, status, repository/reconciliation/config entry points | A + B, with E/F for final host-facing composition |
| Original Phase 5.2 | public library facade and real external consumers; Warlock, CLI, ACP/agents as applicable; queue/cancel/handoff rules | C + E + F + G, with B owning host capability semantics |
| Original Phase 5.3 | remove ambient ReliquaryRuntimeHost, host-global PHY/session state and obsolete forwarding | D |
| Original Phase 5.4 | current-state architecture/API/maintainer/contracts/limitations/ADR updates | D at convergence, lane-specific acceptance docs before it |
| Roadmap: complete live runtime seam | adapter reconnect/resume, normalized tool/session/artifact events, cancellation/shutdown, bounded controls, capability cache, diagnostics | E |
| Roadmap: workspace management API | conversation/session, files/sources, Memory lifecycle/inspection, health/stats, selective import/export | F over A/E primitives |
| Roadmap: production imports/interoperability | ChatGPT/Claude/structured agent imports and ACP | G |
| Roadmap: external canonical transcript integration | reference-only external source IDs/index/hydration/rebuild/Insomnia, Hermes first target | G using the existing R0–R7 plan |
| Phase 6 | multi-instance/failure/host/reconnect/privacy/performance and release gates | P6-H concurrently, P6-R post-D |

This matrix is the no-gap gate for the multiplexing program. A lane may split internally, but its rows cannot disappear or be declared complete by another lane's source guard alone.

Adjacent roadmap sections **File usability** and broader **Runtime/security hardening** remain separately owned future product milestones except where a concrete P5 consumer requires one of those capabilities to complete migration. Their explicit roadmap status is not a silent P5 omission; do not expand the multiplexing hard cut into unrelated product work without that dependency.

## P5.0 shared contract freeze — brief and bounded

Before *shared public export integration*, A/B/C/E/F/G publish one combined table of real consumer → exact GatewayRuntime/InstanceRuntime method, required grant, owner, durability, stream visibility and cancellation/reconciliation authority. Identify which methods already exist and which are truly missing. Freeze final missing signatures only after source-backed agreement; not a stub bus, compatibility facade, second queue or dual worker. A/B/E/F/G/P6 can begin work on already stable internal interfaces immediately. C can inventory/refactor Warlock-local state immediately and compile against the frozen updated API once available.

Required external contracts:

1. verified instance admission/binding; current grant, attach, detach, refresh and per-instance selected REL/branch;
2. native durable queue submit/status/cancel, generation control, host operation ID/handoff/Unknown Held;
3. authorized REL snapshot/subscribe/resync and exact conversation stream checkpoint join;
4. owner-qualified Archive/semantic/private PHY/project/maintenance operations with no mutable handle exports;
5. host capabilities ConversationExecution, Insomnia, Dream, Embedding and ProjectEnvironment only where real executors conform.

## Non-overlap and hard-cut rules

| Files or responsibility | Exclusive edit lane |
| --- | --- |
| runtime_host_gateway_ambient*.rs, runtime_host_gateway_semantic*.rs, gateway project/retrieval APIs | A |
| runtime_host_rel_*.rs, runtime_host_phy_*.rs, runtime_host_gateway_host.rs, worker and route ownership | B |
| Warlock workspace/conversation/chat/provider files and pinned manifests, relevant CLI callers | C |
| InteractionRuntime/session plus normalized adapter event/status modules | E |
| management facade/DTOs and management-specific tests | F |
| importer/external-source/ACP/Hermes adapter contracts and tests | G |
| new integration/stress/fault tests and benchmarks | P6-H |
| runtime_host.rs root, shared gateway module root exports, obsolete legacy runtime_host_* modules, old-only tests, final canonical docs | D/convergence |

A/B **do not both edit mixed legacy runtime_host_* files**: implement/use canonical gateway/REL/PHY paths, supply D a deletion manifest and maintain old characterization until the convergence gate. P6 fixtures may exercise public APIs but may not modify semantic owners. P3/4 coordinator, queue, sequencer, journal, grant and subscriber ownership are frozen: fix regressions at the established owner, not by introducing a wrapper.

Internal hard cut: coherent subgraph tests can replace whole-repo green temporarily during a declared isolated migration window. At every convergence gate, remove disposable compatibility code and restore whole-repo intended build/test baseline. Preserve persisted REL/PHY formats and concrete supported external disk/wire contracts; never assume old internal APIs are public promises simply because they exist.

## Ordered integration gates

**G0 — common base.** All Reliquary tracks branch from one exact main+plan commit, with clean worktrees; Warlock gets a separate clean checkout; both repos' existing dirty changes preserved. Capture real baseline and known inherited exceptions.

**G1 — consumer/API map.** Each still-live old host/public call and each roadmap host-facing responsibility has a named target method/track or explicit documented retirement/deferment. One minimal final external API freeze, no shims. A/B/E/F/G continue in parallel; C requests missing operations from their owning track.

**G2 — per-lane acceptance.** A: owner-scoped REL/private/project tests. B: worker/capability/host-recovery tests. E: live runtime adapter/reconnect/event/status tests. F: management authorization/composition tests. G: normalized import/external-source core plus explicit external adapter blockers/results. C: Warlock-local branch migrated, provider fake tests; final upgraded build still pending integration revision if required Reliquary lanes are not merged. P6: deterministic harness green against current APIs. Include a delete manifest and actual test logs, not just source guards.

**G3 — consumer convergence.** Merge A+B+E+F and applicable G core source into one Reliquary integration branch; resolve root module/public exports once. C pins *that* Reliquary revision, aligns direct Arcana pin, regenerates lockfile, compiles/tests upgraded Warlock. Preserve provider ownership and UI channel semantics. If C finds a missing operation, repair the canonical owning lane and rerun rather than reinstating old host.

**G4 — P5-D hard deletion.** Prove no real caller requires legacy host; delete obsolete module tree/exports/aliases; update retired characterization tests to Gateway/Instance test seams and update docs/ADR. Run Rust+CLI+Warlock negative source inventories and integration tests.

**G5 — P6 final release.** Run the complete adversarial and performance matrix on G4 output plus upgraded Warlock. Verify fmt/check/all appropriate tests/examples, CLI, docs, source guards and architecture-index status truthfully. Only after G5 may final merged main be described as P5/P6 complete.

## Critical invariants

Native pending queue stays REL-owned; inspected Warlock cannot claim durable cancel/dequeue authority. Host queue transfer requires durable acceptance, stable IDs, linearizable cancellation/dequeue, queryable status and idempotent handoff, all *verified on the actual adapter*. Unknown host outcome is held and reconciled, never replayed blindly. Running cancellation is cooperative, not guaranteed rollback. Same principal across two instances has one PHY worker; distinct principals never alias PHY. REL feed never carries PHY-private data or generation text. Owner locks are short; network/inference/backpressure and nested REL↔PHY owner locks are prohibited. No permanent legacy wrapper, duplicate processing authority or parallel mutable session.

## Completion gates

P5 requires all real consumers migrated and tested, the live runtime and management surfaces explicitly owned, known interoperability/import responsibilities implemented or carrying named external blockers, legacy host deleted, one canonical execution path and current-state documentation corrected. P6 requires final fault/recovery/stress/performance and release evidence. The serialized work is only shared API/export convergence, old-host deletion and final release acceptance—not all of P5 and P6.

## Documentation and architecture impact

Planning changes only: do not mark implemented APIs or migrations as shipped. At source integration update architecture, api, maintainer-map, invariants, behavioral-contracts, documentation-coverage, current-limitations, roadmap and ADR 0037 singleton language. Use current Arcana graph for supplementary reverse-dependency checks only if available; direct source/caller/test evidence remains decisive.

## Related docs

[P5-A](multiplexing-phase5a-rel-consumers-2026-10-06.md) · [P5-B](multiplexing-phase5b-background-host-2026-10-06.md) · [P5-C](multiplexing-phase5c-external-consumers-2026-10-06.md) · [P5-D](multiplexing-phase5d-legacy-deletion-2026-10-06.md) · [P5-E](multiplexing-phase5e-live-runtime-surface-2026-10-06.md) · [P5-F](multiplexing-phase5f-workspace-management-2026-10-06.md) · [P5-G](multiplexing-phase5g-interoperability-2026-10-06.md) · [P6](multiplexing-phase6-parallel-verification-plan-2026-10-06.md)

## Notes

The parallel plan freezes ownership and merge gates, not an implemented API. A lane acceptance record supersedes planning status only after actual source and tests establish it.

