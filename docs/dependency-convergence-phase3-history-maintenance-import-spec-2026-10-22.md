# Dependency Convergence — Phase 3: History, Maintenance, Import and Reconciliation

Date: 2026-10-22
Status: **Correction-only execution specification; implementation in progress (formal acceptance tracked separately)**
Parent: [Dependency convergence architecture refactor](dependency-convergence-architecture-refactor-2026-10-21.md)
Authoritative DAG: [Accepted logical component DAG](dependency-convergence-target-dag-2026-10-21.md) and [machine allowlist](../tools/pitlord/convergence-component-dag.json)
Predecessor: [Phase 2 accepted local execution record](dependency-convergence-phase2-execution-2026-10-22.md) and [Phase 3 owner-contract handoff](dependency-convergence-phase3-handoff-2026-10-22.md)
Pre-approved storage correction: [Semantic import-settlement minimization](semantic-import-settlement-minimization-plan-2026-10-21.md)
Execution sequencing and evidence overlay: [Phase 3 execution realignment](dependency-convergence-phase3-execution-realignment-2026-10-22.md)

## Purpose

Remove historical, maintenance, import and reconciliation policy from broad facade/store internals. Make these **three deep, directional components** over Phase 2's owner-local contracts, without building new product capabilities or entangling the stores again. Perform the **already-approved** semantic import-settlement size correction once the owner proof seams support it.

## Overview

This spec builds directly on Phase 1 physical storage and Phase 2 owner-local state. The Phase 2 ledger reports 91 tasks/12 gates locally accepted, the 34-component DAG unchanged, full feature-enabled Rust and CLI suites green, and the six-source historical oracle green. **That ledger is the source of those acceptance claims, not an independent current validation of the worktree or a claim Phase 2 has been committed.** Gate B0 must pin the exact accepted source checkpoint before Phase 3 code begins. No concurrent Phase 2 changes are absorbed silently.

There are **98 stable numbered execution tasks in twelve formal acceptance sections**. The owner-lifecycle work embedded in P3.3 proved larger than its original task breakdown, so the [execution realignment](dependency-convergence-phase3-execution-realignment-2026-10-22.md) now identifies narrower, evidence-backed enabling checkpoints. These do not expand the 98-task denominator or claim gate acceptance. Tasks are deterministic Rust/source/fixture changes; no task requires repeated LLM inference, rebuilding semantic Dreams, model calls, or restarting historical normalization. The full Arcana/Lexicon source graph is refreshed **once after source stabilization**, not once per task. Long opt-in historical oracles run at the phase integration/corpus gates, not in every owner checkpoint.

## Problem statement

Current code has reusable historical cut/proof logic, rewrite planning, Import epoch coordination and owner maintenance scattered across `src/facade/historical_*`, `cva_reconcile*`, `cva_history_prune`, `phylactery_history_prune`, `storage_reclamation`, `imports/{reconciliation,completion,readiness}` and `src/rewrite/**`. Even where Phase 2 removed store-to-store imports, this cross-owner policy remains hard to reason about when it is implemented as arbitrary Cva methods.

Separately, the already completed 377-step storage hard cut retained the wrong **size** of immutable semantic-import settlement: the current `CVAFRS06` family uses **7,415,741 bytes** across six canonical fixtures, versus **395,533 bytes** for the former physical-prefix-dependent family (18.75×; +7,020,208 bytes). The old representation was not independently prunable and **must not** be restored. The objective is to remove **reconstructible duplicated semantic state**, not shave arbitrary bytes at the expense of authority, historical verification, corruption detection or pruning.

## Target architecture and immutable invariants

```text
public Cva/PHY/CLI wrappers (thin; Phase 6 refines public surface)
    ├──> Import coordinator ──> read-only History / owner facts
    │          └──> owner-lifecycle / Archive / Memory / Freshness / Dream / Insomnia / Perception owners
    └──> Maintenance ──> read-only History ──> Phase 2 owner-local read/proof views
                    └──> Container / owner guarded writes
History  ──X──> Maintenance, Import, facade or Runtime
owner stores ──X──> History/Maintenance/Import upcalls
```

Arrow means **consumer depends on dependency**, not ownership transfer. The accepted DAG permits **Import → History and owner-lifecycle/owner services**, but **does not permit Import → Maintenance**. Therefore generic reconciliation policy belongs in Maintenance and epoch-specific reconciliation belongs in Import, sharing the already accepted lower History/owner evidence and owner-lifecycle physical operations rather than directly calling each other. Any useful generic copy primitive needed by both belongs at a genuinely lower existing owner seam only when its responsibility justifies it; it must not become a new universal coordinator. **Do not add an Import→Maintenance edge without a separately reviewed, explicit DAG change.** Maintenance must never call Import upward.

- **History is read-only**: bounded current/historical cuts, accepted owner proof inspection and reconstruction. It cannot commit Freshness changes, prune, repair, invoke providers, or acquire semantic writer authority. Existing explicit offline historical Freshness **publication** is split from History read-only preparation, not moved into a read-only interface.
- **Maintenance owns authorized mutation policy**: explicit prune, reclaim, physical/semantic repair, fresh-history rewrite and generic divergent-history reconciliation. It uses History read views and each store's accepted publication/retention contracts. It never authors Memory/Graph/Entity semantics on behalf of stores.
- **Import owns epoch-specific decisions**: durable ImportStart/Complete bindings, scoped admission/fences, exact readiness, scoped historical Freshness settlement, completion sync and recovery, import-specific divergent continuation policy. It cannot make unaccepted authoring evidence true.
- **No circular, upward, ancestral or unlisted dependencies**, including noncyclic backreach; no hidden runtime/facade callback or common generic mutable manager. Owner qualified REL/PHY scopes remain private; History reads never broaden authorization.
- **One physical writer/snapshot authority**, one owner-local storage authority, one logical History read path. No automatic open-time repair, physical prefix pinning, unlimited temporary payload materialization, and no second project VCS.
- **No new semantic feature**: no full historical restore, new divergence resolution/synchronization policy, new mounted import/reconstruction mode, automatic historical embeddings, Observation synthesis, Ego/Context inference machinery, External Sources redesign, Project Environment features or new Dream schedules.
- **Current-format-only policy**: ordinary source code supports **one** development format. The explicit semantic settlement correction is an exception to Phase 2's no-format-change rule, using the existing hard-cut development corpus policy. An interim converter exists **offline only** and is deleted after accepted promotion. Persisted real external user-data support is a separate, explicitly evidenced exception, not an excuse for generic compatibility wrappers.
- **Don't overbuild**: preserve deep concrete modules. No one-trait-per-store interfaces, bridge adapters, proof copy database, generalized transaction/replay framework or alternate source-of-truth.

### Explicit ownership routing (existing source to final responsibility)

| Current source | Target authority and correction |
| --- | --- |
| `src/facade/historical_global_{cuts,frontier}.rs`, `historical_cut_enumeration.rs`, `owner_history_proof.rs`, existing read-only reconstruction helpers | History-owned nonmutating proof/cut composition. Test-only historical fingerprint/oracle files **remain test-only**, not a runtime service. |
| `src/facade/historical_reconstruction.rs`, `cva_historical_import.rs`, `historical_import_base.rs` | History prepares accepted cut facts; Import/higher authorized operation owns freeze/settlement publication and existing explicit offline behavior. |
| `src/maintenance/{store,manifest}.rs`, `facade/{cva,phylactery}_history_prune.rs`, `storage_reclamation.rs`, `semantic_maintenance.rs` | Durable maintenance proof remains owner-local; Maintenance service composes retention, explicit repair/prune/reclaim and authorized mutation. |
| `src/rewrite/{model,plan,file,semantic_state,lineage}.rs` and facade rewrite wrappers | Existing Maintenance-owned rewrite planning/copy/publication; **no redesign of the rewrite product feature or identity modes**. |
| `src/facade/cva_reconcile*.rs`, `src/imports/{reconciliation,reconciliation_replay}.rs` | Maintenance generic divergent-history comparison and publication; Import retains epoch-specific interpretation and decides whether existing bounded replay is authorized. |
| `src/imports/{epoch_store,epoch_model,epoch_codec,epoch_scope,readiness,completion}.rs` | Import store owns epoch state; Import higher coordinator owns readiness, settlement and atomic completion. |
| `src/freshness/freshness_semantic_settlement.rs`, `freshness_import_settlement.rs`, Freshness replay; `src/imports/completion.rs` | Freshness owns correct compact semantic settlement format/validation; Import builds/binds/commits accepted epoch proof; owner facts remain authoritative elsewhere. |

### Interface alternatives considered

1. **Chosen:** History read-only, Maintenance explicit operations, Import epoch policy, all consuming owner-local facts. High leverage at three meaningful boundaries, minimum reverse dependencies.
2. **Rejected:** Keep all coordination in Cva/facade with forwarding traits. Makes the facade the integration hub again.
3. **Rejected:** A new all-domain replay/transaction framework or common generic history store. Duplicates Phase 1 parser and Phase 2 owner authority; harder to validate.
4. **Rejected:** A permanently dual-format `CVAFRS06`/new codec reader. Violates the development hard-cut and leaves needless dead machinery.

## Migration strategy and work discipline

Choose a **staged hard cut**, not perpetual expand-contract. Before each subsystem cut, record the final owner/function and all consumers. Move the implementation once, migrate callers directly, delete old paths and prove the production dependency direction. A short, explicitly bounded **red migration window** is allowed while coherent source consumers move; localized tests apply during the cut, full Rust/integration gates at the named block closure. Temporary wrappers are allowed only when an actual external API requires them, and remain outward delegates rather than dual internal owners.

**Dependency/order constraints:** P3.0→P3.1 are mandatory prerequisites. Owner-lifecycle REL/PHY reconstruction and bounded replay are explicit enabling checkpoints before M3 acceptance, not a new DAG component or a second M3 policy owner. Verified History, pruning and Import preparations may be credited against later tasks without granting their formal gate ahead of its declared prerequisite; full P3 prune/reclaim acceptance is not an implicit M3 prerequisite. The [execution realignment](dependency-convergence-phase3-execution-realignment-2026-10-22.md) controls the checkpoint ledger and paused hard-cut handoff.  P3.2 establishes History; P3.3–P3.6 converge higher Maintenance and existing rewrite/reconciliation. P3.7 Import depends on stable owner proof and History seams. P3.8 freezes the corrected settlement representation. **P3.9 is the only planned format-changing migration**, with separate approved design, proof tests, disposable conversion, canonical corpus promotion/commit, and immediate old codec/converter deletion. P3.10 ratchets once code stabilizes; P3.11 is a full integration/behavior/history/corpus gate. Independent narrow tests may run in parallel; no simultaneous edits to `src/facade`/import completion without explicit single owner.

**Testing policy:** deterministic Rust source/tests and immutable six-file history oracles only. Real model inference is not a phase requirement. No repeated exhaustive full-scan or 6,000-second history oracle per step. Run focused tests per move, full Rust suite at H3/M3/I3/C3 final integration as appropriate, the exhaustive oracle on converted corpus only after freeze and at final A3. Use the source index/Arcana if **already current**, otherwise targeted `rg`; regenerate source graph **once** at D3. Never relaunch index scans to answer documentation questions.

## 12 gated execution blocks — 98 bounded tasks

### P3.0 — Freeze Phase 2 handoff and historical before-image

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.0.01** | accepted Phase 2 source handoff | Pin exact locally accepted Phase 2 source tree, commit or uncommitted source manifest and gate receipts | Separate accepted source evidence from any concurrent Phase 2 mutation |
| **P3.0.02** | P3.0.01 | Inventory Phase 0 original graph, reviewed Phase 2 convergence delta and current 34-component DAG | Preserve immutable before-image and 4,630 reviewed findings; no rebaseline |
| **P3.0.03** | P3.0.02 | Map production facade/historical_*, cva_reconcile*, native import, history prune, storage_reclamation and rewrite consumers | Each operation has a canonical owner, reader/writer authority and target seam |
| **P3.0.04** | P3.0.03 | Inventory maintenance/store, maintenance/manifest and Phase 2 accepted owner read/proof methods | No imagined store APIs or new durable owner |
| **P3.0.05** | P3.0.04 | Freeze all six source REL/PHY canonical history files, original digests, V2 clock/proof oracle and CVAFRS06 family byte costs | Preserve immutable pre-format baseline for later semantic-equivalence comparison |
| **P3.0.06** | P3.0.05 | Identify current CLI/API and mounted Runtime uses of historical, maintenance and import operations | No new public behavior or mounted operations authorized |
| **P3.0.07** | P3.0.06 | Record executable focused baseline tests and current source-edge violations, tagging test-only and analyzer artifacts | B0 bounded source/corpus evidence, including Phase 2 Windows/platform qualifications |

**Gate B0:** Frozen Phase 2 source, owner contracts, original graph, V2 corpus hashes and permitted operation inventory, without touching current implementation.

### P3.1 — Freeze History/Maintenance/Import interfaces and hard-cut windows

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.1.01** | B0 | Define History deep interface over accepted read-only owner cuts and proofs | History never owns writer, Freshness reinforcement or Maintenance/Import dependency |
| **P3.1.02** | P3.1.01 | Define Maintenance as coordinator for explicit prune, reclaim, repair, rewrite and generic reconciliation | Owner-specific validation stays in each store; no reverse import |
| **P3.1.03** | P3.1.02 | Define Import as exclusive epoch/readiness/completion, historical settlement and epoch-aware reconciliation coordinator | Import invokes permitted lower History/owner/owner-lifecycle primitives; neither Import → Maintenance nor the reverse is allowed |
| **P3.1.04** | P3.1.03 | Choose concrete existing inspect/plan/authorize/emit/verify/publish shapes and error boundaries | No universal transaction manager, service locator or duplicate replay |
| **P3.1.05** | P3.1.04 | Freeze current file identity, read snapshot, exclusive write lease, collision, generation and crash/retry guarantees | Failed read/plan leaves bytes unchanged; fail closed on stale source |
| **P3.1.06** | P3.1.05 | Specify temporary Cva/PHY/CLI public adapters above operation owners until Phase 6 | Thin delegation, not duplicate policy or bidirectional facade dependency |
| **P3.1.07** | P3.1.06 | Freeze format-design gate: field-level proof inventory, two alternative compact designs, independent reconstruction evidence | No hash-only shortcut lacking actual accepted member proof |
| **P3.1.08** | P3.1.07 | Declare hard-cut stages, permitted bounded red windows, developer corpus promotion and delete-once rules | B1 concrete contracts with old internal paths assigned deletion checkpoints |

**Gate B1:** Concrete deep History/Maintenance/Import interfaces, DAG direction, hard-cut/delete maps and settlement audit/design choices reviewed.

### P3.2 — Extract read-only History ownership

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.2.01** | B1 | Partition historical_global_cuts/frontier, cut enumeration and historical reconstruction from test-only fingerprint/oracle files | No new general-purpose history engine or promoted test oracle |
| **P3.2.02** | P3.2.01 | Move existing read-only cut reconstruction/composition into History over Phase 2 owner read facts | Narrow concrete API, no Cva internals imported into History |
| **P3.2.03** | P3.2.02 | Reuse bounded physical prefix cursor and owner-local replay for historical evidence | Exact frame clock and frontier parity, no second parser |
| **P3.2.04** | P3.2.03 | Concentrate existing read-only cross-owner historical proof lookup in History | Owners keep fact/proof encoding and mutation authority |
| **P3.2.05** | P3.2.04 | Move reusable normalized source-history digest inspection from facade/owner_history_proof into History-owned read path | Same digest across permitted repack/prune relocation |
| **P3.2.06** | P3.2.05 | Separate explicit historical Freshness preparation/inspection from authorized settlement publication | `reconstruct_historical_freshness` remains supported offline, but History never commits or calls model endpoint |
| **P3.2.07** | P3.2.06 | Verify every History/prefix/metadata inspection is nonmutating and nonreinforcing | Byte hashes and Freshness accepted-use counters unchanged |
| **P3.2.08** | P3.2.07 | Cut obsolete production history lookup duplicates, retain only needed test fixtures | H3 accepted historical cuts, known error order and REL/PHY parity |

**Gate H3:** One nonmutating production History read/proof path; existing historical operations still behave correctly, with publication outside History.

### P3.3 — Converge Maintenance and accepted proof authority

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.3.01** | H3 | Bind owner-neutral maintenance manifest store and proof metadata to high Maintenance operation | No second manifest store or generic semantic owner |
| **P3.3.02** | P3.3.01 | Establish the single Maintenance authorization and cross-owner retention/eligibility admission boundary using owner proofs; complete operation-specific prune/reclaim policy at P3.4 | Exact owner authorization and invalid-reference rejection; no implicit full P3 gate |
| **P3.3.03** | P3.3.02 | Compose accepted watermark, publication and relocation-normalized history proof through History | No `History -> Maintenance` backreach |
| **P3.3.04** | P3.3.03 | Preserve REL Archive retention versus PHY own-domain maintenance limits | No automatic cross-scope access or Archive behavior added to PHY |
| **P3.3.05** | P3.3.04 | Freeze explicit maintenance authorizer and stable source snapshot before destructive transformed copy | Ordinary owner open remains byte-pure |
| **P3.3.06** | P3.3.05 | Keep Phase 1 tail/compaction/Entity repair explicitly selected and properly writer-guarded | No new auto-repair, no temporary unsafe write route |
| **P3.3.07** | P3.3.06 | Migrate supported public maintenance entrypoints to thin outward error-mapping dispatch | Same API/CLI semantics and refusal outcomes |
| **P3.3.08** | P3.3.07 | Delete duplicated facade Maintenance authorization/repair and common policy routes moved in M3; remove domain-specific prune/reclaim duplicates under P3.4 | M3 no store-to-Maintenance/facade dependency; P3 remains independently gated |

**Gate M3:** One Maintenance mutation authorization boundary; accepted owner-local facts and Phase 1 file identity/lease upheld.

### P3.4 — Pruning, reclamation and retained historical evidence

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.4.01** | M3 | Move REL/PHY history_prune coordination to Maintenance without altering selection | Supported source domains and explicit policy preserved |
| **P3.4.02** | P3.4.01 | Make each owner provide payload-retention proof and let Maintenance compose it | No shared semantic eligibility rule invented by Container |
| **P3.4.03** | P3.4.02 | Preserve prune-marker original identity, revision, digest, ordering and historical payload-availability evidence | Pruned payload never mistaken for corrupt complete record |
| **P3.4.04** | P3.4.03 | Keep conservative reclaim separate from prune and terminal lifecycle | Tombstoning alone never licenses physical erasure |
| **P3.4.05** | P3.4.04 | Verify read-only History proof equivalence before/after eligible payload removal | Canonical semantic history and clocks preserved |
| **P3.4.06** | P3.4.05 | Preserve retired vector/profile, compact body and description proof checks | No retained source evidence or live references lost |
| **P3.4.07** | P3.4.06 | Use Phase 1 writer/snapshot/destination authority through copy, validation, publish and reopen | Source file untouched, collision and interrupted output refused |
| **P3.4.08** | P3.4.07 | Hard-cut duplicate prune/reclaim policy routes and prove REL/PHY tests | P3 source DAG/range and owner-evidence suite passes |

**Gate P3:** REL/PHY prune/reclaim existing policy preserved, with exact retained/pruned history proof and no implicit deletion.

### P3.5 — Rewrite planning and transformed-copy publication

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.5.01** | P3 | Place existing rewrite/model, plan, semantic_state, lineage and file coordination under Maintenance responsibility | No new rewrite filters or version control features |
| **P3.5.02** | P3.5.01 | Separate rewrite policy and referential closure planning from destination emission | Planning read-only, execution explicitly authorized |
| **P3.5.03** | P3.5.02 | Use owner read/proof views for retained Memory, Archive, Entity, Graph, Ego, Freshness, vector references | No dangling or unprivileged cross-owner retention |
| **P3.5.04** | P3.5.03 | Preserve new-owner and preserve-owner generation/rewrite lineage semantics | Different generations cannot silently reconcile |
| **P3.5.05** | P3.5.04 | Keep existing profile/vector row selection exact without new embedding generation | No inference needed for rewrite |
| **P3.5.06** | P3.5.05 | Verify stable source identity/head and destination atomic publication/lease semantics | Source untouched on conflict or write failure |
| **P3.5.07** | P3.5.06 | Run compact/retired/import/Ego/Graph rewrite and controlled destructive selection oracles | Exact accepted selected outcome and proof validity |
| **P3.5.08** | P3.5.07 | Remove old policy-owning facade/rewrite bridges, leave only supported public callers | W3 transformation and source-integrity gates pass |

**Gate W3:** Existing rewrite closure, identity/generation and copy/publish remain correct; no duplicate planner or inference calls.

### P3.6 — Divergent history reconciliation

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.6.01** | W3 | Separate facade/cva_reconcile and imports/reconciliation callers into generic Maintenance comparison and Import epoch-authority portions | One operation owner, not new sync product |
| **P3.6.02** | P3.6.01 | Move common-prefix/source-head comparison and accepted history checks into Maintenance using History | No writes during probe or conflict report |
| **P3.6.03** | P3.6.02 | Preserve UUID, local/global clocks, generation, transaction timestamps and branch/source ancestry | Existing incompatible histories fail closed |
| **P3.6.04** | P3.6.03 | Use owner-normalized semantic evidence for Memory, Archive, Graph, Perception, Ego, vectors and Dream | No store-to-reconciler import |
| **P3.6.05** | P3.6.04 | Keep import-specific epoch continuation in Import through the shared lower History/owner-lifecycle proof and copy primitives | No forbidden `Maintenance -> Import` edge |
| **P3.6.06** | P3.6.05 | Preserve source-only replay, duplicates/conflicts, terminal state and compact envelope semantics | Existing conflict and retry matrices pass |
| **P3.6.07** | P3.6.06 | Guard destination promotion and cleanup using Phase 1 writer/identity semantics | Failed reconciliation leaves source and preexisting outputs intact |
| **P3.6.08** | P3.6.07 | Delete duplicate facade reconciliation implementation and enumerate import-specific consumers for next gate | R3 selected right-tail/epoch/refusal tests pass |

**Gate R3:** Generic reconciliation in Maintenance; Import-specific epoch interpretation higher and same supported refusal/conflict behavior.

### P3.7 — Import epoch, readiness and completion ownership

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.7.01** | R3 | Separate ImportEpochStore local records from Import coordinator policy and validation | No durable import store importing facade/History/Runtime |
| **P3.7.02** | P3.7.01 | Compose ImportStart/Complete, Archive receipt and source identity only from accepted owner evidence | Stable owner UUID, digest and source namespace |
| **P3.7.03** | P3.7.02 | Retain exact Insomnia, Dream, Memory and Perception readiness rules and unrelated-work exclusions | Zero-output, failed, pending, reopen behavior identical |
| **P3.7.04** | P3.7.03 | Retain explicit Freshness settlement preparation and narrow approved epoch writer scope | No autonomous Freshness worker or internal read reinforcement |
| **P3.7.05** | P3.7.04 | Retain completion marker binding, sync-before-write-admission reopen and uncertain-result idempotence | No duplicate completion after interrupted sync |
| **P3.7.06** | P3.7.05 | Maintain open-epoch write fence and existing live/import exclusion; do not add mounted import support | Runtime scheduling/features deferred |
| **P3.7.07** | P3.7.06 | Make epoch-aware divergence replay consume lower History/owner-lifecycle capabilities, not Maintenance directly | Same common-boundary and source receipt conflicts |
| **P3.7.08** | P3.7.07 | Delete internal Cva/import policy duplication while preserving supported public signatures until Phase 6 | I3 import readiness, completion, retry, replay and transformation tests pass |

**Gate I3:** Import readiness/settlement/completion exactly preserved, with no upward callbacks and atomic import admission.

### P3.8 — Audit and select minimum semantic import settlement

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.8.01** | I3 | Freeze existing CVAFRS06 bytes/codecs and per-file measurements on six originals | Completed 377-step storage refactor stays closed |
| **P3.8.02** | P3.8.01 | Measure each accepted-event, recipients/born/cohort, source-node, superseded-ID, digest and postimage field | Classify irreducible authority, compact commitment or reconstructible duplicate |
| **P3.8.03** | P3.8.02 | Map required accepted owner facts at import start/base/settlement cuts through prune/rewrite/repack | No durable physical prefix authority or dependence on removed event payload |
| **P3.8.04** | P3.8.03 | Compare at least two materially different compact proof representations | Document authenticity, read locality, historical correctness, cost and bounds |
| **P3.8.05** | P3.8.04 | Demonstrate every omitted field can be reconstructed from retained current/history owner evidence | Counterexamples include superseded event and same-turn attribution |
| **P3.8.06** | P3.8.05 | Fix canonical ordering, domain-separated digest and exact ImportStart/Complete identity binding | No weaker corruption/identity/duplicate rejection |
| **P3.8.07** | P3.8.06 | Benchmark reconstruction memory/time and size impact for each corpus while bounding scans | Do not create persistent cache or repeated inference to save storage |
| **P3.8.08** | P3.8.07 | Select minimal correct design and freeze new current-only format, migration procedure and test fixtures | S3 approved before codec changes or corpus writes |

**Gate S3:** Every settlement field has measured authority justification; at least two shapes compared; smallest independently reconstructible authenticated commitment approved.

### P3.9 — Implement settlement correction and hard-cut development corpus

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.9.01** | S3 | Implement Freshness-owned compact codec/semantic model replacing overmaterialized CVAFRS06 fields | Typed owner-local admission, one canonical current format |
| **P3.9.02** | P3.9.01 | Compose Import settlement from accepted Archive/Memory/Freshness/Import facts instead of storing duplicate postimages | Exact cohort/event causal scope and turn ordering |
| **P3.9.03** | P3.9.02 | Adapt replay and semantic validation with corruption, absent-member and superseded-event refusal | Exact historical Freshness semantics across accepted cuts |
| **P3.9.04** | P3.9.03 | Bind ImportComplete to new accepted raw settlement payload with atomic sync and idempotent retries | No unbound digest or duplicate accepted marker |
| **P3.9.05** | P3.9.04 | Adapt completed-import reconciliation and source-only epoch replay to compact evidence | No dependence on physical prefix bytes |
| **P3.9.06** | P3.9.05 | Test prune/reclaim/repack/rewrite and retired vector payload removal with corrected commitment | Eligible payload remains removable without weakening proof |
| **P3.9.07** | P3.9.06 | Create temporary **offline development-only** converter; never dual-format runtime reader | Converter explicitly scheduled for deletion P3.9.10 |
| **P3.9.08** | P3.9.07 | Convert disposable copies of six canonical REL/PHY files and compare semantic/oracle outputs with frozen originals | Record before/after hashes and only intentional wire differences |
| **P3.9.09** | P3.9.08 | Promote verified converted canonical development corpus and commit changed format/corpus in auditable checkpoint | Keep immutable original as before-image; no silent in-place overwrite |
| **P3.9.10** | P3.9.09 | Delete old CVAFRS06 runtime reader/writer, temporary converter, normalization tools and aliases | Only corrected current codec persists; hard cut complete |
| **P3.9.11** | P3.9.10 | Measure post-cut semantic settlement bytes, whole-file bytes and compression-independent effects | C3 all six history oracles and event attribution/rewrite acceptance green |

**Gate C3:** Canonical corpus migrated, verified and committed, old runtime format/converter retired, changed byte accounting measured.

### P3.10 — Hard-cut caller integration and dependency enforcement

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.10.01** | C3 | Move supported Cva/Phylactery/CLI callers to canonical History/Maintenance/Import owners | Outward wrappers may remain, policy not duplicated |
| **P3.10.02** | P3.10.01 | Delete obsolete historical/maintenance/import implementation routes and one-off forwarding layers | One canonical operation per responsibility |
| **P3.10.03** | P3.10.02 | Extend Pitlord exact source rules for History read-only, Maintenance/Import downward, no cross-owner upward reach | Negative noncyclic upward, History-write and callback cases fail |
| **P3.10.04** | P3.10.03 | Retain Phase 1 physical and Phase 2 owner guards, private data access and no internal Freshness reinforcement | No regression in earlier ratchets |
| **P3.10.05** | P3.10.04 | Run direct old-path reverse consumer search and one source-stabilized Arcana/Lexicon refresh | No scanner rescan after each task; source graph freshness verified |
| **P3.10.06** | P3.10.05 | Reconcile source findings against Phase 0/Phase 2 reviewed ledgers, no rebaseline or blanket suppression | New/changed findings individually justified, original baseline immutable |
| **P3.10.07** | P3.10.06 | Search for retired settlement runtime codec and migration tools, remove obsolete tests/shims | No dual runtime formats or ephemeral converter remains |
| **P3.10.08** | P3.10.07 | Run source DAG and relevant CLI/policy integration gate with migration window closed | D3 no orphaned implementation or unintended edge |

**Gate D3:** Production graph and targeted source guards enforce DAG/default deny; obsolete code removed and original debt provenance retained.

### P3.11 — Full acceptance and Phase 4 handoff

| Task | Prerequisite | Concrete change | Verification/deletion obligation |
| --- | --- | --- | --- |
| **P3.11.01** | D3 | Run historical cuts/prefix/read-only proof tests for REL/PHY and no-reinforcement assertions | Hashes/clock evidence unchanged by inspection |
| **P3.11.02** | P3.11.01 | Run maintenance prune/reclaim/repair/rewrite/generation/crash tests | Source/destination evidence and retention/lineage proofs pass |
| **P3.11.03** | P3.11.02 | Run import start/readiness/completion/reconciliation and corrupt/same-cut/superseded semantic tests | Accepted import behavior and idempotent completion preserved |
| **P3.11.04** | P3.11.03 | Run **old frozen semantic oracle vs new converted current corpus** for all six files | Record intentional format hash changes separately from zero semantic/history drift |
| **P3.11.05** | P3.11.04 | Run locked Rust/CLI, full integration, explicit opt-in corpus, Pitlord/Archivist/doc checks | Actual results recorded, no inferred pass for unrun suites |
| **P3.11.06** | P3.11.05 | Update current Architecture, Invariants, Storage Format, Native Import, Maintenance, Freshness, API/CLI, Limitations and Maintainer Map | No stale active codec or obsolete operational ownership statements |
| **P3.11.07** | P3.11.06 | Update parent campaign and Phase 4 Project Environment handoff, excluding P4/P5/P6 feature work | No new sync, Observations, Ego/Context, Gateway or repository VCS features |
| **P3.11.08** | P3.11.07 | Verify all gates, deletion/format migration receipts, source graph, doc links, diff/commits/platform constraints | A3 accepted only with no dual codec, no upper callback and every owner proof validated |

**Gate A3:** Whole suite, current corpus/historical oracles, docs, source/format/physical safeguards and Phase 4 handoff accepted.

## Tests, evidence and completion

**Named test seams:** existing Archive/Memory/Graph/Perception/Vector owner proofs; `historical_current_corpus_tests`, `historical_global_frontier_tests`, `historical_cut_enumeration_tests`, `historical_matrix_*`, `historical_recovery_tests`; `history_prune_tests`, `cva_reconcile_*tests`, `imports/{reconcile_tests,reconciliation_*tests,completion_tests,readiness_recovery_tests}`; `freshness_semantic_settlement_tests`, `historical_import_*tests` and `semantic_state_tests`. Confirm actual test filters/feature gates before running; no invented completed test report. Oracle comparisons must check owner current and prior cuts, local/global versions, transaction times, owner lineage, import attribution, accepted Freshness scores/event receipts, exact retry/refusal and unchanged source bytes except the approved new-format corpus.

**Suggested deterministic commands (run only when relevant, record actual outcomes):** `cargo check --locked`; `cargo test --locked --no-run -j 2`; `cargo test --locked --lib`; `cargo test --locked --tests`; `cargo check --manifest-path cli/Cargo.toml --locked`; `cargo test --manifest-path cli/Cargo.toml --locked`; `python scripts/check_physical_boundary.py`; `python scripts/check_owner_boundaries.py --self-test`; `python scripts/check_component_dag.py --self-test`; `python scripts/check_architecture.py --refresh` **once at stabilized D3**; `ddocs check --indexes --links`; `git diff --check`. The separately documented opt-in corpus command for `canonical_current_corpus_matches_frozen_history_oracle` must be executed against the converted corpus only when ready; don't confuse `--no-run` with test success. For format migration, capture old and new **different** expected wire hashes and identical semantic history oracle values.

**Execution evidence for every task:** ID, target owner, removed/added direct dependency with mode, modified code symbols, focused test command/result, format/source hash implications, deletion obligation, blocker classification, and checkpoint. This is verification, not a requirement to execute inference at each task. Retain Phase 0 and Phase 2 graph evidence and Phase 1/2 acceptance as immutable prior receipts.

**Completion criteria:** all 98 tasks and twelve gates accepted; sole History is read-only; Maintenance owns destructive policy; Import owns epoch semantics; no owner-to-facade or unlisted upward dependencies; one runtime semantic settlement format with development corpus promoted and no old conversion machinery; original and converted six-file histories equivalent at accepted semantic cuts; no unexpected source-file mutation, history/clock/provenance/privacy regression; whole Rust/CLI/source/doc suite green on the declared supported environment. Phase 3 must not declare completion based solely on an architectural graph or a field-size percentage.

**Phase 4 handoff:** Project Environment/Lore/Git repository authority and path authorization, with no new project VCS or cloud sync. Runtime/Gateway and facade convergence remain Phases 5 and 6. No reopen of the completed minimal-lossless-history 377-step program or implementation of unrelated roadmap features.

## Related docs

- [Phase 2 source/handoff](dependency-convergence-phase3-handoff-2026-10-22.md)
- [Parent campaign](dependency-convergence-architecture-refactor-2026-10-21.md)
- [Accepted DAG](dependency-convergence-target-dag-2026-10-21.md)
- [Semantic import-settlement minimization](semantic-import-settlement-minimization-plan-2026-10-21.md)
- [ADR 0041](decisions/0041-semantic-import-boundaries-owner-pruning-history-rewrite.md)
- [Current semantic maintenance](subsystems/semantic-maintenance.md)
- [Current historical Freshness](subsystems/freshness.md)
- [Current native import](native-import.md)
- [Canonical migration/format results](canonical-history-migration-results-2026-10-21.md)
- [Maintainer map](maintainer-map.md)
- [Development format hard cut](development-format-hard-cut-interruption-spec-2026-10-19.md)

## Notes

This is a specification only. It does not certify an in-flight Phase 2 worktree, run long historical tests, modify the development corpus or authorize new inference. The accepted DAG JSON, current owner contracts and previous correction/feature exclusions govern any ambiguity.