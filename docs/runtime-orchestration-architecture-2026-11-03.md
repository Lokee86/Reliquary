# Runtime and orchestration architecture — Stage 3 final design (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Design status: COMPLETE FOR STAGE 3; no Rust implementation has changed.** This specifies Gateway/Instance ownership, scheduling, host delegation, durable coordination, worker concurrency, stream visibility, recovery and the application workflows that connect [Stage 1 storage](rel-phy-storage-architecture-2026-11-03.md) to [Stage 2 semantic services](semantic-service-architecture-2026-11-03.md). The Rust crate DAG and migration order are Stages 4 and 6. The decisions are recorded in [ADR 0046](decisions/0046-gateway-runtime-and-named-workflow-ownership.md) and grounded by the [runtime source crosswalk](runtime-orchestration-source-crosswalk-2026-11-03.md).

## Purpose

Replace the Root runtime/owner dependency knot with a single well-defined execution authority **without reimplementing the existing multiplexed product**. Preserve behavior, durability, privacy, host capability contracts and throughput while enabling future services (Observation, Ego, other host integrations) to join by narrow workflow/scheduling seams.

## Overview

- **Gateway** is the canonical in-process mounted owner and execution coordinator. One owner worker set per mounted REL/PHY, many short-lived authenticated Instances, one authoritative operational journal/queue per owning operation scope.
- **Instance** owns only identity/authorization/incarnation, selected REL/conversation/branch and viewer state. It does not have a semantic store, independent conversation execution or ownership of another principal's PHY.
- **REL/PHY sessions** own durable semantic facts, typed read/commit and validation. The Gateway holds owner-session handles but never copies their mutable domain stores; its journals for provisional work/host results are **operational evidence**, not Archive/Memory semantic state.
- **Named workflows** are focused application operations: source ingest, Insomnia completion, Dream settlement, Entity Resolution, retrieval/accepted-use, import, maintenance, generation finalization and context delivery. They prepare/compute outside locks and commit through typed owner-session operations.
- **Semantic evaluators** own algorithms, not owner locks, provider transport, worker lifetimes or final durable publication. Context Engine assembles final model-facing context from supplied authorized inputs; Ego later provides a bounded projection.
- The existing Gateway/Instance implementation in Root is **real**; the `reliquary-runtime-gateway` crate is still a placeholder. Design acceptance is not extraction acceptance.

## 1. Selected runtime shape and rejected alternatives

| Shape | Benefit | Failure mode | Decision |
| --- | --- | --- | --- |
| Entire `InteractionRuntime`/CVA/Gateway moved unchanged into one crate | Minimal relocation edits | Clones present semantic mutable god-object and Root interdependence | **Reject** |
| Independent Gateway per Instance, workers per session and one queue per host/provider | Simple isolation at first | Duplicate owner state, queue races, cross-instance inconsistent ordering and broken owner-grant revocation | **Reject** |
| **One Gateway per process with owner-scoped sessions/workers and instance-only attached views** | Shared owner authority, explicit lifetime and privacy, bounded coordination | Requires canonical operation/journal/types and disciplined lock/commit gates | **Select** |
| Generic workflow engine, event bus or actor framework for every semantic feature | Apparent extensibility | Unnecessary indirection and competing event truth; hard to reason about publication and crash cases | **Reject** |

The Gateway may internally organize **deep modules** for attachments, mounted owner lifecycle, conversation execution, host contract, journals, streaming, worker pools and provider-facing adapters. These are responsibility boundaries, not a demand for individual Cargo crates. A provider can be replaced at a **real host transport seam**; semantic workflows remain concrete and typed. Do not embed all processing into a universal `GatewayManager` or a public “run arbitrary closure with REL stores” API.

## 2. Authority, object lifetime and the access contract

```text
Authenticated host/user request
       |
Gateway admission (principal + exact grants, attachment incarnation)
       |
InstanceRuntime (view, selection, grant generation; no mutable REL/PHY)
       |
Gateway owner mount registry (owner ID, file kind, mount epoch, fence)
       |              |                    |
  REL/PHY worker set  |           conversation/host journals
       |              |
  named workflow <----+----> semantic evaluation + Switchboard endpoint
       |
short typed owner-session read / commit
       |
  RelSession / PhySession (one authoritative mutable aggregate per owner)
       |
 purpose-built persistent domain owners / Container
```

**Gateway authority:** mounted-owner identity, physical file claim/exclusivity as supported, mount generation, owner admission fence, owner-local sequencer, worker lifecycle, durable operational queue/journals, instance attach/revoke/detach, connection state, capability delegation, stream distribution, host handoff, cancellation, and shutdown. Gateway retains current trusted **embedding-level** lifecycle operations but does **not** expose them as unauthenticated instance request APIs.

**Instance authority:** verified principal, exact owner grants, attached PHY identity, attachment incarnation/generation, active REL and selected conversation/branch/leaf state. A user-facing `Arc<InstanceRuntime>` is an observed attachment, not a grant mutation handle. **Across the Rust crate seam**, Gateway-only mutations require a private per-instance controller/capability or equally strong unforgeable authority; never turn former `pub(super)` grant/select/detach operations into unrestricted public functions just to compile. `runtime_host_instances.rs` is the concrete source witness.

**Mounted REL:** holds a single handle to `RelSession`, owner admission/mount epoch, owner-local durable work references, worker control, sequencer/distribution state and relevant semantic route bindings. **Mounted PHY:** holds one `PhySession` plus PHY-eligible workers/queues. Per-principal PHY binding is unique across the Gateway, as in the current implementation. Detached Instances do not unload shared owners.

**Owner access token:** bound to attachment ID/incarnation, current grant generation, durable owner ID and kind, mount epoch, current fence generation, and required grant. This is a *revalidation coordinate*, not free-standing transferable authorization or a persisted bypass capability. Revalidate on admission, before hydration, commit and external delivery after any lengthy inference/host work. REL dependency closure does not grant other users' PHY rights.

**Offline owner tools:** Explicit exclusive open/maintenance access is distinct from a Gateway-mounted write. It must honor physical writer claims and owner-local format constraints; a direct standalone `Rel` facade cannot silently bypass concurrent Gateway ownership or authorization requirements.

## 3. Journals, queue authorities and states

**Two separate kinds of durable truth:**
1. **Semantic truth** is in REL/PHY domain stores: accepted Archive turns/branches, Memory/Graph/Ego/history, Insomnia work/attempt results, Dream/Freshness events, provenance and source receipts.
2. **Operational execution truth** is in Gateway journals keyed to a REL and/or exact operation: provisional conversation submissions, cancellation/finalize intent, host handoff/status, generation progress/checkpoints, owner-operation attribution. The existing `RQMUXJ01/RQMUXS01/RQMUXH01` and `RQOP0001` families remain authoritative for **their operational transitions**, not independent semantic publication.

An operation binding includes stable owner UUID, conversation/branch/submission or workflow identity, actor/principal, selected source cut/expected head where applicable, payload digest, and relevant mount/host generations. Never use instance selection, wall time alone or arbitrary provider response text as an identity.

**Native conversation queue lifecycle:** `Pending` durable enqueue → selected by a branch coordinator → `FinalizeWon` / in-flight transition as in existing journal → Archive final commitment → durable post-commit receipt. `Cancelled` can win only before finalization; the queue's atomic cancel/dequeue ordering determines the winner. Pending content is **not** an Archive turn, REL activity tick, Episode or Insomnia input. Recovery checks Archive by stable submission identity before replaying a finalization with a missing post-commit receipt; no duplicate turn.

**One queue owner per stage:** native Gateway remains authoritative for native pending submissions/owner workers. Host-owned pending queue is possible **only** when host negotiation proves durable acceptance, stable IDs, linearizable cancel/dequeue, queryable status and idempotent handoff. `ExecutionOnly` means execution delegated but **native pending admission remains native**. Existing `GatewayHostAdapter`, `HostQueueGuarantees`, `StageAuthority::{Native,Host,Held}` and `CapabilityRouter` already represent this boundary. No parallel competing queues for the same stage.

**Insomnia work is not the conversation queue.** Its Episode-based lease/attempt/complete state remains in `InsomniaStore` inside REL owner state. Workers use typed claim/renew/complete operations and bound idempotent publication; the Gateway merely executes and schedules them. Dream first-cycle/pair evidence remains its existing durable owner record; worker wake/defer is operational. Perception mention retries similarly distinguish durable resolution state from rebuildable scheduling queues.

**Unknown host outcome:** persist exact handoff intent before dispatch; host accepts or returns a matching stable ID and payload digest. Invalid/transport-ambiguous receipts or remote unknown state become **Held** pending reconciliation, not “failed, retry from scratch”. On reconnect, query the authoritative host and compare the exact operation; no unverified duplicate dispatch, automatic silent failover, or fabricated success.

**Operational journals are not a second database framework.** Preserve specific codecs and crash semantics, one journal implementation per operation family, and exactly one sequencer/authority; do not introduce a universal append-only event store or a second queue abstraction simply because domain stores also append frames.

## 4. Named workflows and transaction boundaries

All workflows follow a *short-owner-access / compute-unlocked / revalidate-and-commit* structure where existing behavior permits it. A proposed semantic effect is not a completed transaction. The lower owner-session contract from Stage 1 owns final admission and grouped physical writes.

| Workflow (named responsibility) | Phase 1: authorized read/prepare | Phase 2: compute / remote operations (no owner mutation lock) | Phase 3: revalidate and commit / result |
| --- | --- | --- | --- |
| **Accept native turn / continue conversation** | Validate attachment, owner, branch head, request and submission identity; durable queue admission | If generation required, build Context Engine request and execute negotiated host/model operation | Serialize FinalizeWon against cancel, commit accepted Archive turn exactly once, sync; post-commit queue receipt; generate Episode eligibility only for accepted turns |
| **Insomnia Episode processing** | Claim exact durable Episode work lease; hydrate bounded Archive/host evidence and owner-qualified source proof | Insomnia extraction/Chronos/verified routing and metadata | Recheck source/lease/grants, commit PHY-targeted user draft first where applicable, then grouped REL Insomnia/Memory/Freshness completion, preserve receipt and crash retry |
| **Dream first-cycle settlement** | Read owner Memory/body/version, pair candidates, Graph/Community snapshot, profile and first-cycle proof | Dream classification/verification, lifecycle, Chronos and optional Community naming | Recheck source/version/Graph cut, named owner-local Graph/Memory/Dream journal/Freshness effects, exact first-cycle idempotence; zero-link settled accepted |
| **Entity Resolution / future Observation** | Read verified body-bound mentions, relevant candidate facts, authorized Graph/Entity/evidence cuts | Bounded matching/admission/ambiguity or future Observation evaluation | Validate unchanged mention/body/entity and grants, commit typed Entity/resolution/Observation evidence; schedule reconsideration without an ambient global graph |
| **Vector/profile publication** | Snapshot exact Memory/Archive source versions and profile, durable work identity | Embedding endpoint and compatibility/packing | Recheck exact source/profile, publish generation/vector batch through typed owner command; retries cannot create incompatible rows |
| **Retrieval and agentic selection** | Authenticate source REL closure/requester PHY; read bounded candidate facts under correct owner access | Pure retrieval scoring, bounded decision query/refinement/traversal | Revalidate IDs, owner permission and cut before hydration; deliver only selected content; no score reinforcement during internal work |
| **Confirmed accepted-use** | Bind exact host handoff receipt to owner/turn/Memory IDs | Freshness computes effect from accepted current Graph/Community facts | Commit idempotent accepted-use receipt only after actual successful delivery; update derived due schedule |
| **Context and Ego** | Collect authorized current-session transcript/compaction, current Memory facts, active-PHY Ego records and permitted REL summaries | Context Engine assembles bounded request; future Ego synthesis is its own source-grounded worker | Store source-bound Ego result only when future feature exists, via PHY/REL typed command; no context read is an accepted-use event |
| **Import and external source** | Native import: lock exclusive epoch and validate source receipts. External host source: authenticate reference/hydration authority | Normalize import or hydrate external evidence without inventing Archive turns | Named import acceptance/settlement; external index/publication receipts are scoped; no forged native authority |
| **History / maintenance / reconcile** | Obtain explicit exclusive/offline owner proof or read-only supported cut | History composes proofs; maintenance plans transformed copy; optional bounded provider work outside owner lock | Verify exact source identity/head and target copy; write, reopen, verify, and separately promote; never mutate mounted owner through generic “all stores” access |

**Cross-owner operations** have no fictional atomic multi-file commit. Bind source and destination identities, commit each exact owner independently and reconcile from durable receipts. A REL-to-PHY routed Insomnia result may already have been published in PHY when REL completion fails; deterministic retries verify matching PHY semantics, then finish the REL, or fail closed. Revocation before the commit stops new unauthorized work while durable previously accepted evidence remains auditable.

**Application workflows are not Gateway methods by necessity.** Their semantic preparation, publication criteria and error mapping belong to cohesive workflow modules that can also be exercised by trusted offline/public facades where permitted. Gateway calls them under current authorization, leases, schedule and host binding; workflows must not import `GatewayRuntime` to get a `RelSession`. The exact Rust package boundary is Stage 4.

## 5. Concurrency and lock ordering

**Primary rule: a semantic owner lock is short and never held over model/embedding/host I/O.** Current `GatewayRuntime::with_rel` and `RelShared::mutate` already take owner-local admission/sequencer gates and the `InteractionRuntime` mutex; Stage 1 replaces this fat `Cva` dependency with an owner session plus separate runtime session selection. This is a **behavior-preserving change**, not a new lock hierarchy invented to mask an existing blocking call.

**General acquisition protocol:**
1. Resolve instance/token + owner identity and obtain an admission/fence lease.
2. Acquire only needed **owner-local** sequencer/short owner mutex for read/commit. Release it before evaluator or external call.
3. For a commit after unlocked work, reacquire owner admission and validate current owner/grant generation, expected revision/cut and idempotency. New data may require rebuilding/re-evaluating the proposal under bounded retry policy, not writing an obsolete decision.
4. Operational journal mutations acquire their own short lock; **never** retain a journal lock while waiting on owner mutex or external host. Where one transition touches both, journal intent and owner receipt are sequenced with explicit durable phases/recovery.
5. For multiple owners, acquire no simultaneous broad REL+PHY locks; perform independent owner operations with stable evidence and receipts. Stable lock order in small internal pairs is documented locally when unavoidable, but is not an authorization substitute.
6. Distribution snapshot/watermark gate serializes publication visibility separately from owner data lock. It may snapshot owner facts then release owner lock before applying distribution; the sequence boundary must prevent a concurrent mutation being missed.

**Separate lock scopes:** mount lifecycle/topology state, instance map/grants, per-owner admission/fence, per-conversation scheduling gate, operational journal, owner session and stream registry. One process-wide mutex over all owners is rejected. Backpressure is bounded per owner/capability and cannot turn a slow subscriber into a block on semantic writes. PHY worker sharing is same-principal only.

**Worker lifecycle:** each mounted owner has a runtime worker coordinator with native work queues/rebuildable wake signals, bounded parallel model work, per-worker cancellation and drain. Current specialties include Insomnia, Dream, entity resolution/Perception, vector generation and due Freshness work. Adding a future Observation or Ego worker adds a scheduling strategy *and a typed workflow*, not new owner fields in Gateway. Worker loops use durable owner work identity/receipt whenever correctness requires retry after restart; an ephemeral wake queue alone cannot authorize publication.

## 6. Streaming, subscriptions and host-facing execution

- **REL-wide update feed** (committed metadata/owner changes) is separate from **conversation/generation stream**; viewing REL does not automatically subscribe to every generation. Only authorized subscribers of the relevant REL conversation/generation receive text.
- Mount epoch + source cursor + owner-qualified generation/branch identity are validated for every subscription. A subscription from a previous incarnation, revoked grant or mount epoch cannot become current by replaying its cursor.
- **Commit/sync before visibility:** generated text and tools are only exposed after the corresponding durable checkpoint. A successful owner commit is published through the distribution sequencer after the authoritative source is observable; no synthetic “committed” notification from an unsynced staging buffer.
- **Snapshot + watermark handshake** is atomic with subscription so a mutation during reconnect is not lost. A cursor gap/overflow/epoch rotation returns resynchronization, not an invented stream continuation.
- Late join/rejoin must reauthenticate, reconstruct authoritative REL queue and generation status, and then attach only selected streams. Disconnecting an Instance cannot cancel another Instance's generation merely because it loses a viewer.
- PHY-private Memory and Ego state never enters general REL update/event payloads merely through cross-owner retrieval or active-PHY composition.
- `LiveRuntimeAdapter` and `LiveSession` remain host-facing recipient projections over Gateway authority. They may not own a separate journal, queue or semantic state; current `interaction_live.rs` explicitly follows that rule.

## 7. Startup, recovery, quiesce and shutdown

**Recovery is a product operation, not a “try replay until it seems okay” loop:**

1. Recover exact typed owner session under its file lease using Stage 1 current-only open/replay. Reject corrupt/unsupported semantic data before admitting writes.
2. Reopen the Gateway's owner-qualified operational journals, verifying sequence/frame integrity and exact owner/generation binding. Journal states cannot override Archive acceptance.
3. Reconcile durable pending/finalized submissions with Archive by stable identity: cancellation winning means no Archive turn; Archive committed but missing journal receipt means finalize the receipt idempotently; unknown host outcome remains Held.
4. Reconcile host capability/status from the negotiated contract before allowing any remote redispatch. Existing operation/owner attribution journals preserve exact source, generation and phase; no second dispatch under an ambiguous receipt.
5. Reconstruct per-owner work eligibility from accepted Insomnia/Dream/Entity and other durable work/receipt projections. Rebuild volatile Freshness boundary index, notifications, wake/scheduling queues and derived indexes; no inference required to *open* an owner.
6. Reinstall owner mount epoch and current admission fences, then workers; only after durable consistency and host reconciliation is the owner made write-admissible. Clients reconnect via fresh tokens and snapshot+watermark, not old stream cursors.
7. Failure of any mandatory proof leaves the corresponding owner/capability held/fenced, not globally corrupting unrelated owners. Non-atomic cross-owner work resumes from its retained exact receipts; no fabricated all-or-nothing transaction.

**Quiesce:** close owner admission, drain accepted short leases, stop/join workers and fence the sequencer. Existing `quiesce_rel` explicitly excludes direct out-of-process writers from its in-process guarantee; physical file claims must be checked separately. An owner can reopen only after the required operational reconciliation, new mount epoch and verified independent session open.

**Shutdown:** reject new attachments/dispatch, establish closed fences, persist/settle recorded durable transitions and stop workers. Do not transfer owner authority to an Instance on detach or abandon ambiguous host work by treating it as completed.

## 8. Source-grounded witness and isolation requirements

| Existing implementation witness | Why it matters |
| --- | --- |
| `runtime_host_gateway.rs::GatewayRuntime` | Already owns mounted REL/PHY maps, Instance attachments, host/router, lifecycle, conversation registry and shutdown. Keep **one** canonical runtime authority. |
| `runtime_host_instances.rs::InstanceRuntime` | Private state includes principal, optional PHY, grant generations and selected conversation. `pub(super)` mutators require a real trust cut when moving across crates. |
| `runtime_host_rel_execution.rs::RelShared`, `runtime_host_phy_execution.rs::WorkerShared` | Owner-local workers exist, but hold `Arc<Mutex<InteractionRuntime>>` or `Arc<Mutex<Phylactery>>`. New design replaces aggregate access with typed owner sessions plus runtime-specific ephemeral state. |
| `runtime_host_gateway.rs::with_rel`, `LeaseGate` | Real attachment/owner generations and short lock/fence behavior; cannot be replaced with casual global scope or unverified callback. |
| `runtime_host_gateway_conversation.rs::submit_conversation` and `process_reserved_conversation_submission` | Exact journal admission, retry, branch scheduling and Archive finalization proof exist. Do not invent a second queue or move pending turns into Archive. |
| `multiplexing_journal.rs::RelJournal` | Queue/generation/host statuses, staged payload limits and `RQMUXJ01/RQMUXS01/RQMUXH01` recovery are operational journal authority, separate from Archive. |
| `runtime_host_owner_operation_journal.rs::OwnerOperationJournal` | `RQOP0001` owner attribution, plan and receipt journal proves other operational recovery needs. Do not replace with a generic semantic store. |
| `runtime_host_gateway_host.rs::GatewayHostAdapter` / `CapabilityRouter` | Actual host queue-conformance and `Native/Host/Held` states already exist; preserve transport/queue decision semantics and unknown-status holds. |
| `runtime_host_gateway_streams.rs::join_stream_bound`, `runtime_host_gateway_convergence.rs::with_conversation_distribution` | Epoch-bound snapshot/stream registration and source-versus-distribution sequencing exist; preserve durable-prefix visibility and gap repair. |
| `interaction_runtime.rs::InteractionRuntime` | Holds `cva: Cva` and local session map; must split semantic state from runtime conversation selection rather than merely rename `cva` to `rel`. |
| `interaction_live.rs::LiveSession` | Live user-facing session is a recipient over Gateway authority and deliberately owns no store, queue or journal. |

See [source disposition crosswalk](runtime-orchestration-source-crosswalk-2026-11-03.md) for intended destinations and contract tests.

## 9. Failure matrix and required verification

| Fault / race | Required stable outcome |
| --- | --- |
| Two Instances submit to one branch concurrently | One gateway-coordinated finalization order or explicit user-selected auto-fork; no overwritten turn |
| Pending cancel races dequeue/finalize | One durable winner; cancelled entry never accepted into Archive or replay-dispatched |
| Restart after durable enqueue before dispatch | Pending submission replayed exactly once with same immutable identity/payload |
| Crash after Archive append but before queue receipt | Reconcile Archive and journal; no duplicate turn, activity count or Episode |
| Host handoff times out / ambiguous receipt | Held until exact host status reconciles; no blind redispatch |
| Host queue lacks one guarantee | Retain native pending admission; no conflicting host queue ownership |
| Instance grant revoked during long model call | Later hydration/commit/delivery revalidates; no new unauthorized effect |
| Same-principal PHY and multiple Instances | Shared PHY runtime; no independent duplicate mutable PHY |
| Different principals attach same PHY | Reject rather than leak or duplicate private state |
| REL unmount/remount during worker work | Stale mount epoch/fence cannot commit or deliver; resume after authoritative reopen |
| Stream subscriber lag/gap or reconnect | Snapshot+watermark or explicit resync; never invent undisclosed committed output |
| Worker killed after semantic result but before grouped commit | Durable work/attempt and exact source permit idempotent resume, not model-result = committed |
| REL-to-PHY publication crashes between owners | Verify already accepted PHY record, complete REL or fail closed, no fake rollback |
| Owner open fails on invalid complete semantic record | File remains unwritable; explicit repair or canonical current-format migration, not inference-driven repair |
| Owner operational journal invalid | Owner/capability held/fenced according to affected authority; no partial state promotion |

**Mandatory implementation gates later:** source call graph and owner-method crosswalk, no lower dependency on Root/Gateway, locked workspace/CLI/focused and full tests, serialized queue and journal golden fixtures, interleaving/fault injection, host adapter conformance, worker-count invariance, stream/delivery replay, authorization negative tests, PHY privacy, mount/remount and existing measured hot-path performance. Do not claim a Green Gateway merely because a small `InstanceRuntime` crate builds. The current six-file `CVAFRS07` recovery failure remains an independent acceptance prerequisite.

## 10. Closure and remaining architecture phases

**Stage 3 decisions closed:** one Gateway and owner runtime per mounted identity; per-Instance view-only lifetime; private grant mutation seam; owner session injection; operational-versus-semantic journal truth; native/host stage queue arbitration; exact order of conversation submission/finalization/cancellation; durable checkpoint stream visibility; runtime/worker locks and coordination; defined cross-service workflows and recovery matrix; no universal orchestration engine.

**Later design:** Stage 4 names physical crates/modules and proves acyclic imports, depth and no Root backreach; Stage 5 covers code/format CVA→REL with stable IDs/corpus conversion; Stage 6 maps every source symbol to a staged hard-cut and acceptance. Neither Stage 3 nor a prior partial Gateway extraction spec authorizes starting implementation.

## Related docs

- [Stage 1 storage](rel-phy-storage-architecture-2026-11-03.md)
- [Stage 2 semantic service architecture](semantic-service-architecture-2026-11-03.md)
- [Runtime source boundary crosswalk](runtime-orchestration-source-crosswalk-2026-11-03.md)
- [ADR 0046](decisions/0046-gateway-runtime-and-named-workflow-ownership.md)
- [Current multiplexing behavior](subsystems/runtime-multiplexing.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Current inventory](current-system-inventory-2026-11-03.md)
- [Older partial Gateway extraction specification](runtime-gateway-service-crate-extraction-spec-2026-10-30.md)

## Notes

This is the **target runtime design**, not a claim of code relocation, newly passing host conformance, or production readiness. The current Root Gateway implementation is preserved as the behavioral baseline until migration proves replacement of its aggregate and journals.
