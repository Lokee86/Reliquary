# Dependency Convergence — Phase 2: Owner-Local Semantic Boundaries

Date: 2026-10-22
Status: **Proposed correction-only execution specification; not implemented**
Parent: [Dependency Convergence Architecture Refactor](dependency-convergence-architecture-refactor-2026-10-21.md)
Accepted target: [Complete Component DAG](dependency-convergence-target-dag-2026-10-21.md), machine allowlist at [tools/pitlord/convergence-component-dag.json](../tools/pitlord/convergence-component-dag.json)
Predecessor: **Phase 1 committed** at `8614547`; conceptual baseline at `6150a3f`
Successor: Phase 3 — Historical/Maintenance/Import/Reconciliation composition

## Purpose

Make the durable owner-store boundary independently correct, acyclic, downward-only and explicit, while preserving current REL/PHY semantic authority and the Phase 1 physical contract.

## Overview

This is the correction-only Phase 2 execution plan. Phases 0–1 are committed; Phases 3–7 and new feature tracks remain separate. The numbered sections below fix scope, source seams, exact tasks, tests and completion conditions.

## 1. Fixed scope

Extract **durable semantic state, replay, local validation, publication and read/proof authority** into independently usable owner-local boundaries. The immediate goal is to remove **store-to-store implementation reachthrough** and store-to-facade/Runtime dependencies, including edges that do not currently participate in a cycle. The future 34-component graph has twelve durable store families beneath `owner-lifecycle` and the semantic services. These are **responsibility boundaries within the existing library**, not twelve daemons and not a justification for one-interface-per-module abstractions.

Phase 1 established read-only caller-pulled physical traversal, OS-backed writer/snapshot ownership, nonmutating normal open and explicit repair. **Keep this unchanged.** The Phase 1 local acceptance record reports 2,119 passing tests, six-file canonical oracle equivalence and Windows-only verified locking. Phase 2 shall not silently introduce open-time repair, loosen writer exclusion or assume untested Unix results are verified.

Phase 2 changes implementation ownership and architecture only; it **does not** implement additional semantic features, replace native formats, optimize storage, introduce a model endpoint, or convert REL/PHY scope. Existing public behavior, durable identities, same-owner history, accepted semantic event/proof history and authorized privacy boundaries must be preserved.

## 2. Permanent invariants and boundaries

1. **DAG, no upward/backreach, allowlist:** all production direct edges must exist in the [accepted JSON target](../tools/pitlord/convergence-component-dag.json) and point strictly downward; absence of cycles does not authorize a cross-owner import. The present 19-area graph is an **evidence view**, never a second architecture target.
2. **Store independence:** stores may use Container and their exact allowed pure codec/clock dependencies. They must not import `Cva`, `Phylactery`, Runtime, facade orchestration, a sibling's concrete Store/codec, or privileged mutators. Each store retains its own durable codec/history/validation. A typed owner-qualified identity is not permission to fetch its target.
3. **No semantic authority transfer:** Archive owns accepted source/Episode/Echo/interaction/compaction evidence; Memory owns authored Memory state; Graph owns Graph/Communities; Perception owns Entity/Resolution and **transitional** Relationship state; Freshness owns scores/events; Vectors owns vector/profile/generation persistence; Dream/Insomnia each own durable work/receipts; Ego owns persisted semantic state; Imports own epochs, External Index its derivative source index, Maintenance its owner-local proofs. Do not make a generic super-store.
4. **Cross-owner validation is higher-level:** the **existing owner lifecycle/admitting caller** obtains exact, read-only accepted fact/proof views from each owner, checks cross-owner liveness/authorization/revision/relationship constraints, and then invokes the owning store's guarded write. The store still validates all of its **own** invariants, including version and format. Composition of already-accepted facts is permitted at a higher level; **new cross-owner History/Maintenance/Reconciliation policy and mutation coordination remain Phase 3**.
5. **No hidden callback inversion:** do not replace imports with a store invoking a facade callback, an opaque service locator or a lower-owned trait object that calls higher semantic behavior. Owner-local results/proofs travel **up** via return values, events or typed read views; invocation stays downward.
6. **Deep interfaces, no copy-paste facts:** prefer narrow concrete APIs over per-store adapter trait frameworks; provide fact/evidence records only where they avoid implementation reachthrough. Do not persist reconstructible duplicates, add a redundant read model, or expose `&mut` foreign stores as a shortcut.
7. **Owner privacy:** REL and PHY remain independent durable owners. Cross-scope references are owner-qualified; no automatic traversal of dependency RELs, no implicit sharing of PHY-private content, no cross-owner Dream edges or Communities. History and evidence do not transfer access rights.
8. **Physical and format hard cuts:** V2 current runtime format remains the only runtime format. No new on-disk tag/record or historical migration is planned. If a format change is objectively unavoidable, stop at the change gate: migrate the development corpus, verify the frozen history/oracle, commit the corpus, delete old readers/converters, and then proceed. Never add a runtime compatibility bridge by convenience.
9. **Context producer versus assembler:** Ego state is persisted here but new synthesis is not built. Context Engine alone owns all future prompt-facing selection/budget/injection. Freshness remains triggered, with non-reinforcing internal retrieval and no semantic access boosts from replay.
10. **Feature exclusion:** Observation synthesis and relational-Observation unification, cross-scope Entity identity improvements, explicit Memory commit, new Dream/Insomnia processing, provider routes, broader Context Assembly, External Sources Warlock-default duplication, historical restore and new Project VCS are all **deferred existing roadmap work**.

## 3. Concrete source evidence and target seams

These are *verified source locations and representative dependencies*, not an exhaustive renaming claim. Phase 2.0 pins actual source identities against current `main` before implementation.

| Durable owner | Current code/evidence | Allowed target and cross-owner split |
| --- | --- | --- |
| Archive/Echo/Episodes | `src/archive/{archive_rebuild,archive_store,episode_store,fragment_store,file_memory_link_store,interaction_stream*,conversation_compaction_store}.rs` | Archive codec/replay/source and link evidence local; Memory link validity consumed and checked above Archive. `archive/interaction_stream*` must not author `CvaError`. |
| Memory | `src/memory/{memory_rebuild,memory_store,memory_source_time,memory_compact*}.rs` | Authoritative body/version/provenance and mention-key facts local; external Episode/time evidence from Archive owner through caller; no Archive implementation import. |
| Entity/Resolution/Relationship | `src/perception/{entity_store,entity_resolution_store,entity_resolution_store_write,relationship_store,entity_principal}.rs` | Store-local revision/status/structured participant indexing; cross Memory/Entity existence, source spans and owner permissions checked above, without new separate Relationship semantic authority. |
| Graph/Communities | `src/semantic_graph/{graph_store,graph_store_write,graph_store_validation,graph_rebuild,community_store}.rs` | Graph local edge/tombstone/Community semantics; Memory/Entity endpoint validity and exact proofs composed above Graph. |
| Freshness | `src/freshness_storage.rs`, `src/freshness/freshness_storage_stream.rs` | Local score/event/proof replay; **remove direct Archive decoding** in Freshness by having the Archive owner expose accepted source birth/receipt facts to the caller, which then supplies only the needed facts. |
| Vectors/profiles | `src/retrieval/{packed_vector*,memory_vector*,archive_vector*,compatibility_profile*,vector_generation*}.rs` | Vector store independent of Retrieval service and Memory/Archive concrete stores. Source/version binding supplied as accepted fact; embedding generation/provider separation belongs later. |
| Dream/Insomnia | `src/runtime_inference/{dream_pair_history,insomnia/**}`, `src/facade/{cva_dream_journal_replay,cva_dream_population}.rs` | Persisted work/pair journal owner-local; Memory birth cohorts and accepted Episode proofs from higher caller; runtime scheduling/extraction remains later service work. |
| Ego | `src/ego/{ego_store,ego_compact*}.rs` | Identity/Personality/Anchor/summary persistence only; no new semantic synthesis or final context assembly. |
| Imports/External/Maintenance | `src/imports/{epoch_store,epoch_codec,completion,readiness}.rs`, `src/external_source/store.rs`, `src/facade/owner_maintenance.rs` | Durable owner-local epochs, cursors and retention proofs; import readiness/completion, reconciliation, prune/rewrite policy remain above and are handled explicitly in Phase 3. |
| Lifecycle | `src/facade/{cva_lifecycle,phylactery_lifecycle}.rs`, `src/owner_lifecycle.rs` | Existing facade remains a **higher-level caller** during transition; only owner-local data/proof/replay moves now. No new omniscient manager and no Runtime/facade import below. |

**Known traps:** `graph_store.rs` and `graph_store_write.rs` import `MemoryStore`/`EntityStore`; `entity_resolution_store.rs` and `relationship_store.rs` import foreign stores; `archive/file_memory_link_store.rs` imports `MemoryStore`; memory/vector binding code and Freshness/Archive stream decoding cross boundaries. An implicit local `use crate::{Cva,...}` is only a true architecture problem after distinguishing test-only, public adaptor, and store implementation. Treat these as **specific migrations**, not a reason to move everything under the facade.

## 4. Phase 2 interface and replay contracts

For each store family, the target interface contains only real capabilities already required by consumers:

- **Accept/replay:** `ingest(frame evidence)` and local `finish` validate the store's own schema, monotone owner versions, owner identity, current format, and publication receipts. Caller-supplied cross-owner facts are checked at admission but do not become another stored copy.
- **Read/proof:** the owner exposes a narrow bounded or version-qualified immutable view of accepted entities/source IDs/revisions/provenance, including historical/prune evidence where it already exists. Owner snapshots must not grant cross-scope access or mutate Freshness.
- **Write:** an existing semantic author/caller proves it has authorized source facts, invokes store-local admitted mutation through the Phase 1 writer contract, and commits the same publication/retry/idempotence semantics as before.
- **Replay-wide validation:** once constituent owner-local `finish` calls succeed, the existing REL/PHY lifecycle performs foreign reference validation before exposing a mounted owner. Preserve deterministic order and exact error cases. No unvalidated partial semantic mutation is permitted after an admission failure.
- **Proof usage:** historical/import/reconcile/prune callers obtain only authoritative read-only owner facts; Phase 2 does **not** change their coordination policy. Phase 3 may then replace facade composition using these seams without adding duplicate accepted histories.

## 5. Execution discipline

Each task below has a **stable ID, bounded change and acceptance assertion**. Work is linear **within each owner**; independent owner families may be implemented in parallel only after P2.1 and without conflicting facade/lifecycle/Pitlord changes. An owner cut consists of tests on the owner interface, migrating actual consumers, deleting old reachthrough, then tightening enforcement. **No indefinite double API.** Move a coherent set together in one hard-cut commit when intermediate Rust state cannot compile, but record both checks and deleted paths.

Existing canonical V2 corpus is immutable input. Preserve Phase 1 committed checkpoint and its test/architecture evidence. Do not run broad Arcana rescans for documentation work; when code reaches a ratchet gate, refresh source evidence once against stabilized authored sources. No bulk rebaseline to make a red gate green.

## 6. Linear implementation tasks — 91 bounded checkpoints

### P2.0 — Source baseline and owner/dependency inventory

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.0.01** | Phase 1 checkpoint 8614547 | Pin accepted `8614547` Phase 1 and retained Phase 0 graph; enumerate new/current cross-owner source edges, production vs test. | Before-edge ledger has exact file, symbol, direction, phase disposition; no rebaseline. |
| **P2.0.02** | P2.0.01 | Inventory Archive/Echo/Episode/interaction, Memory, Graph/Community, Entity/Relationship/Resolution durable types and codecs. | One owning component and format authority for every durable family. |
| **P2.0.03** | P2.0.02 | Inventory Freshness event/score, Dream/Insomnia work, vector/profile/generation, Ego, import epoch, external index and maintenance state. | List owner-local mutable and read/proof entrypoints and all foreign-store imports. |
| **P2.0.04** | P2.0.03 | Identify upward facade/Runtime/Cva error, helper and replay dependencies inside stores, including indirect reexports. | Each edge tagged local, cross-owner validation, higher coordination or test-only. |
| **P2.0.05** | P2.0.04 | Map existing REL/PHY `ingest`, `finish` and post-replay validations after Phase 1 cursor cutover. | Exact replay order/owner-side effects preserved; mark cross-owner checks to lift above stores. |
| **P2.0.06** | P2.0.05 | Inventory all current public, History, native-import/reconcile, prune/rewrite and runtime consumers of store internals. | For each read/mutation/proof need, document narrow already-owned evidence; no speculative API. |
| **P2.0.07** | P2.0.06 | Freeze corpus, accepted global/owner clocks, provenance, source hash and baseline tests plus existing 4,625-classified debt ledger. | B0 complete before-image and named per-owner protecting tests; no corpus mutation. |

**Gate B0:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.1 — Owner contract and hard-cut policy freeze

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.1.01** | B0 | Freeze store-level interface shape: typed `ingest`, owner-local `finish`, read-only historical/current view, validation/proof and guarded publication. | No facade-owned DTO, foreign store pointer, shared mutable registry or generic persistence trait. |
| **P2.1.02** | P2.1.01 | Define neutral scalar/owner-qualified identifiers and accepted-fact read contracts; place semantics with the producer owner. | Consumer never imports implementation internals or reconstructs another owner's truth. |
| **P2.1.03** | P2.1.02 | Split local state validation from cross-owner relation admissibility; lifecycle caller supplies accepted facts without taking over store policy. | No store->sibling edge, and no new central semantic authority. |
| **P2.1.04** | P2.1.03 | Freeze REL/PHY privacy and mount scope boundary; no implicit cross-owner reads or inherited grants. | Explicit authorized owner reference may remain unresolved without leaking private state. |
| **P2.1.05** | P2.1.04 | Freeze all replay/proof invariants: V2 current-only, append-only history and pruning/rewrite evidence, transaction/global clocks, import/Dream/Insomnia receipts. | Current V2 bytes, revisions, timestamps, semantics and six-file oracle remain unchanged. |
| **P2.1.06** | P2.1.05 | Write owner-by-owner deletion plan for old cross-store argument signatures, facade/Cva type dependencies and translation helpers. | Each removal assigned a task; no lasting dual API or compatibility shell. |
| **P2.1.07** | P2.1.06 | Define Pitlord source mapping/ratchet progression per owner: production allowlist, justified modes, error-on-unlisted and no-upward even without SCC. | B1 no new feature, cross-owner policy coordinator or speculative abstraction accepted. |

**Gate B1:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.2 — Archive, Echo and conversation/interaction owners

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.2.01** | B1 | Isolate Archive owner-local replay, format markers, fragments, Episodes and source records in `archive/{archive_rebuild,archive_store,episode*,fragment*}`. | Archive accepts physical evidence; exact replay/global and Episode provenance parity. |
| **P2.2.02** | P2.2.01 | Move `archive/file_memory_link_store.rs` Memory validation into higher authorized composition while Archive keeps accepted link storage/proof. | Archive no longer imports `MemoryStore`; dangling/retired foreign references still fail at admitting boundary. |
| **P2.2.03** | P2.2.02 | Remove facade-shaped error/type knowledge from `archive/interaction_stream*` and `archive/episode_policy.rs` owner-local implementation. | Domain errors are mapped above owner; turn/Episode/interaction sequencing unchanged. |
| **P2.2.04** | P2.2.03 | Narrow Echo, interaction streams and conversation-compaction checkpoint reads to owner-provided identity/record evidence. | Original transcript/Echo authority and compaction current state preserved without Context Engine assembly. |
| **P2.2.05** | P2.2.04 | Keep Archive-local import receipt decode and accepted record recognition with Archive; isolate external/import orchestration callsites. | Import receipt remains durable source evidence; no new import completion logic here. |
| **P2.2.06** | P2.2.05 | Define exact Archive current and historical read/proof seam consumed by Memory provenance/Insomnia, Context and later History. | Bounds/privacy/version checks; no direct raw implementation state returned. |
| **P2.2.07** | P2.2.06 | Prove Archive-only replay, normal read-only open, compaction/interaction tests and malformed/replay/refuse cases. | No Archive -> Memory, Cva or Runtime production import; exact V2 frames. |
| **P2.2.08** | P2.2.07 | Hard-cut legacy cross-owner Archive helpers and ratchet Archive logical boundary without touching Project VCS redesign. | A2 green; owner docs updated to match implementation. |

**Gate AR2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.3 — Memory and provenance owner

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.3.01** | B1 | Isolate Memory publication, revisions, lifecycle, terminal/tombstone and compact carrier validation in `memory/{memory_store,memory_rebuild,memory_compact*}`. | Accepted Memory identity, versions and current-only binary codec unchanged. |
| **P2.3.02** | P2.3.01 | Classify `memory_source_time.rs` and Memory source provenance reads against Archive authority; pass typed accepted Episode/time evidence from higher caller. | Memory no longer imports Archive implementation; exact recorded provenance and timestamps validated. |
| **P2.3.03** | P2.3.02 | Expose minimal Memory body/mention historical-key and revision/accepted-use read facts for Perception, Graph and Freshness. | Passive inspection has no Freshness reinforcement or privileged mutation path. |
| **P2.3.04** | P2.3.03 | Make cross-store application checks caller-composed and keep Memory-local field, span, revision and admission validation in Memory. | Foreign stores never gain permission to author Memory state. |
| **P2.3.05** | P2.3.04 | Preserve Memory compact/delta, grouped publication, idempotence and first-acceptance receipts with canonical codecs. | Golden V2 bytes and retry/terminal semantics survive reopen. |
| **P2.3.06** | P2.3.05 | Migrate Insomnia, external source, Dream and native-import consumers to supported Memory contracts, preserving their existing author authorization. | No caller-authored unvalidated public Memory write API, no new explicit Memory-commit feature. |
| **P2.3.07** | P2.3.06 | Delete cross-owner Memory helper imports and ratchet `memory-store` allowed dependencies. | M2 Memory fixtures, source proofs and REL/PHY coverage green. |

**Gate M2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.4 — Perception Entity/Relationship/Resolution state

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.4.01** | B1 | Separate Entity format/replay/owner-local revision/merge/terminal validation in `perception/{entity_store,entity_rebuild,entity_*codec*}`. | Entity ID, revision and canonical-identity evidence unchanged. |
| **P2.4.02** | P2.4.01 | Extract Entity principal/profile/candidate policy from durable Entity store where it reads higher coordination or PHY profile. | Entity store does not import Phylactery/Profile facade; existing profile privacy retained. |
| **P2.4.03** | P2.4.02 | Split `entity_resolution_store` local retry, fingerprints, statuses and accepted revisions from Memory mention-key and Entity target admission. | Store takes typed accepted facts, not `MemoryStore`/`EntityStore` objects. |
| **P2.4.04** | P2.4.03 | Relocate retired-reference canonicalization and cross-owner mention/Entity checks to existing higher authorized owner coordination. | Existing explicit Phase 1 repair retained; normal open stays byte-pure. |
| **P2.4.05** | P2.4.04 | Split `relationship_store` persistence, participant index and revisions from Entity/Memory identity/access/visibility checks. | No Relationship store import of EntityStore/MemoryStore; owner-qualified provenance still verified above. |
| **P2.4.06** | P2.4.05 | Preserve provisional Relationship persistence only; do not invent independent long-term proposition/synthesis/lifecycle authority. | ADR 0034 relational Observation direction remains deferred; current V2 records unchanged. |
| **P2.4.07** | P2.4.06 | Define owner-local Entity, Resolution and Relationship accepted read/proof contracts for Graph, Dream, Perception and History. | Cross-owner participants stay owner-qualified, authorization-aware, optionally unresolved. |
| **P2.4.08** | P2.4.07 | Migrate Entity/Resolution/Relationship callers and import/reconciliation inspection to those contracts, without moving service scheduling into store. | Public behavior, ambiguity, terminal status and owner-privacy rules preserved. |
| **P2.4.09** | P2.4.08 | Ratchet Perception store boundary and run REL/PHY resolution/relationship compact, merge, historical and privacy tests. | E2 accepts all three durable families with zero foreign-store/upward imports. |

**Gate E2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.5 — Graph/Community store boundary

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.5.01** | B1 | Split `semantic_graph/graph_store{,_write,_validation}.rs` Graph-local structural/rule validation from `MemoryStore` and `EntityStore` queries. | Graph store owns relationship semantics but cannot directly import foreign store implementations. |
| **P2.5.02** | P2.5.01 | Lift cross-owner endpoint existence, liveness, participant and exact version admissibility to authorized caller using Memory/Entity accepted facts. | Missing, stale or private endpoints still refuse before Graph publication. |
| **P2.5.03** | P2.5.02 | Divide `graph_rebuild.rs::finish` into owner-local finalization and higher replay-wide referential validation. | Replay retains complete invalid-history detection before public owner acceptance. |
| **P2.5.04** | P2.5.03 | Keep typed Graph endpoints, local versions, tombstones and relationship authority unchanged. | No Observation, duplicate policy or cross-REL graph authority introduced. |
| **P2.5.05** | P2.5.04 | Keep Community derivation in Graph family while moving `community_store` foreign endpoint validation to a typed Graph-local evidence seam. | Leiden/routing snapshots and historical Community reconstruction remain exact. |
| **P2.5.06** | P2.5.05 | Migrate Dream/Freshness/retrieval read paths to Graph owner read/proof API rather than private indexes. | No silent Freshness reinforcement or cross-owner privacy bypass. |
| **P2.5.07** | P2.5.06 | Ratchet Graph/Community imports and run replay/compact/graph-query/Leiden/publication and historical proof suites. | G2 source no Graph -> Memory/Entity store reference; invariant and corpus parity. |

**Gate G2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.6 — Freshness owner and Archive evidence seam

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.6.01** | B1 | Identify every `freshness_storage.rs` / `freshness_storage_stream.rs` direct Archive payload decode and semantic source dependency. | List exact accepted event kinds, identity, global clock and historical import attribution requirements. |
| **P2.6.02** | P2.6.01 | Define one Archive-owned accepted source/receipt evidence read contract for Freshness, supplied by caller/replay composition. | No decoded Archive payload or Archive store imported into Freshness store; no duplicate durable event history. |
| **P2.6.03** | P2.6.02 | Replace Freshness-origin birth extraction/replay with caller-supplied accepted Archive facts, preserving chronological and source identity gates. | REL current and historical event semantics identical, including retry, Dream birth cohort and receipts. |
| **P2.6.04** | P2.6.03 | Separate Freshness persistent store validation/effects/proof from Memory/Graph eligibility/adjacency selection done in triggered policy layer. | Freshness scores remain owner-local; no independent worker or Graph ownership. |
| **P2.6.05** | P2.6.04 | Keep lazy decay, one-boost accepted-use and bounded propagation semantics; internal source/proof reads stay non-reinforcing. | No metadata lookup increments score or changes event identity. |
| **P2.6.06** | P2.6.05 | Verify imported Freshness event scopes and CVAFRS06 semantic settlement remain exactly current; provide read/proof seam needed by Phase 3. | Do not minimize settlement bytes, change codec or migrate corpus in Phase 2. |
| **P2.6.07** | P2.6.06 | Ratchet Freshness store and run accepted-use, Dream linkage, historical, import, replay and performance-characterization suites. | F2 no Freshness -> Archive implementation dependency; REL parity and PHY clock exclusion. |

**Gate F2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.7 — Vectors, profiles and generation persistence

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.7.01** | B1 | Inventory packed vectors, Memory/Archive bindings, compatibility profiles and vector generations under `src/retrieval/**`. | Assign each persisted binding/profile/generation to `vector-store`, independent of `retrieval`. |
| **P2.7.02** | P2.7.01 | Separate `memory_vector_store` and `memory_vector_rebuild` MemoryStore validations from vector-local binding/wire invariants. | Foreign Memory source existence/version supplied as typed accepted facts. |
| **P2.7.03** | P2.7.02 | Separate `archive_vector_rebuild` and generation validators' Archive/Memory object reads from vector store replay. | Vectors retain exact owner/local generation identity, sizes and profile matching. |
| **P2.7.04** | P2.7.03 | Define compact vector/profile/generation read and publish contracts; keep model execution in Switchboard and generation workflow in `vectors` service. | Store itself never calls a provider/Runtime; no embedding redesign or new generation policy. |
| **P2.7.05** | P2.7.04 | Move direct retrieval/semantic similarity callers onto vector owner read contracts without merging lexical and vector state. | Current compatibility and deterministic rankings preserved. |
| **P2.7.06** | P2.7.05 | Verify retired/physical-reclaimable vector references, historical generations and owner identity through existing replay/prune tests. | No orphan admission, new format, or forced vector regeneration. |
| **P2.7.07** | P2.7.06 | Ratchet vector store against Memory/Archive/facade import and execute packed/vector/profile/generation suites. | V2 demonstrates persistent vector owner independent of retrieval orchestration. |

**Gate V2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.8 — Dream, Insomnia, Ego, Import, External and Maintenance state

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.8.01** | B1 | Make Dream pair/Begin journal and first-cycle durable evidence `dream-store` owner-local (`runtime_inference/dream_pair_history`, facade journal replay). | Retain first-mutation identity, pending Begin and full replay proof; scheduling not moved. |
| **P2.8.02** | P2.8.01 | Lift Dream population Memory/Archive/Insomnia cohort resolution into higher caller using read proofs, not store internals. | No Dream store -> other owner; first-pass/retry/duplicate semantics unchanged. |
| **P2.8.03** | P2.8.02 | Make Insomnia durable work/attempt/lease/receipt store owner-local, separate from extraction execution and model endpoints. | Owner work lifecycle and successful completion evidence unchanged. |
| **P2.8.04** | P2.8.03 | Move Insomnia completion cross-store Memory publication admission to existing higher semantic author with typed store operations. | No new explicit Memory commit behavior; retries/crash idempotence preserved. |
| **P2.8.05** | P2.8.04 | Confirm Ego Identity/Personality/Anchors/summary **persistence only** and owner-local revision/provenance reads, no Runtime/Ego synthesis extraction. | No future Ego generation, no Context Engine injection or derived semantic content created. |
| **P2.8.06** | P2.8.05 | Separate import epoch/marker storage/validation from native-import readiness, reconciliation and source/settlement cross-owner policy. | Current CVAFRS06 authority and format remain; Phase 3 owns reconciliation/minimization. |
| **P2.8.07** | P2.8.06 | Keep external-index durable cursors/hash provenance separate from host transcript adapter and Insomnia processing. | Current reference-only external source contract unchanged; Warlock duplicate-source redesign deferred. |
| **P2.8.08** | P2.8.07 | Keep maintenance metadata/proof state owner-local; actual prune/reconcile/repair/rewrite policy stays above stores for Phase 3. | Container explicit repair and Phase 1 lock contract unchanged. |
| **P2.8.09** | P2.8.08 | Ratchet Dream/Insomnia/Ego/Import/External/Maintenance stores against unrelated owners, facade and Runtime imports. | O2 owner identities/receipts/replay and test-only exceptions classified. |
| **P2.8.10** | P2.8.09 | Run Dream/Insomnia/import/Ego/external-index/prune/repair and REL/PHY recovery regressions; freeze old paths to delete. | No owner-to-owner loops or duplicate accepted evidence storage introduced. |

**Gate O2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.9 — REL/PHY owner lifecycle and caller migration

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.9.01** | AR2, M2, E2, G2, F2, V2, O2 | Keep Cva and Phylactery existing entrypoints as temporary **top-level composition**; use owner-local `ingest/finish` and read/proof contracts. | Do not introduce new global mutable lifecycle registry or rewrite Phase 5 runtime. |
| **P2.9.02** | P2.9.01 | Move replay-wide cross-owner referential checks after owner-local finish into existing lifecycle composition, preserving exact order. | All accepted invariants checked before exposing Cva/PHY and before permitted write. |
| **P2.9.03** | P2.9.02 | Preserve physical source lease, read-only snapshot and no-on-open mutation while binding owner-local views. | Ordinary REL/PHY open immutable, writer admission distinct from semantic authority. |
| **P2.9.04** | P2.9.03 | Move direct facade operations that are logically owner-local into their owner without moving public facade API or cross-owner policy. | Facade becomes delegation/composition; tests pin signature, error and behavior. |
| **P2.9.05** | P2.9.04 | Migrate existing Graph, Perception, Dream/Insomnia, Freshness, Vectors and archive consumers of foreign private struct state to accepted read facts. | No sibling/store reachthrough, no secret private-owner fan-out. |
| **P2.9.06** | P2.9.05 | Ensure cross-REL/PHY owners aren't auto-opened, auto-linked or shared; preserve authorization and unresolved owner-qualified IDs. | ADR 0034/0037/0038 current multiplexer and privacy invariants pass. |
| **P2.9.07** | P2.9.06 | Build source-to-34-component mapping for all touched owner files and discriminate test-only Rust imports from production. | Source-level allowlist ratchet can reject concrete foreign imports without false positive baselines. |
| **P2.9.08** | P2.9.07 | Run full REL/PHY replay/source/owner comparison and protected V2 before-image equivalence after all owner cuts. | L2 historical cuts, clocks, provenance and nonmutating open preserved. |

**Gate L2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.10 — Deletion, ratchet and negative acceptance

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.10.01** | L2 | Delete obsolete foreign-store argument signatures, facade/error aliases, indirect callbacks and temporary shim modules created by owner migration. | No dual owner API or generic shared trait remains. |
| **P2.10.02** | P2.10.01 | Add/tighten Pitlord source-area rules for Archive, Memory, Graph, Perception and Freshness store direction; reject acyclic upward imports. | Tests prove fixture with forbidden foreign store or Cva import fails. |
| **P2.10.03** | P2.10.02 | Add/tighten vector/Insomnia/Dream/Ego/import/external/maintenance store rules using production symbol ownership rather than path coincidences. | Target rank and exact allowlist respected; not merely reduced SCC. |
| **P2.10.04** | P2.10.03 | Check no write/repair bypass, no ordinary-open mutation, no leaking private store implementation and no Freshness boost on internal read. | Phase 1 physical boundary still green, negative behavior fixtures green. |
| **P2.10.05** | P2.10.04 | Refresh Arcana/Lexicon once after stable code, verify authored source manifest; do not invent scanner availability or reset Phase 0 graph. | Original source baseline preserved; new production edges individually classified. |
| **P2.10.06** | P2.10.05 | Reconcile owner-related 4,625 existing findings by deletion or evidence-based rebinding; refuse new unreviewed/stale debt. | Debt decreases or concretely justified unchanged; no test-only suppression of production edges. |
| **P2.10.07** | P2.10.06 | Ratchet all touched owners; demonstrate a noncyclic upward edge and owner-to-owner implementation reachthrough both fail. | D2 no masked structural violations or mutable facade dependency admitted. |

**Gate D2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

### P2.11 — Documentation, regression and formal acceptance

| Task | Prerequisite | Bounded change | Required evidence |
| --- | --- | --- | --- |
| **P2.11.01** | D2 | Update current Archive, Memory, Graph, Perception, Freshness, Vectors, Dream/Insomnia, Ego, Import and external-source subsystem docs where behavior moved. | State implemented contracts only; don't promote roadmap features into current docs. |
| **P2.11.02** | P2.11.01 | Update Maintainer Map, Architecture/Invariants, target DAG source crosswalk, storage-format/behavioral-contracts only for actual owner changes. | No unsupported format or semantic-policy claim, and no stale ownership reference. |
| **P2.11.03** | P2.11.02 | Record Phase 3 handoff with exact accepted read/proof seams for History, Maintenance, Import/Reconciliation and CVAFRS06 minimization. | Defer semantics and format cut to Phase 3 with identified input contracts. |
| **P2.11.04** | P2.11.03 | Run focused per-owner read/publish/replay/corrupt/current-format and REL/PHY authorization suites; capture commands/results. | Every moved store has owner-interface tests, not just facade smoke tests. |
| **P2.11.05** | P2.11.04 | Run locked Rust/CLI check/tests, full library and integrations, docs, fmt, source DAG, and all negative rules. | No unexplained regressions versus Phase 1 baseline; explicit platform constraints. |
| **P2.11.06** | P2.11.05 | Run six-file retained corpus historical oracle with canonical hashes, timestamps, exact semantic/history/global-clock parity. | No file format change and no accepted provenance/History regressions. |
| **P2.11.07** | P2.11.06 | Audit deletion list, updated edge ledger and source graph, index/links, diff and version control; record Phase 2 accepted checkpoint. | A2 all 12 gates passed; no new feature, compat scaffolding or incomplete owner debt. |

**Gate A2:** complete the entire block; capture code-map, tests, deleted paths and source-edge proof. Every gate is a blocking checkpoint, not a progress percentage.

## 7. Required verification and exclusions

**Functional owner tests:** before-and-after exact REL/PHY open/replay, accepted owner versions, Memory/Entity/Relationship/Graph IDs, typed endpoint/principal restrictions, Archive Episode/interaction checkpoints, Dream/Insomnia durable work and duplicates, Freshness events, accepted-use non-reinforcement, Vector profile/generation identity, Ego persisted state, native import source receipts, external-source index and maintenance/proof records. Include corrupted/unknown record rejection; mismatch must fail closed without rewriting source. Test owner-only APIs directly and existing Cva/Phylactery public paths; never replace essential semantic tests with implementation-shape tests alone.

**History/retention:** frozen six-file oracle and source hashes, historical cut/rewrite/repack/reclaim/prune, import attributed birth/settlings, owner-only read/proof and global/transaction clock accuracy. Preserve native historical import terminal/pending behavior, completion markers and original CVAFRS06. All new source-read APIs must remain non-reinforcing and writer/snapshot coherent.

**Source architecture:** physical guard; logical DAG validator; Pitlord source-policy run, code edge mapping and fresh graph. Demonstrate that `graph-store -> memory-store`, `perception-store -> memory-store`, `archive-store -> memory-store`, `freshness-store -> archive-store`, `vector-store -> retrieval`, store -> Cva and any parent/facade dependency fail even without cycles. Each owner has an explicit **allowlist**, not simply an SCC reduction. Keep historical before-image and ratchet only source-confirmed edge changes.

**Commands (run/report as available, not claims of prior success):** `cargo check --locked`, `cargo test --locked --no-run -j 2`, per-owner filtered library tests, `cargo test --locked --lib`, `cargo test --locked --tests`, `cargo check --manifest-path cli/Cargo.toml --locked`, `cargo test --manifest-path cli/Cargo.toml --locked`, `python scripts/check_physical_boundary.py`, `python scripts/check_component_dag.py --self-test`, `python scripts/check_architecture.py --refresh`, `ddocs check --indexes --links`, `git diff --check`. Use the documented **explicit opt-in** feature/ignored test for the six-file canonical history oracle, not a normal abbreviated smoke run. Never count a compiled-but-unexecuted test as a pass; distinguish original platform limits and pre-existing errors.

**Absolute exclusions:** do not implement Perception Observations, relational Observation, Ego synthesis, Context Engine future context assembly or compaction refinement, implicit Memory commit, new embedding models, new Dream scheduling/Insomnia engine, PHY Freshness policy, Warlock source duplication, new Project VCS, broad historical restore, native import settlement minimization, new format compression or new synchronization conflict resolution.

## 8. Acceptance and Phase 3 handoff

Phase 2 is complete only when **all relevant durable owner stores** replay, validate, expose accepted read/proof evidence and publish via **their own** boundaries; no owner store imports another store's implementation, higher facade, Gateway or Runtime. The exact read/write/cross-owner validation separation must be demonstrated by concrete tests and source checks, including noncyclic upward dependencies. REL and PHY normal opening remains nonmutating. Public interfaces and current V2 owner semantics, history and privacy remain intact. The major owner-store SCC has materially collapsed without inventing a super-store.

The Phase 3 handoff is an **owner contract register** naming exact functions/types for History read-only reconstruction and Maintenance/Import/Reconciliation policy, with a list of remaining higher-service edges and unchanged coordination. In particular, the [semantic import-settlement minimization plan](semantic-import-settlement-minimization-plan-2026-10-21.md) receives authoritative Freshness/Memory/Import read/proof seams but **no Phase 2 field removal or CVAFRS06 format migration**. It must still eliminate reconstructible duplication in Phase 3 through separately measured changes and the required development-format hard cut.

## Related docs

- [Parent dependency-convergence campaign](dependency-convergence-architecture-refactor-2026-10-21.md)
- [Phase 1 verified physical boundary](dependency-convergence-phase1-verification-2026-10-22.md)
- [Target DAG](dependency-convergence-target-dag-2026-10-21.md)
- [Maintainer map](maintainer-map.md)
- [REL/PHY scope ownership](subsystems/scope-ownership.md)
- [Perception current owner](subsystems/perception.md) and [Perception future plan](perception-subsystem-plan.md)
- [Freshness current contract](subsystems/freshness.md)
- [Ego current state](subsystems/ego.md) and [Ego synthesis plan](ego-web-synthesis-plan.md)
- [ADR 0034 cross-owner relationships and privacy](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0037 Reliquary ownership](decisions/0037-reliquary-context-memory-project-environment.md)
- [ADR 0038 Gateway/Instance multiplexing](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Semantic settlement follow-up](semantic-import-settlement-minimization-plan-2026-10-21.md)

## Notes

This specification describes **future correction work**, not a claim that Phase 2 owner separation has already landed. Deliberately deep owner-local interfaces are preferable to proliferating small facade-forwarding traits or retained compatibility scaffolding.