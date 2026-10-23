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
