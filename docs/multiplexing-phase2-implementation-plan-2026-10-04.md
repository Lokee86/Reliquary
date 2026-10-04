# Multiplexing Phase 2 — PHY ownership and private processing implementation plan

Parent: [Multiplexing implementation plan](multiplexing-implementation-plan.md)  
Foundation: [Phase 1 finish record](multiplexing-phase1-finish-plan-2026-10-04.md)  
Personal-publication contract: [Phase 0B G2](multiplexing-phase0b-personal-publication.md)  
Authorization contract: [Phase 0B G3/G4](multiplexing-phase0b-events-authorization.md)  
Lifecycle contract: [Phase 0B G5/G6](multiplexing-phase0b-lifecycle-consumers.md)  
Authority decision: [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)  
Status: **R2.1–R2.7 verified; local packaging in progress**  
Required base: `feat/multiplexing-phase1` at or after `c65aa81213307a1f382df7cc46f4e64b53c89113`

## Purpose

Implement true multi-principal PHY isolation on top of the completed Phase 1 gateway authority model. Phase 2 makes each mounted PHY a first-class execution owner with one personal worker set, removes personal processing from REL execution state, durably attributes shared-REL work to verified principals, routes personal Insomnia results to exact PHY owners without ambient selection, and makes private semantic/retrieval access instance-authorized.

This is an ownership extraction and routing correction, not a redesign of Dream, Perception, Insomnia, vector generation, Memory semantics, or model/provider policy.

## Overview

Phase 1 already provides the pieces Phase 2 must reuse rather than duplicate:

- one `GatewayRuntime` registry keyed by durable REL/PHY owner ID;
- physical-identity mount claims and duplicate-owner exclusion;
- authenticated `InstanceRuntime` attachments with immutable principal/PHY binding and explicit grants;
- mount epochs, grant generations, short leases and lifecycle fencing;
- per-instance REL/conversation selection;
- one REL-wide durable conversation journal with captured initiating principal;
- process-local legacy/gateway exclusion;
- a clean committed Phase 1 branch.

The original mixed REL/PHY execution coupling has now been substantially extracted in the Phase 2 worktree. New `RelRuntime` and `PhyRuntime` implementations split REL and personal vectors/Dream/Perception execution, `PhyExecution` owns one personal worker set plus lifecycle fencing, the gateway enforces stable principal→PHY binding, and a new instance-authorized semantic layer exists. The legacy single-user `ReliquaryRuntimeHost` still contains its old ambient PHY fields by design until Phase 5, but the Phase 2 ownership guard reports **zero violations in the new gateway/per-PHY production modules**. Private retrieval/profile isolation, pending-work lifecycle and the final hard-cut audit are verified. The final matrix is recorded below; local packaging is in progress.

Phase 2 replaces that coupling with explicit owner-local execution:

```text
GatewayRuntime
├── REL execution (one per REL)
│   ├── InteractionRuntime / Archive / Episodes
│   ├── REL vector worker
│   ├── REL Dream worker
│   ├── REL Perception worker
│   └── REL Insomnia workers
│       └── durable cross-owner personal publication plan -> exact PHY(s)
│
├── PHY execution A (one per PHY)
│   ├── Phylactery
│   ├── personal compatibility profile
│   ├── personal vector worker
│   ├── personal Dream worker
│   └── personal Perception worker
│
├── PHY execution B
│   └── independent equivalent state
│
└── InstanceRuntime
    ├── verified principal -> exactly one bound PHY
    ├── explicit REL grants
    └── instance-local REL/conversation selection
```

**Critical ownership decision:** Insomnia remains REL-owned. Episodes, extraction/classification, the REL completion receipt and the source transcript all belong to the REL. Personal candidates are routed from that REL-owned operation to exact PHY destinations using a durable frozen publication plan. Do not create one Insomnia extractor/claimer per PHY.

## Non-goals

Phase 2 does not implement:

- the Phase 3 sequential/visible-branch conflict policy;
- Phase 4 REL update feeds or live generation stream subscriptions;
- Phase 5 Warlock/CLI consumer migration or deletion of `ReliquaryRuntimeHost`;
- production host transfer of Insomnia, Dream, Embedding or ProjectEnvironment;
- new REL/PHY semantic file formats;
- a distributed transaction across REL and PHY files;
- a generalized worker framework or generic owner trait solely to reduce code duplication;
- conversation ACLs beyond the existing REL-wide grant model;
- cross-process gateway arbitration.

Keep unsupported host capabilities Native/Held as established by Phase 1.

## Source-backed current ownership map

| Responsibility | Verified Phase 2 owner | Acceptance evidence |
| --- | --- | --- |
| PHY mount/identity/fence | One `PhyExecution` owns its `PhyRuntime`, mount epoch and fence | Same-principal sharing, duplicate identity rejection, remount and stale-token cases |
| Personal workers | Concrete PHY vector/Dream/Perception modules | One worker set per mounted PHY; detach persistence and attachment churn |
| REL workers | Concrete REL vector/Dream/Perception/Insomnia modules | No mutable PHY slot or personal queue; full library regression and canonical source guards |
| Principal binding | Gateway-lifetime principal→PHY mapping | Principal retargeting and cross-principal PHY aliasing rejected |
| Owner-operation persistence | REL-scoped operational journal | Ten journal cases; User/explicit Agent attribution, compaction and durable withdrawal |
| Insomnia routing | REL-owned frozen exact-owner publication | Sixteen production publication/recovery/lifecycle cases plus four codec cases |
| Private semantic access | Instance-authorized exact-owner reads and combined retrieval | A/B isolation, guessed-owner denial, independent grants and blocked-embedding reauthorization |
| Principal profile/wakeups | PHY-local mutation and explicit REL projection | Original capabilities and private mount retained through commit; exact-owner wakeups |
| PHY lifecycle | Exact-owner quiesce/reopen/reconcile/restart | Pending plans remain Held, admitted writes drain, unrelated owners progress |
| Ambient hard cut | Gateway and concrete REL/PHY source families | Python guard has zero violations; two compiled routing-guard cases and negative probes |

## Invariants

1. **Exactly one personal execution per mounted PHY.** Two instances for the same principal/PHY share it; no REL creates a clone.
2. **A REL never owns a mutable PHY handle after the Phase 2 cut.** REL code may request a routed PHY operation through Gateway authority, not dereference an ambient PHY slot.
3. **Distinct principals cannot share one PHY identity.** Same-principal multiple attachments are allowed.
4. **No personal worker consults active REL selection.** Personal workers are scheduled by their PHY owner state.
5. **No REL worker consults active PHY selection.** Personal publication resolves from durable provenance and captured principal attribution.
6. **A User/Personal classification is not routing authority.** Exact source-principal resolution plus verified principal→PHY mapping is required.
7. **Ambiguous or legacy-unknown personal provenance produces no PHY write and no REL fallback.**
8. **Cross-owner publication is recoverable, not atomic.** Exact route intent is durable before the first PHY write; retries never retarget.
9. **Private data is visible only through the initiating instance's bound PHY.** Mounting another PHY does not make it readable.
10. **REL dependency grants do not imply PHY access; PHY binding does not imply REL access.**
11. **Profile/vector/Dream/Perception background work is deduplicated by owner ID, not by instance count.**
12. **External/model work occurs outside owner locks.** Revalidate/fence at commit boundaries.
13. Existing REL/PHY semantic bytes remain readable; new operational recovery state stays under Gateway `state_dir`.

## Current implementation checkpoint — 2026-10-04

The implementation has advanced well past P2.0:

- **P2.1 — implemented and verified.** `PhyExecution` owns a real `PhyRuntime`; one personal vector/Dream/Perception worker set starts per mounted PHY. Gateway-lifetime principal→PHY binding prevents retargeting and cross-principal aliasing. Same-principal sharing, detach persistence, remount fencing, shutdown and lifecycle acceptance pass in the final 908-case library suite.
- **P2.2 — implemented and verified.** New `RelRuntime` and `PhyRuntime` module families split project and personal worker ownership. Direct source audit finds no mutable Phylactery slot, personal profile, `PerceptionOwner::User`, `active_execution()` or ambient zero-argument `active_rel_id()` in the new REL worker modules.
- **P2.3 — implemented and verified at the gateway's supported source boundary.** User admission syncs original identity plus optional captured UseOwnPhy authority; explicit Agent journal binding inherits that original admission. Ten journal cases and live admission/restart cases pass. Gateway assistant-result ingestion remains unsupported; missing or contradictory attribution stays Unknown/Held.
- **P2.4 — implemented and verified.** The coordinator freezes the complete prepared Episode, exact PHY destinations and authorizing User generations before writes, verifies exact original records/attachments, then commits/readbacks the REL receipt and observes it. Durable withdrawal covers mounted/offline journals and survives regrant/restart. Sixteen publication/recovery/lifecycle cases pass, including real worker replay without extraction or configured endpoint, mixed destinations, partial-write recovery, conflicts and missing-state repair.
- **P2.5 — implemented and verified.** Combined private search/retrieval and exact-owner semantic reads pass A/B privacy, guessed-owner, dependency-grant and blocked-embedding withdrawal/detach/remount acceptance.
- **P2.6 — implemented and verified.** Explicit A/B projection retains both original capabilities and the private mount through REL commit. Exact-PHY profile mutation and REL projection wake only their respective owners.
- **P2.7 — implemented and verified.** Offline/quiesced exact plans, reopen/remount, admitted-write draining, stale tokens, unrelated background vectors and attachment churn pass production acceptance.
- **P2.8 — implemented and verified.** Both canonical guards cover gateway and concrete REL/PHY production modules, reject legacy ownership/ambient selection, and pass negative probes. The isolated old host remains Phase 5.
- **P2.9 — verified; local packaging in progress.** The final library, integration, CLI, consumer-audit and documentation gates pass. The inherited example allocator conflict and bounded architecture timeout are recorded separately below.

R2.1 restored the compile checkpoint: formatting, diff-check, `cargo check --locked -j 2` and the enforced ownership guard passed. After durable User-attribution integration, `cargo test --locked --lib gateway:: -j 2` passed **86/86**, including six activated owner-operation journal cases and three new admission/restart/corruption cases. Publication scaffolding remains unused until R2.3. This is focused evidence, not the final Phase 2 matrix.

## Ordered critical path

Finish Phase 2 in this order; do not reopen completed extraction work unless a failing test proves a defect:

1. **R2.1 — restore a clean compile checkpoint (complete).** Run `cargo fmt --all`; expose or relocate `wake_phy`/`wake_rel` at the narrow internal module boundary; remove only imports that remain unused after journal integration. Require fmt, diff-check and `cargo check --locked -j 2` green before deeper work.
2. **R2.2 — integrate durable attribution (finish P2.3; complete at supported gateway source boundary).** Import the owner-operation journal into production, capture User actor/generation attribution from verified instance admission, reopen it with each REL, and fail closed on missing/corrupt attribution. Activate its existing tests plus mixed-principal/restart attribution tests.
3. **R2.3 — implement frozen exact-owner personal publication (finish P2.4; complete).** Replace the current Insomnia fail-closed placeholder with plan creation/recovery, exact principal→PHY resolution, stable mutation IDs, idempotent PHY writes, REL receipt verification, Held/offline behavior and crash replay. The authority/recovery matrix is green; the remaining gates below still determine Phase 2 completion.
4. **R2.4 — close private access and profile projection (finish P2.5/P2.6; complete).** Complete owner-qualified search/retrieval where needed; prove same-REL/different-PHY privacy, guessed-PHY denial, dependency grant isolation, revoke-during-embedding no-delivery, explicit A/B principal profile projection, and owner-local wakeups.
5. **R2.5 — close PHY lifecycle around pending routed work (finish P2.7; complete).** Prove quiesced/offline target plans remain Held without retargeting, reopen resumes exact owner work, stale tokens fail, unrelated REL/PHY owners keep progressing, and attach/detach churn never duplicates worker sets.
6. **R2.6 — final hard-cut audit (finish P2.8; complete).** Keep `check_multiplexing_phase2_ownership.py --enforce` green, verify the new gateway never calls legacy ambient personal APIs, and remove any extraction-only forwarding seam introduced during R2.2–R2.5. Do not delete the old host.
7. **R2.7 — final verification/package (P2.9; verification complete, packaging in progress).** Run the complete Phase 2 matrix from the spec, update current-state docs, record inherited unrelated failures separately, and commit coherent Phase 2 changes on `feat/multiplexing-phase2`.

The Phase 2 exit decision must be based on R2.1–R2.7, not merely on the existence of the new runtime modules.


## Execution sequence

### P2.0 — Establish the Phase 2 branch and verification baseline

Create Phase 2 from the completed Phase 1 head, **not from `main` while Phase 1 is still unmerged**. Preferred branch/worktree:

- branch: `feat/multiplexing-phase2`
- base: `c65aa81213307a1f382df7cc46f4e64b53c89113` or the exact later Phase 1 head if Phase 1 receives a documented repair;
- dedicated worktree; do not implement Phase 2 in the Freshness/staleness worktrees.

Before production edits, rerun a bounded Phase 1 smoke baseline: formatting, `cargo check --locked -j 2`, gateway/instance tests, the public gateway API contract, and the Phase 0B personal reference contract. Record inherited full-suite/example failures separately. Do not rerun the entire expensive release matrix after every subphase.

Create a Phase 2 source inventory test/search that identifies all production references to:
- `Shared.phylactery`
- `phylactery_enabled`
- mixed `RuntimeMemoryProfiles.user`
- `PerceptionOwner::User`
- host-global `phylactery_owner_id`/active-PHY semantic routing
- personal worker use of `active_execution()` or `active_rel_id()`.

**Exit:** a frozen baseline and explicit migration inventory; no Phase 1 regression before extraction begins.

### P2.1 — Make `PhyExecution` the final personal runtime owner *(implemented and verified)*

Deepen the existing gateway `PhyExecution`; do not create a parallel PHY registry.

Add only the state needed by personal execution:
- the mounted `Phylactery` handle;
- owner fence/mount epoch already present;
- owner-local stop/wake signal;
- personal compatibility-profile state;
- personal vector scheduling state;
- personal Dream cooldown/worker state;
- personal Perception queue/reconciliation state;
- worker join handles / lifecycle state.

Prefer a narrow internal `PhyRuntime` or equivalent private module if it keeps `PhyExecution` small. Do **not** introduce a generic `OwnerRuntime<T>` trait hierarchy unless an actual shared invariant cannot be expressed locally.

Start the personal worker set once when the PHY execution becomes runnable. Additional instances bound to the same PHY only add attachments; they do not start workers. Unmount/shutdown must stop the one worker set before returning the `Phylactery`.

Strengthen the principal binding invariant:
- multiple verified instances with the same principal may bind the same PHY;
- if a mounted PHY is already bound to principal A for the current gateway lifetime, verified principal B cannot attach it;
- a pending personal route intent freezes principal+PHY and prevents retry-time retargeting;
- do not persist a new identity field into the PHY semantic file solely for Phase 2. Trusted admission plus exact frozen route identity remains the authority.

**Tests:** two same-principal instances → one `PhyExecution` and one worker set; different principals cannot bind same PHY; two different PHYs are independent; instance detach does not stop shared personal workers; unmount stops workers and stale PHY tokens fail after remount.

**Exit:** mounted PHY is a real execution owner, not just `Arc<Mutex<Phylactery>>`, and worker lifetime is owner-scoped.

### P2.2 — Remove personal state from REL `OwnerExecution` *(implemented and verified)*

Hard-cut the mixed execution state once P2.1 exists.

From `runtime_host_owner_execution.rs::Shared` / `OwnerExecution`, remove:
- `phylactery: Arc<Mutex<Option<Phylactery>>>`;
- `phylactery_enabled`;
- the personal/user half of `RuntimeMemoryProfiles`;
- user entries in the REL Perception queue;
- any helper whose only purpose is "is a PHY attached to this REL execution?".

Split worker behavior explicitly:

**Vectors**
- REL vector worker opens/updates only the REL compatibility profile and REL Memories.
- PHY vector worker opens/updates only that PHY profile and personal Memories.
- Endpoint/profile inference may share a pure helper; mutable profile state does not.

**Dream**
- REL Dream worker uses the existing project preparation/commit path only.
- PHY Dream worker uses the personal preparation/commit path against its exact `Phylactery`.
- The personal worker queues Perception work only into its own PHY queue.

**Perception**
- REL worker owns only REL mention queue/reconciliation.
- PHY worker owns only the PHY mention queue/reconciliation.
- Remove runtime switching on `PerceptionOwner::{Project,User}` from the hot worker path when the split makes it unnecessary. Keep pure owner-polymorphic helpers only where they reduce duplication without reintroducing ambient ownership.

**Insomnia**
- stays REL-owned and is **not** moved into `PhyExecution`.

Model/provider calls stay outside owner locks. Prepare under one owner lock, release, infer, reacquire the same owner and validate revisions/epoch before commit.

**Tests:** two PHY workers process independent personal vectors/Dream/Perception while one REL continues project processing; adding a second instance causes no duplicate worker activity; stopping one PHY does not stop REL or another PHY; no personal worker reads a REL active-selection field.

**Exit:** source search finds no mutable Phylactery or personal profile/queue stored inside REL execution state.

### P2.3 — Make principal attribution durable and instance-derived *(implemented and verified at the supported gateway boundary)*

Use the Phase 1 authenticated instance as the only source of user identity.

For each admitted User turn:
- stamp `actor_principal` from `InstanceRuntime::principal_id()`; request payload cannot override it;
- continue writing the existing Archive principal field with its compatibility meaning where appropriate;
- capture a stable `generation_id` and `generation_initiator` for generated work from the verified initiating instance;
- never infer a later Agent turn's principal from its parent node.

Implement the narrow operational attribution record required by G2 under Gateway `state_dir`. Use a REL-scoped `owner-operations/<rel-uuid>.roj` sidecar or an equivalently isolated module implementing the already accepted contract:
- header binds schema/version/exact REL UUID;
- append-only bounded checksummed frames;
- `Attribution`, `EpisodePlan`, `EpisodeReceiptObserved`, `Held`;
- monotonic sequence;
- durable sync before the operation depends on a frame;
- truncated final-frame repair only; interior corruption fails closed;
- atomic compaction/replacement using the same proven replacement discipline as the conversation journal, without pretending the two journals are one format.

Do not duplicate facts already durably available in the Phase 1 conversation journal. The owner-operation record should bind the generation/source identity needed for personal routing and recovery; it may reference the stable conversation/submission identity instead of copying arbitrary transcript payloads.

Legacy/unknown attribution remains readable but cannot authorize a new personal PHY Memory.

**Tests:** two principals in one conversation preserve distinct User actor IDs and distinct generation initiators; late join and restart recover attribution; Agent ancestry cannot overwrite initiator; missing/corrupt attribution causes personal routing to hold/reject, not guess.

**Exit:** source principal can be resolved after restart without active instance state or parent-principal inference.

### P2.4 — Implement durable multi-principal personal publication *(implemented and verified)*

This is the core G2 production cut.

Keep Episode claim/extraction/classification on the REL Insomnia worker. After candidates are prepared, resolve every material provenance reference against the REL and durable attribution:

- a personal candidate requires at least one principal-authored supporting User source;
- all personal-bearing sources must resolve to exactly one principal;
- corroborating Agent sources may participate only when their durable generation attribution is consistent;
- unknown, contradictory or multi-principal personal evidence is quarantined/rejected;
- never turn an ambiguous personal candidate into a REL/project Memory as fallback.

Resolve that principal to its exact verified PHY owner. No active instance, selected REL, worker-local slot, request owner ID or first-mounted PHY may affect the destination.

Before the first cross-owner write:
1. derive stable mutation IDs for every result;
2. freeze the **entire Episode route plan**, including REL results, rejected candidates, personal payloads, provenance fingerprint, destination principal, destination PHY owner and expected receipt identities;
3. append/sync `EpisodePlan`.

Then publish:
- each personal Memory to its exact PHY using its stable mutation ID and owner-qualified `MemorySourceRef`;
- verify matching existing records as idempotent success;
- same-ID semantic/source-ref conflict → hold/fail closed;
- unavailable/revoked target → `Held`, never retarget;
- after every PHY result is verified, commit the REL-owned project batch plus Insomnia completion receipt containing owner-qualified external refs;
- read back/verify the REL receipt, then append/sync `EpisodeReceiptObserved`;
- only then acknowledge completion/schedule downstream completion-dependent work.

Recovery must load an existing frozen plan **before rerunning extraction, classification or wording**. Replay exact missing writes and exact receipt only. If the operational state directory is missing and a partial personal publication cannot be proven complete, hold for repair rather than reconstructing destinations from a new model run.

**Concurrency/authorization rule:** a route plan is owner/coordinator work once durably accepted, but each not-yet-admitted cross-owner write must still satisfy the captured owner/grant/fence policy. Revocation may hold remaining writes; it cannot redirect them.

**Tests:** one Episode with A-personal, B-personal and REL-shared candidates; A+B ambiguous candidate rejected; crash after each individual PHY write; crash after REL completion before observed marker; restart finishes once; principal→PHY mapping changes while plan pending cannot retarget; offline target remains held; same-principal two instances do not duplicate writes; no completed REL receipt references an unverified PHY Memory.

**Exit:** mixed-principal Insomnia publication is deterministic, restart-safe, privacy-safe and selection-independent.

### P2.5 — Move private retrieval and semantic visibility to instance-authorized owner sets *(implemented and verified)*

Implement the new gateway semantic-owner view; do not extend the old host-global visibility model.

For an instance:
- REL visibility = selected REL plus only dependency RELs for which the instance independently has the required grant;
- private visibility = **only** the exact bound PHY for that verified instance/principal;
- another mounted PHY is invisible even if its owner ID is guessed;
- a bound PHY does not grant REL access;
- a mounted REL does not grant PHY access.

Add gateway owner-polymorphic operations needed by current consumers:
- visible semantic-owner enumeration;
- exact-owner Memory/Entity/Graph/Perception reads where the underlying PHY supports them;
- memory retrieval/search across authorized REL owners plus the bound PHY;
- provenance/Echo expansion only through exact authorized source owners;
- personal profile/entity stats as exact bound-PHY operations.

Do not merge private PHY results into REL-wide APIs that are documented as shareable/broadcast-safe. Where a combined search is useful, return owner-qualified lanes/results so caller policy can preserve scope.

Embedding happens once where possible, then owner-local retrieval can run concurrently across the authorized owner set. Revalidate the instance/owner token after embedding and before returning private results; detach/revoke during provider I/O must prevent delivery.

**Tests:** same principal on two devices sees the same PHY; different principals sharing one REL see the same allowed REL data but different private lanes; guessing another PHY ID fails; dependency without grant is excluded; revoke during embedding prevents private result delivery; no PHY data appears in REL-only result/feed structures.

**Exit:** private read paths are instance/PHY-bound and cannot be widened by mount topology or REL selection.

### P2.6 — Correct principal profile projection and owner wakeups *(implemented and verified)*

Remove active-REL side effects from personal profile management.

- `set_phylactery_profile` equivalent on the gateway mutates/syncs only the exact bound PHY.
- REL principal-profile projection is keyed by explicit `principal_id`.
- Refresh a REL projection at a source-backed seam such as authenticated User-turn admission or explicit projection maintenance for that principal; do not broadcast a changed personal profile into whichever REL happens to be active.
- Two principals in one REL retain two independent principal entities/profiles.
- Personal Memory/Dream/Perception commits wake only that PHY's workers.
- REL Memory/Dream/Perception commits wake only that REL's workers.
- Cross-owner personal publication wakes the destination PHY after the durable PHY write, and wakes REL work only after the REL receipt requires it.

**Tests:** A and B alternate turns in one REL without overwriting profile projection; changing A's display profile does not mutate B or unrelated RELs; personal publication to A does not wake B; project publication does not schedule personal queues merely because an instance is attached.

**Exit:** no profile or wakeup behavior depends on “last active user/PHY”.

### P2.7 — Add PHY lifecycle, fencing and recovery *(implemented and verified)*

Phase 1 established REL lifecycle/fencing; extend the same concrete rules to the now-active PHY executions.

Add/finish trusted and instance-authorized PHY maintenance as needed:
- quiesce PHY admission;
- freeze personal queues;
- drain short commit/delivery leases;
- stop personal workers;
- reconcile owner-local durable state;
- reopen with new epoch/fence generation;
- restart exactly one worker set;
- unmount only after workers/leases are drained.

Pending cross-owner Episode plans targeting a quiesced/offline PHY remain durable and Held/Pending; they do not force-open a PHY or retarget. On reopen, recovery reconciles exact target owner and authorization before continuing.

Gateway shutdown stops REL and PHY workers exactly once. Detaching the last instance from a PHY does **not** automatically unmount it or stop background personal processing unless product lifecycle policy explicitly asks for unmount.

**Tests:** stale PHY token after quiesce/reopen; unmount/remount epoch invalidation; quiesce A while B and REL continue; pending route into A waits safely; shutdown with two PHYs joins all worker sets; attach/detach churn never creates duplicate personal workers.

**Exit:** PHY is lifecycle-equivalent to a real gateway owner, not a passive store hanging off an instance.

### P2.8 — Hard-cut remaining Phase 2 ambient personal ownership *(implemented and verified)*

Once the new gateway paths and per-PHY workers are green:

- remove personal `Phylactery` fields from REL execution/shared worker state;
- remove `phylactery_enabled` from REL worker control;
- remove mixed user/project memory-profile storage;
- remove user work from REL Perception queue;
- remove personal-worker dependence on `ReliquaryRuntimeHost::active_execution()`, `active_rel_id()`, `phylactery_owner_id()`, or active selection;
- remove any Phase 2-only forwarding seam introduced during extraction.

Do **not** delete the whole `ReliquaryRuntimeHost`; Phase 5 owns external legacy consumer migration. The legacy host may keep its isolated single-user implementation temporarily, but the new gateway path must not call back into it or synchronize mutable personal state with it.

Add a source guard covering the new gateway/per-PHY production modules so regression cannot reintroduce ambient PHY selection.

**Exit:** new gateway execution has exactly one personal ownership model: `GatewayRuntime::phy_executions[owner_id]`.

### P2.9 — Final verification and packaging *(verified; packaging in progress)*

Run the final matrix serially against the finished Phase 2 source.

Required:
1. `cargo fmt --all -- --check`
2. `git diff --check`
3. `cargo check --locked -j 2`
4. all gateway/instance/PHY execution tests
5. owner-operation journal + personal publication recovery tests
6. Insomnia ownership/routing tests
7. vector, Dream and Perception REL/PHY ownership tests
8. `multiplexing_phase0b_personal_contract`
9. `multiplexing_baseline_contract`
10. `multiplexing_gateway_api_contract`
11. full library tests
12. integration tests
13. CLI fmt/check/tests if public library signatures changed
14. both documentation checks
15. direct source guard for removed ambient personal ownership
16. one bounded architecture refresh/check; if indexing again times out, report it honestly and retain direct structural evidence rather than blocking indefinitely.

Run unrestricted `cargo test --locked` once as regression evidence; the inherited `archive_open_profile` allocator conflict remains non-Phase-2 unless its behavior changes.

Update current-state docs only for behavior actually shipped: architecture, API, invariants, behavioral contracts, storage-format description of the operational owner journal, current limitations, maintainer map and roadmap.

Package coherent commits on the Phase 2 branch. Do not merge to `main` unless separately requested.

## Test matrix

The minimum adversarial matrix must cover these combinations:

| Scenario | Required outcome |
| --- | --- |
| Same principal, two instances, same REL/PHY | independent instance selection; one PHY workers/store |
| Same principal, two RELs, same PHY | one personal execution; both RELs may route authorized personal results to it |
| Different principals, same REL, different PHYs | shared REL/project state; strictly separated personal writes/search/workers |
| Different principals attempt same PHY | reject attachment/binding |
| Principal A guesses B PHY owner ID | deny/no existence leakage beyond authorized error contract |
| Mixed A/B/REL Episode | exact A→A PHY, B→B PHY, shared→REL routing |
| Ambiguous A+B personal evidence | quarantine/reject; no PHY and no REL fallback |
| Target PHY offline after plan durability | plan Held/Pending; no retarget |
| Crash after A write before B write | restart writes only missing B then exact REL receipt |
| Crash after all PHY writes before REL receipt | verify existing PHY writes, then one REL receipt |
| Crash after REL receipt before observed marker | read back receipt; append marker; no model rerun |
| Revoke while embedding/searching | no private delivery after revocation |
| Revoke before planned PHY write | hold remaining exact destination |
| Quiesce one PHY | its work freezes/drains; other PHY and REL continue |
| Last instance detaches | owner-local background PHY work persists; user delivery/control ends |
| Two instances trigger same personal wake | one deduplicated owner work item |
| Profile A changes while B active in REL | only A PHY/profile projection changes |
| Legacy unknown principal source | readable source; no new personal publication |

## Persistence and compatibility

No existing REL or PHY semantic format version changes are required.

New operational state is gateway-owned and state-directory-scoped:
- `conversation-queues/<rel-uuid>.rqj` remains Phase 1 provisional conversation state;
- `owner-operations/<rel-uuid>.roj` holds Phase 2 attribution/cross-owner publication recovery.

The owner-operation journal is not Archive history, a Memory store, an event feed, or a cross-host transaction log. Losing it does not make REL/PHY bytes unreadable, but any personal operation whose safe destination/attribution cannot be proven becomes Unknown/Held and requires repair rather than guessed replay.

Preserve existing:
- REL/PHY owner UUID semantics;
- `MemorySourceRef` owner-qualified personal provenance;
- Insomnia completion receipt meaning;
- existing typed REL/PHY file decoding;
- existing single-principal behavior where attribution is unambiguous.

## Locking and performance constraints

- Never hold REL and PHY locks simultaneously.
- Never hold two PHY locks simultaneously.
- Freeze a cross-owner plan under operational-journal authority, release it, then perform each exact owner write independently.
- Provider/model/embedding calls occur outside owner locks.
- A personal worker holds only its own PHY mutation lock for bounded prepare/commit sections.
- REL Insomnia does not wait with the REL store locked while publishing to PHYs.
- Do not add a global gateway mutex around worker execution; topology/lifecycle gates remain short admission controls.
- Parallelize retrieval across authorized owners only after one query embedding is available.
- Do not optimize worker counts/lock sharding in Phase 2 without measurement; correctness and deduplication come first.

## Phase 2 exit gate

Phase 2 is complete only when all are true:

- every mounted PHY has exactly one gateway-owned personal execution and worker set;
- REL execution contains no mutable personal PHY slot, personal profile state or personal worker queue;
- Insomnia remains REL-owned but routes personal results from durable principal attribution to exact PHY owners through a restart-safe frozen plan;
- two principals can safely share one REL without cross-principal personal publication, retrieval, profile overwrite or background scheduling;
- same-principal multiple instances share one PHY execution without duplicate workers/writes;
- private semantic/retrieval access includes only the verified instance's bound PHY and independently granted RELs;
- ambiguous/legacy personal provenance cannot create a private write or shared fallback leak;
- personal vector/Dream/Perception work is PHY-local and independent of active REL/instance selection;
- PHY quiesce/remount/recovery fences stale operations and does not block unrelated owners;
- no production new-gateway path depends on host-global active PHY selection;
- Phase 0B G2 adversarial contract and production integration tests are green;
- final library/integration/CLI/docs verification has no new Phase 2 regression;
- implementation is documented and committed on the Phase 2 branch.

After this gate, Reliquary has actual multi-principal private-state isolation. Phase 3 can then extend the already-shared conversation coordinator without reopening PHY ownership.

## Implementation handoff

A fresh implementer should read, in order:

1. this document;
2. [Phase 1 finish record](multiplexing-phase1-finish-plan-2026-10-04.md);
3. [Phase 0B G2 personal publication](multiplexing-phase0b-personal-publication.md);
4. [Phase 0B authorization](multiplexing-phase0b-events-authorization.md);
5. [Phase 0B lifecycle](multiplexing-phase0b-lifecycle-consumers.md).

Then inspect the exact Phase 1 head and run P2.0 before editing.

**Serial dependency:** P2.0 → P2.1 → P2.2 → P2.3 → P2.4 → P2.5 → P2.6 → P2.7 → P2.8 → P2.9.

P2.3 owner-operation journal mechanics can be developed in a separate file scope while P2.1/P2.2 extraction is underway, but integration begins only after the final PHY owner exists. Retrieval tests for P2.5 may be prepared independently. Avoid parallel agents editing `runtime_host_gateway.rs`, `runtime_host_owner_execution.rs` or the same worker modules concurrently.

## Historical implementation record

### P2.0 — 2026-10-04

Created `feat/multiplexing-phase2` in its dedicated worktree at exact Phase 1 head `c65aa81213307a1f382df7cc46f4e64b53c89113`. Preserved the Phase 1 worktree's uncommitted documentation edits; copied the plan/index handoff into this worktree without modifying the original.

Before production extraction: `cargo fmt --all -- --check` and `cargo check --locked -j 2` passed; gateway tests passed 69/69, instance tests 7/7, `multiplexing_gateway_api_contract` 1/1, and `multiplexing_phase0b_personal_contract` 8/8. The latter is a reference-contract baseline, not proof that Phase 2 production routing has shipped.

The initial gateway test build encountered missing cached dependency artifacts. Targeted dependency-cache cleanup plus nonincremental rebuild repaired that incidental failure. One rebuild hit the ten-minute job limit before tests; its resumed run passed. No production source had changed during these smoke runs. The inherited unrestricted `archive_open_profile` example allocator conflict remains separately tracked until the final matrix.

`scripts/check_multiplexing_phase2_ownership.py` inventories the mixed legacy execution/profile/queue and ambient routing references. Its enforcement scope targets new gateway/per-PHY production modules, not the isolated Phase 5 legacy implementation. Since this baseline, P2.1/P2.2 extraction, substantial P2.5/P2.6/P2.7 source work and the P2.8 new-path guard have been implemented. The authoritative remaining-work status is the current checkpoint and R2.1–R2.7 critical path above; P2.3/P2.4 production integration remains the main blocker.

### R2.1 and R2.2 User-attribution checkpoint — 2026-10-04

R2.1 is complete. Limited wake-helper visibility to the gateway parent module and formatted the existing Phase 2 source. The original compile, formatting, diff and enforced ownership gates passed.

R2.2 now mounts one operational journal alongside each REL conversation journal, rejects corrupt operational state before mounting workers, and syncs verified User actor/initiator, stable conversation-operation ID, original attachment/owner grant generation, mount epoch and bound PHY before acknowledging new work. Retries from another same-principal attachment preserve the original admission. Missing entries and legacy sources are never backfilled from Archive principal fields or parent ancestry. The existing Archive/REL/PHY formats and public signatures are unchanged.

Activated and repaired the six orphaned journal tests (stale framing constants/checksum expectation, wrong-owner fixture setup and required external receipt refs). Added three production gateway tests for mixed principals in one conversation with fresh-gateway recovery, same-principal attachment retries, and corrupt/missing journal handling. The complete focused gateway run passed **86/86**. The first test build failed in Lore because expected dependency artifacts were missing; a separate nonincremental target with debug information disabled rebuilt the dependencies and passed. A test-only type-inference error was repaired before the successful run.

**Next in order:** finish R2.2 generated-source binding where a gateway generation/checkpoint/result entrypoint exists, then R2.3 frozen exact-owner publication. The current gateway ConversationExecution stage only finalizes admitted User turns; it does not expose host assistant-result semantics. Agent sources without explicit durable attribution remain Unknown. No personal publication coordinator or recovery claim is made at this checkpoint.

Documentation impact:
- Inspected: plan, architecture, invariants, storage format, maintainer map, current limitations, roadmap, behavioral contracts and documentation coverage.
- Updated: plan and current operational-journal documentation/coverage.
- Not affected: public API signatures, semantic bytes, provider policy and Phase 3–5 contracts.
- Compliance check: formatting, diff-check, `cargo check --locked -j 2`, the enforced ownership guard and both shared documentation modes passed after integration.
- Known documentation gaps: remaining Phase 2 behavior must graduate only when implemented and verified.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: existing REL-scoped operational journal is mounted by the gateway conversation registry; verified admission supplies source identity.
- Pitlord enforcement impact: existing Phase 2 ownership guard passed; the 60-second `check_architecture.py --refresh` attempt timed out during Lexicon Rust/Python analysis before Arcana/Pitlord enforcement, so no refreshed architecture-policy pass is claimed.
- Other verification impact: 86 gateway cases pass, including all nine journal/attribution cases.
- Known architectural gaps: generated-source binding and frozen publication/recovery remain open; refreshed Arcana/Pitlord evidence is unverified after the bounded indexing timeout.

### R2.2 generated-source binding and R2.3 integration checkpoint — 2026-10-04

Added explicit Agent source binding to a preceding durable User operation. The original conversation, initiating principal and admission/PHY identity are copied from that operation, with no caller-supplied principal or Archive ancestry. Append and replay validate the relationship; compaction retains its origin. User admission identities must agree, and contradictory actor records for one source resolve to Unknown. The mixed-principal gateway restart case now covers A's generated result arriving after B's User turn. The gateway still does not admit assistant results; this is the internal binding seam for that future entrypoint, and unbound Agent sources remain Unknown.

Publication coordinator/codec and REL-worker integration appeared concurrently in the shared worktree and were preserved. This pass repaired the codec's journal-type import, exposed deterministic temporal assessment at crate scope, separated semantic record verification from routing-attachment verification, and repaired the Held/retry return type. Added a regression proving record validation can precede attachment while final validation requires the attachment and rejects changed principal provenance.

Verification: the journal alone passed **9/9**; the complete focused gateway run passed **93/93**, including four codec cases. Formatting, both documentation modes and the enforced ownership guard passed before the final record update. The first integration compile exposed the Held/retry return mismatch and passed after repair. A rebuild regenerated dependency artifacts; no broad Phase 2 acceptance is inferred from this focused run.

**Next in order: R2.3 acceptance and authority completion.** The frozen coordinator is connected, but mixed A/B/project publication, individual-write crash replay, receipt-before-observation recovery, and offline-target acceptance are not proven. Captured UseOwnPhy authority and durable revocation must be connected to each remaining routed write; the current revocation-marker readers alone are not that gate. Do not advance to R2.4 or mark Phase 2 complete from this checkpoint.

Documentation impact:
- Inspected: Phase 2 plan, architecture, storage format, documentation coverage and current limitations.
- Updated: generation-binding format/ownership and this concrete implementation record.
- Not affected: public signatures, REL/PHY semantic formats and host assistant-result policy.
- Known gaps: R2.3 authority/recovery acceptance and the remaining ordered Phase 2 gates.

Architecture impact:
- Standards changed: none.
- Ownership: generation identity remains in the gateway's existing REL-scoped journal; no new semantic owner or ambient PHY selection.
- Structural enforcement: the direct Phase 2 ownership guard reports zero gateway violations.
- Verification: 93 focused gateway cases; final formatting, diff-check and both documentation modes passed. The new 60-second architecture refresh timed out during Lexicon Rust/Python analysis before Arcana/Pitlord; refreshed architecture enforcement is not claimed.


### R2.3 authority and recovery completion — 2026-10-04

R2.3 is complete. User admission now captures a current UseOwnPhy token independently of the stable destination binding, with both short owner leases covering journal sync. Bound instances may lack or withdraw private permission; binding never implies authorization. Frozen personal drafts name the exact durable User generations supporting every material source, including an explicitly bound Agent's origin.

Grant refresh drains admitted personal publications and syncs withdrawal markers for the original attachment before exposing changed grants. Offline REL journals participate, and compaction retains withdrawal across later Held reasons. Detach preserves owner work; fresh attachments or restored grants never revive withdrawn source authority. Read-only verification of a completed receipt remains permitted after withdrawal.

The worker holds missing durable authority before extraction, including on attempt one after reopen resets runtime counters. Existing plans replay without an extractor endpoint; bounded idle polling allows pending recovery to retry. PHY semantic records and routing attachments are verified separately so an exact write interrupted before attachment can resume. Same-ID conflicts, unavailable targets and changed exact bindings hold the operation rather than retargeting or publishing a REL completion.

Verification: **106/106 gateway tests passed**, including ten journal cases, four codec cases and twelve production publication cases. The publication matrix covers exact A/B/project routing; ambiguous/legacy rejection; no captured private grant; partial/all-PHY/REL-before-observation restart cuts; persistent withdrawal before and after A's write; offline REL withdrawal; same-ID conflicts; changed bindings; operational-state loss; routing-attachment recovery; and real background replay without any extractor configured. A fail-if-called endpoint also proves the worker skips extraction for an existing plan.

This pass repaired the admission helper's REL-only token restriction, removed the implicit binding-to-permission requirement, and corrected test setup to reclaim a fresh runtime lease after reopen and use a strictly later retry timestamp. Earlier failing builds/runs are superseded by the collected green run. Public signatures and REL/PHY semantic formats are unchanged.

**Next in order: R2.4 private access/profile acceptance.** Combined authorized retrieval remains to integrate and verify. Review the bound-but-ungranted owner's enumeration behavior and revalidate both private-source and REL authority at explicit profile projection. R2.5–R2.7 remain required; Phase 2 is not complete or packaged.

Documentation impact:
- Inspected: plan, architecture, storage format, invariants, current limitations and coverage.
- Updated: captured private admission, frozen authorizing generations, durable withdrawal/compaction, worker recovery and this ordered completion record.
- Not affected: public signatures, semantic storage formats, provider policy and gateway assistant-result policy.
- Compliance: formatting, diff-check, `cargo check --locked -j 2`, the ownership guard and both documentation modes passed.
- Remaining gaps: private retrieval/profile acceptance, pending-work lifecycle, final audit/full matrix and packaging.

Architecture impact:
- Standards changed: none.
- Ownership: the existing REL operational journal owns frozen intent and source withdrawal; each PHY owns its independent publication and workers.
- Boundaries: publication read admission permits independent writes in parallel; grant withdrawal alone takes the policy write gate. No REL/PHY semantic store locks or two PHY store locks are held together.
- Enforcement: the direct ownership guard passed with zero gateway violations. The 60-second architecture refresh timed out during Lexicon Rust/Python indexing before Arcana/Pitlord; no refreshed architecture pass is claimed.
- Remaining gaps: R2.4–R2.7; refreshed architecture evidence follows the bounded indexing result above.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Phase 1 finish record](multiplexing-phase1-finish-plan-2026-10-04.md)
- [Phase 0B G2 personal publication](multiplexing-phase0b-personal-publication.md)
- [Phase 0B events and authorization](multiplexing-phase0b-events-authorization.md)
- [Phase 0B lifecycle and consumers](multiplexing-phase0b-lifecycle-consumers.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Architecture](architecture.md)
- [Invariants](invariants.md)

## Notes

This plan makes the previously accepted Phase 2/G2 design executable against the completed Phase 1 source. It deliberately keeps REL Episode/Insomnia ownership separate from PHY personal processing, avoids a generalized worker abstraction, and does not pull Phase 3–5 consumer/synchronization work forward.


### R2.4 private retrieval and profile acceptance — 2026-10-04

R2.4 is complete. Wired the existing combined semantic-search implementation into the gateway, repaired its module/route imports, and omitted the bound PHY when UseOwnPhy is absent. One query embedding runs outside owner locks, then owner-local retrieval runs in parallel over independently ViewRel-granted REL dependencies and the exact authorized bound PHY. Search consumes established owner profiles without creating or mutating them; an unavailable profile is reported only in its authorized lane. Original attachment, grants and owner mount tokens are revalidated before delivery. REL-only APIs retain their scope.

Explicit profile projection now retains the original private-source authority through the REL commit. A short source fence lease and attachment lease prevent private mount or grant transitions from racing the commit; no REL/PHY semantic store locks overlap. Both original capabilities are checked. Principal Entities use the verified durable PHY ID rather than an arbitrary authenticated account label, and projection updates existing Entities only. Profile writes wake only their PHY; explicit projection wakes only its REL, independently of selected REL.

Five production acceptance cases prove same-REL A/B private search/read isolation; guessed-PHY denial; independent dependency grants; bound-but-ungranted omission; withdrawal of private or REL access, detach and private remount during blocked embedding; explicit A/B projection while selection points elsewhere; both original projection capabilities and source mount fencing; and exact-owner/idempotent wakeups. The first run passed the 106 existing cases but failed new fixture setup because metadata mutation requires a ViewRel grant on the dependency. The fixture now grants that authority for metadata setup and withdraws it before restricted retrieval; the final complete run passed **111/111 gateway tests**.

Verification: library formatting, diff-check and locked check; standalone CLI formatting, locked check and **19/19 tests**; enforced Phase 2 ownership guard; and both documentation-policy modes passed. No new semantic storage format or CLI signature change was introduced. The bounded architecture refresh remains part of the ordered R2.7 final gate; no new refreshed-policy pass is claimed here.

**Next in order: R2.5 pending routed-work lifecycle.** Quiesced/offline target plans, exact-owner reopen, unrelated-owner progress and attachment churn must pass before the R2.6 hard-cut audit and R2.7 full matrix/commits. Phase 2 is not complete or packaged.

Documentation impact:
- Inspected: plan, API, architecture, invariants, maintainer map, current limitations, storage-format guidance and roadmap.
- Updated: current private retrieval/projection API and ownership, acceptance mapping, current limitations and this completion record.
- Not affected: semantic storage formats, established grant/lock invariants, provider policy, gateway assistant-result admission and Phase 3–5 contracts.
- Compliance check: both shared documentation modes and diff-check passed.
- Known documentation gaps: pending lifecycle acceptance and final phase packaging remain explicitly open.

Architecture impact:
- Standards added or changed: none.
- Ownership or boundary impact: search uses existing exact-owner executions; private projection retains a source admission lease without taking its semantic store lock during REL commit.
- Pitlord enforcement impact: the direct ownership guard reports zero gateway violations; see the bounded refresh result below.
- Other verification impact: 111 gateway and 19 CLI cases pass.
- Known architectural gaps: pending-work lifecycle/full acceptance and refreshed architecture evidence.

The initial CLI test build could not find cached Lore, Reqwest and Base64 dependency artifacts in the shared target. Rebuilding the standalone CLI in its own nonincremental target repaired that infrastructure failure; the collected run passed all 19 tests and both documentation modes.


### R2.5 pending routed-work lifecycle acceptance — 2026-10-04

R2.5 is complete. Four additional production cases prove that an exact frozen plan holds while its PHY is offline or quiesced, leaves the REL receipt absent, and resumes at that same durable owner after remount/reopen. Original client tokens stay stale. An admitted private mutation drains and syncs before worker shutdown. Unrelated personal/REL operations continue, and B's real background vector worker finishes new canonical Memory while A is quiesced. Twenty rounds of same-principal dual attachment/detachment retain the identical single worker set; accepted frozen work completes after the final attachment detaches and replay creates no duplicate Memory.

The complete focused run passed **115/115 gateway cases**. A moved-token mistake in the new drain fixture was fixed by giving the writer a clone while retaining the original token for stale-access assertions. No production lifecycle repair was needed; these cases close acceptance of the existing concrete lifecycle implementation.

Next in order is R2.6, then the final R2.7 matrix and local branch packaging. The R2.4 standalone CLI result remains 19/19; this step changed only test modules and documentation, with no public or production implementation change.

Documentation impact:
- Inspected: API, architecture, invariants, behavioral contracts, current limitations, roadmap and maintainer map.
- Updated: exact-PHY lifecycle API, acceptance counts, current limitations and this record.
- Not affected: semantic bytes, source-authority rules, provider policy and later phase contracts.
- Compliance check: final formatting and both documentation modes run with the ordered audit/final gates.
- Known gaps: R2.6/R2.7 remain open at this checkpoint.

Architecture impact:
- Standards changed: none.
- Ownership: one existing PHY runtime survives attachment churn; exact frozen intent outlives attachment lifetime.
- Enforcement: no new semantic owner or forwarding seam; expanded structural audit follows in R2.6.
- Verification: 115 gateway cases, including four pending-lifecycle cases.
- Known gaps: final architecture refresh and broad regression/package evidence.


### R2.6 hard-cut audit — 2026-10-04

R2.6 is complete. The ownership script previously discovered gateway/PHY modules but omitted the new REL worker family. Its canonical scope now includes gateway, REL and PHY production modules and rejects legacy Host/OwnerExecution/profile/Perception ownership types as well as ambient selection. The compiled routing guard discovers the same family automatically and adds negative probes for those legacy types and ambient PHY selection.

Direct source audit found no legacy worker/selection forwarding in any new production module. No extraction-only forwarding seam needed removal. The old single-user host remains isolated for Phase 5. Both guards pass, including two compiled routing cases and temporary-file Python probes proving REL/PHY violations are rejected while instance-taking selection, test fixtures and the isolated legacy path remain permitted. Formatting, diff-check and both documentation modes passed.

Documentation impact:
- Inspected: API, architecture, invariants, behavioral contracts, coverage, maintainer map, current limitations and parent plan.
- Updated: current private/lifecycle contracts and acceptance mapping, canonical guard scope, Phase 1 inventory qualifiers and ordered status.
- Not affected: semantic bytes, provider policy, grant vocabulary and later phase contracts.
- Compliance check: formatting/diff and both documentation modes passed before the final record update; final R2.7 repeats them.
- Known gaps: final broad matrix, bounded architecture refresh and local commits.

Architecture impact:
- Standards changed: none.
- Ownership: one canonical concrete REL/PHY execution path; no bridge to legacy personal authority.
- Enforcement: both structural guards cover the new REL family and report zero canonical-path violations.
- Verification: two compiled routing cases plus Python negative probes; 115 focused gateway cases remain green.
- Known gaps: R2.7 final regression/refresh/package evidence.

### R2.7 final verification and packaging — 2026-10-04

The ordered Phase 2 implementation and acceptance gates are complete. The final source retains one concrete execution per REL/PHY, REL-owned extraction with frozen exact-owner personal publication, instance-authorized private retrieval and fenced explicit profile projection. No Phase 3–5 branch/feed/consumer migration is included.

| Gate | Final result |
| --- | --- |
| Root formatting, diff hygiene and locked check | Passed |
| Full library suite | 908/908 passed, including all 115 gateway cases |
| All Rust integration targets | 93/93 passed across all 11 targets |
| Standalone CLI formatting, locked check and tests | Passed; 19/19 tests |
| Python Phase 0B consumer audit | 5/5 passed against the mounted consumer checkout |
| Canonical Python ownership guard | Passed; zero violations in gateway and concrete REL/PHY modules |
| Compiled routing guard | 2/2 passed within the integration matrix; negative probes also passed |
| Documentation policy | Structural and changed-from-origin/main modes passed |
| Unrestricted Cargo test | Inherited `archive_open_profile` example global-allocator conflict; no changed Phase 2 failure |
| Bounded architecture refresh | 60-second timeout during Lexicon indexing; no refreshed Arcana/Pitlord policy pass claimed |

The CLI uses its independent lockfile and a separate target directory. Missing cached dependency artifacts encountered earlier were repaired by the isolated build; the final check/test completed successfully. Existing semantic storage formats are unchanged; the owner-operation journal remains gateway operational state. Unsupported gateway assistant-result ingestion stays unsupported; missing or contradictory original authority remains Unknown/Held. Combined retrieval requires established owner profiles, and explicit principal projection updates existing PHY-keyed principal Entities.

Documentation impact:
- Inspected: architecture, API, invariants, behavioral contracts, storage format, current limitations, maintainer map, documentation coverage, roadmap, index and both multiplexing plans.
- Updated: these current-state documents and the Phase 2 completion record describe only implemented and verified ownership, authorization, recovery, private retrieval and lifecycle behavior.
- Not affected: CLI command contracts, existing REL/PHY semantic byte formats, graph-corpus import behavior and Phase 3–5 designs.
- Compliance check: both required documentation-policy modes passed; final record edits are checked again before packaging.
- Known documentation gaps: none for the supported Phase 2 boundary; unsupported ingestion and verification limits remain explicit.

Architecture impact:
- Standards added or changed: no shared engineering standard or architectural decision changed.
- Ownership or boundary impact: PHY owns its personal worker set; REL owns Insomnia and completion; Gateway owns durable attribution, exact routing and authority; Instance owns authorized attachment/context.
- Pitlord enforcement impact: one bounded refresh timed out before completed refreshed enforcement. Canonical source guards directly enforce the new ownership boundary; legacy-host inventory remains outside Phase 2 enforcement for Phase 5.
- Other verification impact: full library/integration/CLI/consumer-audit acceptance and both documentation checks passed.
- Known architectural gaps: refreshed Arcana/Pitlord evidence is unavailable after the bounded indexing timeout; the inherited example allocator conflict prevents an unrestricted Cargo test pass. Neither is represented as a green gate.

Local packaging on `feat/multiplexing-phase2` follows this verified checkpoint. No merge or push to `main` is authorized or performed.
