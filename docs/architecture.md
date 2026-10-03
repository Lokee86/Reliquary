# Architecture
Parent index: [Documentation index](INDEX.md)
## Purpose
This document owns the current Reliquary Memory v2 implementation boundaries, state ownership, lifecycle, and code map.
## Overview
A homogeneous Reliquary `.rel` is one physical file containing the same explicit concrete owners as the former CVA format. Legacy `.cva` and typed REL files remain readable as compatibility inputs:
```text
Reliquary (internal compatibility type: Cva)
├── Container
│   ├── durable typed owner UUID
│   └── global_version: u64
├── Archive
│   ├── archive_version: u64
│   ├── conversation/session-local ancestry
│   ├── source turns + native attachments + embedded files
│   ├── explicit file-to-Memory links
│   └── immutable Episodes
├── Memories
│   └── memory_version: u64 + immutable revisions
├── EntityStore
│   └── entity_version: u64 + durable canonical referent revisions
├── Graph
│   ├── graph_version: u64 + append-only relationship mutations
│   ├── persistent typed SemanticNodeRef ↔ dense NodeId catalogue
│   └── active topology/traversal via Arcana graph kernel
├── CommunityStore
│   └── owner-local derived Leiden snapshots keyed to graph_version
├── InsomniaOperational
│   └── queue / priority / leases / retries / compact completion receipt
├── EchoStore
│   └── ordered turn-attached reasoning / tool / activity evidence
├── PackedVectorStore
│   └── immutable numeric matrices
├── MemoryVectorStore
│   └── immutable (profile, MemoryBodyId) -> packed row bindings
├── ArchiveVectorStore
│   └── immutable row -> FragmentId bindings
├── CompatibilityProfileStore
│   └── immutable vector-space compatibility contracts
└── VectorGenerationStore
    └── vector_version: u64 + current generation per profile
```
`Reliquary` is the public product-facing type; `Cva` remains the compatibility/internal implementation name. It owns physical composition and the single Container handle. New REL files carry authoritative Reliquary file identity plus a durable UUID in the header and use canonical owner ID `rel-<uuid>`. Optional `RelMetadata` stores a human-facing type label and directed REL dependency IDs; the type label is organizational metadata only, while dependency topology is the explicit context-inheritance substrate. Legacy typed REL headers retain their historical owner prefixes for compatibility. Filenames may supply display names to the host but are not identity. Physical maintenance follows the same ownership boundary: `reclaim_storage` computes owner-aware reachability above Container, rewrites a separate file, relocates only physical `ObjectRef` metadata, and reopens the result before success; Container itself remains unaware of semantic reachability.

The same low-level container now also backs a distinct `Phylactery` type for user-global Identity state:

```text
Phylactery (.phy)
├── Container
│   └── global_version: u64
├── Memories
│   └── memory_version: u64 + immutable revisions
├── EntityStore
│   └── entity_version: u64 + durable canonical referent revisions
├── Graph
│   └── graph_version: u64 + semantic topology; published relations currently Memory-to-Memory
├── CommunityStore
│   └── owner-local derived Leiden snapshots keyed to graph_version
├── PackedVectorStore
│   └── immutable numeric matrices
├── MemoryVectorStore
│   └── immutable (profile, MemoryBodyId) -> packed row bindings
└── CompatibilityProfileStore
    └── immutable vector-space compatibility contracts
```

Phylactery does not own Archive/history, Episodes, Files/attachments, Insomnia state, Archive Vectors, Vector Generations, interaction-stream checkpoints, or the Archive lexical index. It does participate in the same disposable owner-local **Memory lexical index** as REL: the index is built lazily from current non-archived Memory title/content after reopen, then follows later Memory revisions incrementally by `memory_version`; it is never persisted in the PHY. Direct PHY Memory publication rejects REL-local provenance fields. A PHY Memory may nevertheless retain an identifier-only `MemorySourceRef` back to an originating REL; that reference does not copy or transfer the source records into PHY.

PHY principal identity and presentation metadata are deliberately separate. The header UUID derives the stable `phy-<uuid>` principal identity. `PhylacteryProfile` stores one optional mutable `display_name` as latest-wins owner metadata. Ego `Identity.name` remains the label of a selectable persona/identity document, not the human principal's durable name. When a REL has materialized the matching reserved principal Entity, RuntimeHost synchronizes current profile values only into that Entity's aliases/summary; canonical name and EntityId remain derived from `phy-UUID`, and principal aliases remain excluded from ordinary Entity candidate indexing.

Ego persistence is a separate owner-local layer over the same container, not another Memory category. PHY may durably store multiple stable Identity documents with exactly one active selection whenever the Identity set is non-empty, plus Personality, Anchors, and one cached Memory-Web synthesis; REL may store Anchors and one cached Memory-Web synthesis but rejects Identity/Personality records. Identity activation is independent of Identity content revision: creating the first Identity activates it, later identities remain inactive until explicitly selected, and swapping the active Identity writes only selection state. Legacy single-Identity PHY records reopen as one active `Default` Identity. The Ego store has an independent contiguous mutation version plus optimistic per-record revisions. Personality may record the PHY `memory_version` from which it was synthesized, and each cached web synthesis records its owner-local source `memory_version`; these are deterministic stale-input watermarks, not new semantic clocks or model-confidence scores. Actual synthesis inference, cross-owner context assembly, scheduling, and prompt injection remain higher-level Ego/runtime responsibilities.

Machine-local application configuration is a separate owner:
```text
ReliquaryConfig
└── reliquary.cfg
    ├── archive.fragments
    ├── retrieval.default
    ├── models.general
    ├── models.insomnia (optional; falls back to general)
    ├── models.insomnia_metadata (optional)
    ├── models.dream (optional; falls back to general)
    ├── models.embedding
    └── credential.<id> (AES-256-GCM)
```
`reliquary.cfg` is current-state configuration only. It is not contained in a `.rel`, consumes no semantic clocks, and has no append-only/history semantics.
## Responsibilities
### Durable owner identity
Every newly created REL/PHY receives a UUID in the container header before creation returns. Current RELs derive canonical owner ID `rel-<uuid>` and PHY derives `phy-<uuid>`. Copies, moves, renames, cloud-conflicted replicas, and reconciliation repacks preserve that UUID; intentionally new durable owners generate a new UUID. REL `type_label` is optional persisted organizational metadata and has no storage/retrieval authority. Directed REL dependencies are persisted separately from identity and are resolved by a multi-REL host/Ego context policy.

### Reliquary comparison and reconciliation
`Cva::compare` requires the same durable owner ID on both files and compares physical chunk streams to find their longest common prefix. `Cva::reconcile` builds on that detection seam: identical/strict-extension cases copy the complete valid side, while true divergence semantically replays the complete left history plus the right divergent tail. If the right/candidate side contributes new semantic state, the result is a fresh workspace CVA with reallocated global/Archive/Memory/Graph version numbers while preserving each replayed mutation's known transaction timestamp; legacy untimestamped mutations remain untimestamped; if every candidate record is already semantically present, the fresh probe is discarded and an exact copy of the canonical/left CVA is returned instead. This no-op path prevents repeated cloud-conflicted copies from rewriting the workspace and avoids any reconciliation-receipt metadata inside the CVA. Replay is owner-ordered: Archive source/Files/Branches/Episodes/Fragments first, Memory revisions next, then Graph transactions after their Memory endpoints exist, followed by cross-owner file-to-Memory links and durable Insomnia receipts; interaction-stream state and compatibility profiles are also preserved through their own seams. Graph single-edge and atomic batch mutations are republished through the Graph owner with fresh destination clocks and dense node mappings. For each oriented `(source, target, kind)` identity, divergent active-state histories are prefix-compared so already-absorbed/stale mutations are skipped and only a novel right suffix is replayed; non-prefix histories fail closed. Graph topology, Dream's duplicate index, and community snapshots remain derived and rebuild from merged relationship authority. Memory mutation/revision invariants and completion identity similarly decide duplicates versus genuine conflicts. Immutable Fragments are revalidated and retained, and the disposable lexical index rebuilds lazily from a changed merged state. Packed vectors, Memory/Archive vector bindings, and Vector Generations are retired only for a real changed merge; `canonical_change_required` and `vector_rebuild_required` report those outcomes separately. Semantic replay collisions are normalized into public `CvaReconcileConflict` values carrying the stable owner identity and the competing state needed by a host UI; corruption and transport failures remain ordinary errors. `Cva::reconcile_and_promote` skips replacement entirely for semantic no-ops and otherwise uses a hidden sibling candidate, pre-promotion fingerprint check, synced recovery copy, atomic path replacement, post-replacement reopen/sync, recovery on finalization failure, and cleanup. It detects ordinary canonical changes during preparation but does not introduce a general interprocess writer lock or cloud-provider discovery. Cloud providers remain responsible for storage transport and conflicted-copy preservation. See [ADR 0019](decisions/0019-cloud-backed-cva-reconciliation.md).

### Repository correlation and repository-backed files

Repository-associated RELs own the semantic side of the boundary between Reliquary history and the associated project repository. `ProjectHistoryStore` persists append-only `ProjectRevisionCorrelation` records that associate a `RelSemanticCut` with one exact Lore/Git `ProjectRevisionRef` plus the repository-management policy. The store does not own repository commits, trees, branches, working-tree mutation, or project-file ancestry; Lore/Git remains authoritative for those concerns.

The correlation sequence is REL-local bookkeeping and consumes no semantic version. Repository kind and durable repository ID are invariant across one REL correlation history. The selected project subtree and management policy may evolve within that repository. Reopen validates sequence continuity, repository-relative path normalization, and stable repository identity. Strict-copy reconciliation preserves the history; divergent semantic repacks compare repository revision/policy histories independently of obsolete branch-local REL cut numbers and emit a fresh final correlation only when the merged canonical state changes.

`ProjectFileStore` owns immutable `FileId -> ProjectFileRef` bindings for transcript/provenance attachments whose bytes are authoritative in the project repository. It validates repository-relative paths and subtree containment, rejects conflicting rebinding, and consumes no semantic clock. Archive continues to own attachment/file identity and source provenance; the Project-file binding supplies the exact historical external content location. Reliquary therefore does not duplicate repository-backed payload bytes into `CVACONT1` merely to preserve transcript provenance.

Reliquary's Project Environment owns repository discovery, Lore/Git integration policy, repository bootstrap/adoption, stable repository-identity validation, managed/manual checkpointing, Git snapshot creation, upload materialization, exact historical repository reads, repository-root resolution, and correlation of those operations with REL semantic history. A host application supplies only the machine-local project directory association and invokes the typed runtime-host operations. Legacy names such as `warlock.repositoryId`, `.warlock/`, `warlock/uploads/`, and `refs/warlock/snapshots/*` remain compatibility identifiers in existing project repositories rather than ownership markers. See [ADR 0027](decisions/0027-warlock-project-repositories-and-reliquary-storage-boundary.md), [ADR 0028](decisions/0028-project-folder-and-repository-bootstrap-contract.md), and [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md).

### Local configuration
`ReliquaryConfig` owns one purpose-built replaceable config file. Logical objects have stable keys and typed payload schemas; replacing a setting rewrites one complete current file image through a temporary-file + atomic-replace lifecycle. Unknown objects are preserved so the object vocabulary can expand. Ordinary objects are unencrypted; credential objects are authenticated encrypted payloads.
### Repo-local CLI
`cli/` is a separate, non-installed Cargo package that depends only on the public library API. It owns argument parsing, secret prompting, human-readable rendering, and explicit operator command dispatch only; it owns no CVA/config/auth/retrieval or configured-runtime semantics and can be removed or detached without changing the core package. `ConfiguredRuntime` is the library-owned seam for configured finite Insomnia/vector execution, while library constants own defaults exposed by optional CLI flags.
### Model switchboard
`ModelSwitchboardConfig` owns machine-local endpoint selection for explicit model capabilities. The current capabilities are `General`, `Insomnia`, `InsomniaMetadata`, `Dream`, and `Embedding`; the providers are `OpenAiCodex` and `OpenAiReady`. Main Insomnia and Dream may each have dedicated routes and otherwise resolve to General. Fixed-group metadata classification remains optional and uses `InsomniaMetadata` only when configured. Post-wording **Entity-mention enrichment** is required by the configured/runtime-host Insomnia workflow: it uses the metadata route when present and otherwise falls back to effective main Insomnia. Lexical Memory routing is deterministic and therefore uses no model route. When owner routing is enabled, ownership classification likewise prefers the metadata route and otherwise falls back to effective main Insomnia. Each configured route carries a stable `CredentialId`. `ModelCapability::route_key()` is the canonical external capability-key vocabulary used by host adapters.

`ReliquaryRuntimeRoutes` is the long-lived host boundary. A host may supply exact provider-adapter endpoints for General, Insomnia, Insomnia metadata, Dream, and Embedding, but Reliquary applies capability fallback and chooses which endpoint each background worker uses. `ReliquaryRuntimeHost` therefore never receives or mutates a single generic inference endpoint: Insomnia uses effective main + optional metadata/ownership routes, Dream uses its effective Dream route, and vector workers use the embedding route. `ConfiguredRuntime::runtime_routes()` builds the same route bundle from `reliquary.cfg`, so the standalone configured path and embedded hosts share the same runtime composition rules.

`CredentialsConfig` owns decrypted in-memory credentials loaded from encrypted `credential.<id>` config objects. Constructing `ModelSwitchboard` validates that every selected route resolves to a credential of the provider's required auth kind. The switchboard can then produce request auth: bearer API key for `OpenAiReady`, or bearer ChatGPT access token plus optional `ChatGPT-Account-ID` for `OpenAiCodex`. `OpenAiReadyEmbeddingEndpoint` consumes the validated embedding route/auth, performs direct OpenAI-compatible HTTP embedding requests, preserves input order across concurrent batches, and normalizes output according to the declared route contract. `OpenAiReadyGeneralEndpoint` uses strict JSON-schema `response_format` for ordinary General/Insomnia OpenAI-compatible calls, while its dedicated Dream constructor forces exactly one named function/tool call whose parameters are the requested schema and accepts only that function's JSON arguments. This keeps Dream's post-model relation/direction/evidence validation strict even on providers that do not reliably honor `response_format` schema shape. Provider/model/URL/credential choices remain routing policy and do not enter Compatibility Profile identity or decide vector compatibility.
### Master key
`MasterKeyStore` owns retrieval/creation of the 256-bit key that protects credential objects. The current `JsonMasterKeyStore` is a temporary implementation that stores plaintext `reliquary.master-key.json` beside `reliquary.cfg`; production storage will move behind the same ownership boundary to the operating-system credential store. Credential ciphertext uses AES-256-GCM with fresh nonces and object-key-bound authenticated data. The master key is machine-local and is never CVA semantic state.
### Container
Container owns the fixed header, opaque length-prefixed chunks, `ChunkRef`, file I/O, sync, the single physical reopen scan, and file-global monotonic version tickets. Current headers distinguish `FileKind::Reliquary` from `FileKind::Phylactery`; both use semantic-scope discriminator `0`. Legacy typed REL discriminators remain readable compatibility data only. Organizational REL type is stored in `RelMetadata`, not file identity. Global version remains the canonical semantic mutation order and current `CVAVERS2` tickets additionally carry owner-local wall-clock transaction time. `transaction_time_ns(version)` exposes the exact persisted mapping when known; `version_at_or_before(transaction_time_ns)` resolves the highest provable contiguous global-version prefix whose included transaction timestamps are known and no later than the requested wall-clock cut. It cannot skip an unknown or future-timestamped earlier version to expose a higher version. Legacy `CVAVERS1` and pre-`CVAINSC4` embedded version ranges remain explicitly untimestamped rather than being assigned fabricated historical times; both `CVAINSC4` and current `CVAINSC5` carry an explicit transaction timestamp for their embedded local Memory-version range. Transaction time is knowledge/system time and does not replace source chronology or Chronos valid-time interpretation.
### Generic review evaluation

`src/staleness/` owns typed source/value validation, pure same-source due comparison and checked fixed/proportional interval arithmetic. It imports no semantic facades, reads no hidden wall clock and persists no state. `src/facade/review_clocks.rs` supplies owner-validated REL activity and REL/PHY Memory-version readings through existing owner accessors. These implemented typed source-clock helpers do **not** implement the newly accepted per-Memory Freshness engine or any durable generic schedule store. Ego's future summary-refresh eligibility/completion remains a separate consumer concern. The planned Memory owner will hold the signed Freshness projection, its event propagation and derived lifecycle boundaries; readiness and queues remain independent. See [Rust API](api.md), [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md) and [the unified implementation plan](generic-staleness-implementation-plan.md).

### Archive
Archive owns source-history semantics: content-addressed content bytes, immutable conversation nodes, native source-turn attachments, standalone embedded file manifests, conversation-local parent ancestry, branch/session-head revisions, derived conversation/leaf inventory, exact-leaf transcript resolution, owner-wide historical lexical search, transient exact-branch lexical search, fragments, immutable deterministic Episodes, explicit file-to-Memory links, the dense Archive watermark, historical branch lookup, and Archive-owned derived indexes. A source turn with attachments enters through one `IncomingTurn` ingestion boundary: body and attachment bytes may be staged as content-addressed backing objects, but the node, attached file manifests, and source provenance become semantically visible together under one Archive publication. Source attachment provenance is therefore part of the source event, not a generic association operation. Later file-to-Memory relationships remain explicit stable-ID links and do not transfer file ownership to Memories.
Archive also owns the disposable REL logical activity projection. Both Node and IngestedTurn acceptance use the same preflight/accounting module. A first accepted exact `user` or `assistant` identity advances a checked REL-local count in Archive publication order; compatible replay retains its first position without a tick, while noneligible nodes occupy a zero-valued ordinal slot. NodeIndex alone resolves logical identity. Node/activity capacity and immutable/attachment constraints are checked before publication; the activity tick installs only after accepted Node/attachment insertion. The projection records sparse version transitions without new persisted bytes and reconstructs through the existing validated version-linked rebuild path. Cva exposes current count, first-position and historical-cut reads; cuts use binary search over sparse transitions and reject versions above Archive head. Runtime receipts consume the same typed Archive acceptance through the internal Cva ingest seam and return only after sync succeeds; see [the activity-clock plan](rel-activity-clock-implementation-plan.md). Accepted activity is not proof of recent project development.

Archive also supplies activity preservation comparisons to existing post-reopen gates. Physical repacks and copy-only reconciliation require identical activity history; fresh migration requires identical eligible identity order, and divergent reconciliation requires the left history followed by novel right identities in destination order. Replayed histories can rebase Archive versions, so their numerical cuts are not portable without an explicit mapping. These comparisons add no second eligibility owner or persisted state.

`archive_version` is a whole-Archive mutation cut. It is not conversation ancestry.

Conversation inventory is derived from current Archive nodes: every durable leaf is exposed, and transcript resolution requires the caller to supply the exact leaf node ID. This adds no persistent conversation-head record and does not silently select among branches. The owner-wide `search_archive` path lazily uses the disposable lexical index over persisted Archive Fragments and hydrates ranked source windows without embedding or semantic mutation. It is distinct from the hot live-history search, which walks only the selected root-to-leaf ancestry, derives ordinary Fragment windows transiently (including the current closed tail), and applies the existing lexical scorer. It creates no Fragment/index/semantic state and never broadens into sibling leaves or other conversations.

The current source-ingestion execution boundary has three library-level seams. `Cva::ingest_turn` is the Archive-facing primitive. `InteractionRuntime` is the transport-neutral synchronous session/message mechanism. `ReliquaryRuntimeHost` is the in-process long-lived environment owner used by hosts such as Warlock: in addition to background semantic workers and retrieval, it owns the managed conversation/session lifecycle above `InteractionRuntime`. Each mounted REL owner has at most one managed active session. The host closes/finalizes that session before starting or reopening another, owns exact-leaf reopen/resume-pending state, generates managed message/stream identities and timestamps, performs message ingestion plus repository-backed attachment rollback as one transaction, and owns assistant stream checkpoint/interruption/completion semantics. Mount clears stale durable conversation-active flags left by process loss; unmount closes/finalizes the managed session and clears its active flag before returning the REL. The graph-aware `ReliquaryRuntimeHost` mounts multiple independent REL owner executions and one optional user-global PHY. Activation selects semantic primacy without destroying unrelated owner executions; the PHY remains host-owned across active-REL switches and is attached/detached only at the graph-host boundary. Session identity maps to Archive conversation identity, message identity maps to node identity, the current durable session leaf becomes the next message parent, normalized `User`/`Agent` roles map to Archive `user`/`assistant`, and source success is returned only after `Cva::sync()` completes. Model and embedding calls run outside owner-local runtime locks; claiming/snapshotting, bounded evidence hydration, routed Memory publication, vector commits, and retrieval hydration acquire only the relevant REL/PHY locks for bounded operations. Route endpoints may be replaced while the graph host remains alive.

The graph-aware runtime host also owns **cross-owner semantic visibility and composition**. Ambient semantic work never means “all mounted owners”: it is the active REL's deterministic dependency closure plus the attached active-user PHY. Unrelated mounted siblings remain isolated from ambient Memory/Archive retrieval. Explicit owner-qualified APIs may address a mounted owner directly when a caller has already selected it. Memory and Archive fan-out is executed concurrently inside Reliquary and reassembled in deterministic owner order; Knowledge mutation/read dispatch, Memory/Knowledge provenance resolution, and exact-path Echo expansion likewise resolve REL-versus-PHY ownership inside the host rather than in an application shell. This is the reusable substrate for later Ego, cross-scope Entity identity, and Relationship composition.

The same `InteractionRuntime` also exposes the bounded Project-REL semantic coordination seam through `process_background_work`: it drains Insomnia, fills missing Memory vectors, derives Dream work from newly `extracted` Memories plus cooldown-eligible active Memories, and runs bounded Dream work against its owned `Cva`. The long-lived `ReliquaryRuntimeHost` performs the corresponding continuous coordination for both its active REL and optional attached PHY. Each owner becomes Dream-eligible only after its own compatible Memory-vector binding exists. Dream owns no durable work queue, service, or process; owner-local append-only maintenance metadata records the last satisfied cooldown epoch, the last successful Dream processing timestamp, and completed unordered Memory-pair evaluations. Host inference snapshots one owner under its lock, releases all REL/PHY/runtime locks for classifier/verifier calls, then revalidates the target Graph version and all participating Memory revisions before owner-local publication. Successful processing records the current cooldown state; classifier/verifier failure does not advance it.

Episodes are contiguous ancestry ranges made from whole user-led response cycles. They are finalized by size, 15-minute configurable inactivity, finite-import end, or the narrow `create_memory` request. Finalizing an Episode never closes its conversation. No semantic topic detector participates in Episode identity.

### Memories
Memories owns authoritative working-memory revisions. One stable `MemoryId` has immutable numbered revisions; publication requires the expected current revision and a stable mutation ID for idempotent replay. Title/content bodies are content-addressed separately from revision metadata. A Memory's `MemoryBodyId` is immutable across revisions: metadata/classification/lifecycle may change, but semantic title/content cannot mutate in place. Semantic corrections create another Memory rather than rewriting an existing body. Each published revision advances dense `memory_version` and consumes one file-global ordering ticket. Memory authority does not depend on vector availability.

MemoryStore also owns optional clock-neutral `MemoryRoutingMetadata` bound to exact `MemoryId + MemoryBodyId`. It contains only verbatim **identity-bearing reusable** Entity mentions (title/content byte spans plus exact text); bare/context-only generic referents plus descriptive action/architecture phrases ending in `flow`, `lane`, `mechanism`, `request`, `response`, `seam`, `state`, or `status` are rejected before persistence. Insomnia v3-4 model output carries a transient occurrence selector so identical surface text with different semantics can resolve to the intended boundary-valid occurrence; that selector is consumed during extraction and is not persisted in `MemoryRoutingMetadata`. This metadata is derived routing input, not Memory semantic authority, Entity identity, or another revision clock. It is validated against the immutable Memory body on write and reopen, so model-produced aliases, paraphrases, or invented names cannot enter the routing record. Because Memory bodies are immutable across revisions, one exact body-bound routing record remains valid across metadata/lifecycle revisions. Legacy `CVAMRTE1` attachments remain readable; their former model-selected lexical strings are ignored during decode. New Entity-only routing attachments use `CVAMRTE2`.

REL and PHY additionally maintain a **disposable Memory lexical index** derived directly from current non-archived Memory title/content. It reuses the Archive lexical tokenizer, inverted-posting representation, bounded term frequency, and `0.85 × query-term coverage + 0.15 × frequency density` score. The index is process-local and rebuilds lazily after reopen. After that initial build, `memory_version` advancement is mutation-aware: only Memory IDs touched since the indexed version are reconsidered, existing immutable-body slots are activated/deactivated directly from revision metadata, and only a newly searchable Memory body is read and tokenized. It persists no authority or semantic version and requires no model call. `search_memories(query, limit)` exposes this exact owner-local lexical primitive for both REL and PHY; Perception lexical locality can consume the same facility instead of persisted model-generated keywords.

Reliquary Memory publication validates Archive/Episode provenance at write and reopen. Insomnia persists assistant-authored **authority provenance** in the existing `content_source_*` record fields only when the user adopts/retains that assistant proposition; separate `grounding_source_*` fields identify context used only to resolve a referent in a user-owned proposition. Grounding never supplies semantic authority.

Phylactery reuses the Memory store/codec but applies a different ownership rule: all REL-local Episode/node/conversation provenance fields remain absent, so PHY never embeds another owner's source records. Routed User Memories instead retain optional `MemorySourceRef` metadata containing only the originating REL owner ID and source identities. `source_time_ns` is resolved while authoritative source evidence is available and persists separately as semantic chronology. A PHY remains independently valid when the referenced REL is unavailable; the provenance reference may simply be unresolved until that owner is mounted.

### Entities
EntityStore is the Perception-owned durable canonical-referent owner shared by REL and PHY. Entities are referential/traversal nodes rather than propositional evidence: they canonicalize identity and connect propositions but do not themselves assert facts. An Entity has a stable owner-local `EntityId`, optimistic revision, dense `entity_version`, canonical name, normalized aliases, kind, semantic summary, mutation identity, and creation/update bookkeeping. Entity metadata may evolve without changing Entity identity. Ordinary new Entities may omit their ID and derive it from creation mutation identity. REL additionally reserves `kind="principal"` for PHY-backed human identity: canonical identity is the exact `phy-<uuid>`, internal EntityId is deterministically derived from that principal ID, malformed/mismatched principal records are invalid, and PHY rejects principal Entity storage because the PHY file is already that identity boundary.

EntityStore deliberately does **not** persist supporting Memory IDs or Memory↔Entity adjacency. Those associations belong to the shared semantic Graph under ADR 0036. Principal Entities are deliberately absent from generic canonical-name/alias indexes and reconciliation candidate generation. Ordinary candidate discovery is bounded and deterministic: a derived one-to-many normalized surface index supplies exact canonical-name/alias matches; an existing source-Memory association supplies a recovery lane; the owner-local Memory lexical index contributes Entities already associated with lexically local Memories; and first-hop Dream Memory neighbours contribute their associated Entities as structural context. These lanes remain candidate evidence only. Exact equality does not assert identity, lexical/structural context does not imply co-reference, and the final set is capped at the resolver's eight-candidate contract. When that set is empty, a separate bounded same-surface Memory lane supplies Admission context only; those Memories are not promoted into Entity candidates. Entity centroid/vector routing is intentionally not synthesized by scanning summaries or re-embedding records on demand; it remains measurement-driven follow-up work. Resolution-state records may reference only Entity IDs that already exist in the same owner. Each Entity revision consumes one file-global semantic version plus one dense Entity-local version. Legacy REL/PHY files with no Entity records reopen with an empty Entity owner.

Pass 1 first applies a deterministic owner-relative principal pre-pass: in REL, `user` / `the user` with durable source-turn `principal_id` resolves directly to the lazy principal Entity for that `phy-UUID`; in PHY the same owner-relative surface is terminally suppressed without self-Entity creation. No model call is used for this lane. All remaining mentions deliberately separate **first-seen admission** from **identity resolution**. `Entity Admission v2` handles zero candidates and decides whether the exact extracted mention is a durable referent worth creating, should remain unresolved, or should be rejected; on creation it materializes stable kind/summary metadata using the current Memory plus only clearly same-referent bounded context. The calibrated `V4` resolver handles non-empty candidate sets and decides existing identity, distinct new identity, unresolved, or reject without changing its frozen calibration payload/schema. Candidate evidence hydration prioritizes Memories already associated with the candidate Entity before incidental discovery Memories. `resolve_entity_mention(...)` composes those judgments with deterministic Entity publication, `Memory -> Entity` Graph association, mention-resolution persistence, evidence fingerprints, and partial-write recovery. The frozen 41-query zero-Entity corpus converges to the expected 25 durable Entities and persists across reopen. In the long-lived host, Perception is a separate post-Dream worker: Dream commits enqueue mentions from the processed source plus bounded affected Memory endpoints; same-surface and candidate-Entity reverse indexes provide event-driven unresolved wakeups; a one-time startup recovery scan covers work stranded before shutdown; preparation and commit hold owner locks only around deterministic reads/writes while model inference runs unlocked; unchanged evidence hashes suppress inference and stale prepared snapshots fail closed against durable owner identity plus Memory/Entity/Graph/resolution versions.

### Transitional relational-Observation persistence

`RelationshipStore` is implemented in both REL and PHY and predates the 2026-09-25 ADR 0034 semantic amendment. It remains a real durable storage owner today, but it is **not** a separate accepted semantic category beside Observation and is **not** the Arcana/Graph relation store. Its current records provide stable owner-local `RelationshipId`, optimistic revision, dense `relationship_version`, open string `kind`, canonicalized n-ary participants with optional roles, owner-qualified Memory evidence references, a bounded compact summary, mutation identity, and creation/update bookkeeping. Participant identity uses `EntityRef { owner_id, entity_id }`; evidence uses `MemoryRef { owner_id, memory_id }`. References into the containing owner must resolve to a current local object; retired local Entity IDs are rejected in favour of their survivor. References into another owner may remain dangling until that owner is mounted.

One current Relationship revision consumes one file-global semantic version plus one dense Relationship-local version. REL and PHY rebuild the store independently, include Relationship claims in global-version collision validation, preserve Relationship history through migration/reconciliation/reclamation, and keep the same participant set independently representable across owners. Direct/current publication validates against current local objects; migration/reconciliation use an internal history-replay seam so an older revision may retain an Entity reference that was valid at that point in owner history and retired only later. Final reopen validates current state and fails closed if local references are stale. When an owner merges two local Entities, locally owned participant references are revised to the survivor before retirement; foreign-owner references are not rewritten.

Under ADR 0034, this implemented store is transitional structured persistence/indexing for the **relational Observation profile**. Before semantic synthesis ships, it must converge with the Observation owner so proposition authority, support lineage, lifecycle/currentness, ambiguity, reconsideration, receptors, and Chronos semantics exist only once. Accepted design requires at least two distinct canonical Entity participants for a published relational Observation, allows either Exploitation or Exploration to derive one, and forbids relational Observations from serving as evidence/anchors for synthesizing another relational Observation. No independent Relationship semantic authority or lifecycle should be added to this store. Active-PHY plus authorized-REL relational-Observation composition also remains future work.

### Graph
Graph owns durable owner-local semantic relationship topology over typed `SemanticNodeRef { kind, id }` endpoints. The accepted node kinds are Memory, Entity, and Observation; current backed owners are Memory and Entity. Existing Memory node mappings and Memory-only mutation payloads remain readable for compatibility. Dense `arcana::NodeId` values are internal topology indexes and never replace semantic identity. Ordinary Perception topology uses directional `Memory -> Entity` `entity-association`. PHY-backed identity uses the reserved durable `Memory -> Entity` `principal-association`, indexed separately for explicit principal lookup and deliberately omitted from ordinary Arcana edge traversal. Dream retains the existing Memory-to-Memory `topical`, `factual`, `causal`, `recurrent`, `references`, `duplicate-of`, `supersedes`, and `structural-parent` families. Relation validation is producer- and endpoint-aware: Dream cannot publish Entity associations, Perception cannot publish Dream Memory relations, and every endpoint must already exist in the same durable owner.

One non-empty semantic Graph transaction advances dense `graph_version` once and consumes one file-global ordering ticket. `graph_version` remains the optimistic concurrency clock across all semantic relation families. Arcana materializes an ordinary typed semantic topology with principal-association edges omitted, plus the Memory-only projection used by existing Dream traversal, duplicate logic, Leiden Communities, and Memory retrieval. Principal forward/reverse lookup is maintained separately and invoked explicitly. `memory_graph_version` is the latest full `graph_version` containing a Memory-to-Memory relation mutation, so Entity-only Graph changes remain visible to Perception without invalidating unchanged Memory-only derived state. Reliquary owns typed endpoint identity, relation vocabulary, publication/versioning, and producer-specific validation; Arcana remains repository-agnostic topology/traversal machinery. The Graph remains owner-local and does not create cross-file edges. See [ADR 0036](decisions/0036-typed-semantic-graph-endpoints.md).

### Community snapshots

CommunityStore owns persisted **derived organization over one owner's Memory Graph projection**, not relationship authority. `Cva::refresh_communities_leiden` and `Phylactery::refresh_communities_leiden` materialize a complete partition of Memories participating in Memory-to-Memory Graph history for the current REL or PHY. Entity/Observation nodes and non-Memory relation families are excluded. Community algorithm v2 projects active oriented Memory relationships to unique undirected structural pairs, then performs a deterministic graph-local scan-and-merge: BFS scan order is split into 2,048-node leaf shards, leaf Leiden runs execute through a bounded worker pool, and their community summaries reduce through a binary merge tree. Each summary carries original node membership plus weighted coarse edges/self-loops so structural evidence crossing shard boundaries enters the first merge level that owns both endpoints. Graph records are never added, removed, redirected, or rewritten by clustering.

The reduction depth is logarithmic in leaf-shard count; total work still necessarily scans the Graph. Merge groups are irreversible within one reduction pass, so the implementation is an approximation to one monolithic Leiden run rather than a claim of mathematical identity. Synthetic sparse planted-community benchmarks are recorded in `community-scan-merge-benchmark-2026-08-27.md`; graph-local shard ordering keeps measured modularity very close to the monolithic reference while substantially reducing wall time at larger sizes.

Each snapshot carries a derived `generation`, its exact `derived_graph_version` (the source `memory_graph_version`), the algorithm version/configuration, quality value, and a complete partition of the current Memory projection nodes. Community generation is derived-state bookkeeping only and consumes no semantic/global version ticket. A snapshot becomes stale only when `memory_graph_version` changes. Refreshing an unchanged current v2 snapshot is an idempotent no-op; refreshing stale or older-algorithm state performs a complete scan-and-merge pass and appends the next derived generation. Supported older derived snapshots remain readable so an algorithm upgrade cannot make an owner file unopenable.

`CommunityId` is scoped by the durable owner UUID and exact sorted membership, so equal membership in the same owner is stable while changed membership deliberately produces a different ID. Continuity is derived separately from consecutive snapshots: predecessor/successor overlap, split/merge participation, and clear continuation are recomputed deterministically without changing exact identity. REL and PHY snapshots are strictly owner-local. Community names are separate clock-neutral metadata keyed to exact `CommunityId`; they may be Dream-generated or explicitly user-authored and never change snapshot generation, membership, Graph semantics, traversal, or identity. Dream keeps the existing four deterministic sub-centroids and selects up to eight representative Memories, targeting two nearest unique Memories per centroid; only bounded Memory title/content text—not embedding coordinates—is sent to the model. A user-authored name is authoritative and follows clear Community lineage across membership changes. Dream-generated names also follow clear lineage while the current membership remains at least 0.750 Jaccard-similar to the original naming baseline; the comparison is cumulative rather than generation-to-generation, so repeated small drift eventually makes the Community eligible for renaming. A real semantic reconciliation repack may omit community snapshots and names and rebuild them from the merged Graph; an already-absorbed no-op reconciliation preserves canonical bytes and therefore preserves existing records. See [ADR 0025](decisions/0025-owner-local-derived-communities.md), [ADR 0031](decisions/0031-dream-derived-community-semantic-names.md), and [ADR 0032](decisions/0032-community-lineage-and-name-continuity.md).

Owner-local Memory-Web retrieval now consumes Community snapshots as a deterministic routing index without making Communities semantic authority. `MemoryRetrievalIndex` is transient derived state built from one Compatibility Profile's current non-archived Memory vectors plus a current Community snapshot of the Memory Graph projection. The default profile derives four deterministic sub-centroids per Community; these vectors are routing-only and are neither persisted nor represented as Memories or Graph nodes. The validated default routes a query to four Communities, always admits vectorized Memories with no Community membership, exact-scores only that admitted Memory set, chooses four Memory seeds, and then traverses the owner-local Graph. Traversal preserves Graph depth as the primary order and uses fewer Community-boundary crossings only as an equal-depth scheduling preference.

The reusable index records Memory, Memory-Graph projection, Community, and profile-local Memory-vector binding watermarks and fails closed as stale after relevant owner or derived-vector state changes. Entity-only semantic Graph changes do not stale this Memory-only index because its traversal and routing inputs are unchanged. A convenience call may rebuild the index for one query, while long-lived runtime/Ego callers can cache it across queries and rebuild only on staleness. `GlobalExact` remains an explicit reference path, and routed queries automatically fall back to it when no current Community routing profile exists. REL and PHY build and query their indexes independently; cross-owner composition remains a higher retrieval/Ego concern. The production algorithm is the same sub-centroid implementation exercised by the five-fold 1,087-Memory validation; see [Community routing validation — 2026-08-29](community-routing-validation-2026-08-29.md) and [ADR 0026](decisions/0026-community-routed-memory-retrieval.md).

### Echo execution evidence

`EchoStore` owns durable ordered execution evidence attached to an already-identified conversation turn/message. Echo records provider reasoning summaries, commentary, exposed reasoning traces, tool-call/result evidence, and deterministic activity events when those records are available to the host/importer. Echo is not Archive source authority, is not a Memory, and cannot establish user intent merely because an earlier model produced it.

Echo records are append-only and keyed by `(conversation_id, message_id, sequence)`. Identical replay is idempotent; conflicting reuse of one sequence fails closed. Echo consumes no semantic/global clock. Migration and REL reconciliation preserve Echo independently of Archive/Memory semantic publication. Provider continuation state required only to resume a live provider loop remains outside Echo.

Echo currently provides durable storage/import evidence rather than a general cross-conversation reasoning-search service. Retrieval must remain source-scoped when exposed: upstream Archive/Memory retrieval selects the visible source first, and Echo may then expand only evidence owned by that source. Dream and Insomnia do not treat Echo as ordinary semantic/user-authoritative evidence. See [ADR 0014](decisions/0014-echo-historical-reasoning-traces.md).

### Insomnia operational state
Insomnia operational state owns finalized-Episode processing coordination rather than another semantic timeline. Every finalized Episode is work. Priority is immediate live (`create_memory`), normal live, then import/backfill, with oldest source chronology inside each class. Queue registration is idempotent. Claims use expiring lease tokens; stale tokens cannot finalize reclaimed work. Pending, processing, lease-renewal, and retryable-failure transitions are runtime-only and are re-derived as Pending after reopen when no final outcome exists. A Terminal outcome remains durable as one final work record; a successful outcome atomically publishes all newly created Memory records together with one compact Episode-completion transaction. Prior failed-attempt history is not retained as active history after success. This owner consumes no semantic version clock.

`create_memory` is not a Memory write API. It finalizes the current uncovered live Episode tail and queues it at immediate-live priority; Insomnia remains the only authority that can turn source material into working-memory revisions.

Insomnia extractor contract `v3-4` keeps semantic authority selection, fixed-group classification, wording, and Entity enrichment as separate bounded responsibilities. Pass 1 emits a clause-level authority/disposition ledger with a schema that requires every authoritative Episode user turn to be accounted for and constrains assistant-authority/grounding IDs to available Episode or bounded-evidence turns. Pass 1 owns retain/omit/supersede disposition, authority kind, lifecycle, source identity, assistant-authority identity, grounding identity, category, and type. Retained clauses are deterministically grouped only when those structural fields match. An optional metadata classifier may refine fixed-group category/type/lifecycle, then an optional dedicated ownership classifier assigns only `user` or `project`; ownership cannot change grouping, propositions, authority, provenance, category/type/lifecycle, or candidate identity. The wording pass receives those fixed groups and may return only one title/body pair for each required group key. After wording is final, a separate enrichment call sees only final Memory title/content and extracts bounded Entity mentions. Deterministic validation requires source body text, normalizes Markdown code delimiters off otherwise-valid mentions, normalizes ASCII case-only drift back to the source casing when no exact occurrence exists, and retains the most-specific span when emitted mentions overlap. Invalid or non-verbatim individual mentions are discarded without discarding valid sibling Entity metadata; structural/schema violations and hard bounds still reject the enrichment result. The pass cannot resolve/create Entities, rename referents, change the Memory, or create semantic authority. **Lexical routing is no longer model output**: full Memory title/content feeds the disposable deterministic Memory lexical index instead. Configured/runtime-host execution uses the metadata route for Entity enrichment when present and otherwise falls back to effective main Insomnia. The fixed group's `current|future|historical` lifecycle classification is persisted on the resulting Memory as `temporal_status`; it is not Dream lifecycle authority and remains separate from `lifecycle_state`. Reliquary reconstructs exact provenance bytes from the selected source IDs and includes deterministic ledger semantic material—but neither destination ownership nor routing metadata—in candidate identity so routing/enrichment does not perturb otherwise-identical Memory IDs.

The one-shot backlog worker is the current concurrency boundary for Insomnia. `Cva::drain_insomnia_backlog` accepts 1–64 workers (default 48, calibrated against the current Luna/low corpus run). During a drain, Archive semantic indexes are shared read-only while Container, Memories, and Insomnia operational state have separate locks. General-model inference holds no CVA lock. Archive body reads and durable state publication take the Container lock only for the single-file cursor/append/version boundary; Memory and Insomnia mutations additionally take only their owning store lock in a fixed Container → MemoryStore → InsomniaStore order. Insomnia claim selection uses an incrementally maintained ordered ready/retry/lease index rather than scanning lifetime work history. Failed extraction attempts retain source-priority ordering when they become retry-eligible; invalid endpoint configuration is terminal, and the configured maximum attempt count prevents a persistent provider/model failure from spinning indefinitely. The drain returns only after no pending, processing, or retryable-failed work remains.

For whole-file import bring-up, `finalize_canonical_imports_and_queue` materializes uncovered deterministic Episodes on canonical Archive branches and idempotently registers all import-origin Episodes as Insomnia work. `drain_insomnia_backlog` remains Project-only. `drain_insomnia_backlog_routed` accepts an explicit Phylactery, publishes user-owned drafts to that PHY before the REL completion receipt, and records external results as owner-qualified `MemoryRef { owner_id, memory_id }` values. Both drains own automatic Memory-Vector completion: after authoritative Episode processing finishes, they establish/reuse the supplied embedding compatibility profile and fill only missing `(profile, MemoryBodyId)` bindings in each participating owner. Embedding failure cannot roll back a published Memory; a later drain retries missing bindings even when no Episode work remains. The repo-local `insomnia run` command supplies the configured embedding endpoint to this core path and enables routed ownership with `--phy <PHY>` rather than performing a separate vectorization/export phase.

Insomnia extraction may perform one bounded read-only Archive evidence round when an explicit callback, adopted/retained assistant proposition, or genuinely unresolved referent cannot be resolved from the authoritative Episode and nearby ancestry. The model may request at most four reads: an exact `(conversation_id, node_id)` turn, a maximum-64-node ancestry range, or lexical Archive search with at most five fragment hits. Returned evidence is deduplicated and capped globally at 64 turns / 128 KiB. Historical evidence can supply an assistant **authority source** only when the authoritative user turn explicitly adopts/retains that proposition, or a user/assistant **grounding source** used only to identify a referent. Neither form of evidence can provide user authority: `source_node_id` remains a user turn inside the authoritative Episode. External provenance is accepted only when that exact historical turn was actually returned in the bounded evidence set, and it may not postdate the user authority turn. A second evidence round is rejected.

### Phylactery lifecycle

`Phylactery::create` writes typed `FileKind::Phylactery` identity with no Reliquary scope and initializes an empty principal profile, Memories, EntityStore, Graph, Packed Vectors, Memory Vectors, Compatibility Profiles, and an empty PHY Ego owner; owner-local Community snapshots are derived and appear only after explicit community refresh. `Phylactery::open` performs one container scan, rebuilds those owners in dependency order, resolves the latest valid `PhylacteryProfile`, ingests and validates optional Ego records, validates that REL-local provenance is absent and any external `MemorySourceRef` is structurally valid, validates Memory/Graph global-version uniqueness, and rejects REL or legacy CVA identity. No Archive object is constructed merely to satisfy Memory validation or to resolve an external source reference.

Phylactery still owns no Insomnia queue/Episode state of its own. Instead, an active REL's Insomnia may route fixed user-owned results into an explicitly attached PHY. User publication converts validated REL-local provenance into an identifier-only `MemorySourceRef`, strips the REL-local Episode/node/conversation fields from the PHY draft, syncs the PHY first, then commits the REL completion receipt with an owner-qualified external `MemoryRef`; deterministic mutation IDs make a crash between those two writes idempotently recoverable. Dream now runs independently inside PHY using the same owner-local Memory/Graph/vector mechanics as REL and persisted `source_time_ns` for chronology. No REL↔PHY candidate federation or persisted cross-file Graph edge is introduced; see [ADR 0024](decisions/0024-owner-local-dream-processing.md).

**Future multi-user cloud REL boundary.** A cloud-backed REL shared by multiple users must not allow synced session history to be processed against whichever PHY happens to be attached on another machine. The intended primary mitigation is **session-local Insomnia processing**: each live session should be processed on the originating user's local host while that user's PHY is explicitly bound to the session. Once the REL carries the resulting terminal Insomnia completion and deterministic mutation identity, ordinary sync/reconciliation replay is idempotent and another user's host should not re-route that already-processed Episode into a different PHY. This is a best-effort ownership boundary rather than a complete multi-user identity solution. Concurrent first processing of the same not-yet-completed Episode, offline/imported sessions, stale completion state, user switching, or a misidentified/compromised host can still leave ownership ambiguous. Before shared multi-user cloud RELs are treated as safe, the host/runtime needs an explicit session-to-user/PHY binding and must fail closed rather than publish `user` Memories when that binding cannot be established reliably.

### PackedVectorStore
Packed vectors own immutable matrix bytes and physical row representation. `VectorSchema` defines dimensions and scalar representation; rows are fixed-width and contiguous. Equal schema+bytes deduplicate.
Packed vectors do not know which Archive fragments or Memory bodies rows represent or which embedding space produced them. The same store is valid in REL and PHY.
### MemoryVectorStore
Memory Vectors are immutable derived bindings over shared `PackedVectorStore` matrices. Their durable identity is `(CompatibilityProfileId, MemoryBodyId)`, not Memory revision. Each profile/body pair may bind to exactly one packed row; metadata-only Memory revisions therefore require no vector work. A genuinely new compatibility profile may add another immutable vector for the same Memory body. Memory Vectors consume no semantic/global version clock and have no update/regeneration path.

The high-level `build_missing_memory_vectors` path verifies the endpoint against the selected profile, finds current Memory bodies without a binding for that profile, embeds only those bodies in Document mode, appends one `f32` packed matrix, and records the row bindings. Re-running it after metadata-only revisions creates nothing.

### Dream candidate retrieval

The first Dream implementation seam is read-only bounded Memory candidate discovery. `Cva::dream_candidates` and `Phylactery::dream_candidates` take an existing source Memory plus one Compatibility Profile and reuse the source Memory's already-stored Document vector; candidate discovery performs no new embedding or model call. `dream_memory_context` on each owner exposes that same context-construction path for one exact Memory without ranking, so exact-pair tuning and inspection do not need to duplicate source-time, temporal, or Graph-context logic. Internally, candidate/context construction and Memory-vector loading operate on the concrete Container/Memory/Graph/vector stores rather than on either high-level owner. The REL wrapper supplies the Archive-aware legacy source-time resolver; the PHY wrapper supplies persisted `source_time_ns` only.

The current lanes are exact cosine similarity over current Memory-body vectors, a bounded prior-semantic quota keyed to authoritative source chronology, deterministic lexical/metadata overlap, and deterministic temporal matching through the shared Chronos core. Temporal matching is based on overlapping parsed content-time anchors or identical recurrence-pattern identity; source-time proximity alone is not a match. Archived Memories are excluded. Before ranking, Dream also excludes any candidate already connected to the source by an active Graph edge in either direction and any unordered Memory pair already recorded as successfully evaluated by Dream, including prior `none` or withheld results. Those settled pairs therefore consume no bounded candidate slots on later maintenance passes. Candidates missing a vector under the selected profile may still enter through lexical/metadata or temporal lanes. Lane ranks are fused deterministically, with stable `MemoryId` tie-breaking.

The source and every returned candidate are materialized as `DreamMemoryContext`: current Memory metadata/body identity, authoritative source timestamp when one is available, derived temporal analysis, and all active Graph relationships touching that Memory. REL source chronology resolves recoverable Archive/Episode provenance first and falls back to persisted `source_time_ns`; PHY uses persisted `source_time_ns` because it has no REL-local source pointers. `Memory.created_at_ns`, `updated_at_ns`, Dream maintenance timestamps, and grounding-only provenance do not define semantic source time.

Temporal analysis is now owned by the shared Chronos core and remains derived from immutable Memory title/content plus authoritative source chronology. Dream contributes only the owner-specific source-time resolution and passes the resulting reference timestamp into Chronos. Chronos first exposes a parser-independent indication pass that preserves original byte spans and can identify temporal material even when deterministic resolution is not yet supported. A bounded temporal-vocabulary normalizer may correct high-confidence typos in a transient analysis view; authoritative source text is never rewritten, and any corrected parse evidence maps back to the exact original span. Explicit timestamps/dates/ranges/months/quarters/years and recurrence patterns are parsed deterministically. Relative forms such as `tomorrow`, `last week`, `next Tuesday`, `three months ago`, and `in two years` are resolved only against authoritative source time; month/year arithmetic preserves the calendar day when possible and clamps end-of-month/leap-day cases rather than approximating months as fixed day counts. Chronos now represents valid-time ranges separately as derived `TemporalInterval` values with optional start/end bounds: bounded `from … to/through/until …` forms have both bounds, while `since`/`after`/`starting` and `until`/`before`/`ending` produce truthful open intervals instead of sentinel dates. Bounded intervals remain mirrored as legacy `Range` anchors for Dream compatibility. Exact standalone durations are represented as quantity plus `TemporalDurationUnit`, preserving calendar units such as months/years without fixed-day approximation. Cue-bound numeric duration ranges preserve explicit lower/upper quantities, while approximate numeric and qualitative durations preserve their uncertainty rather than fabricating exact bounds. Exact event-relative offsets such as `30 minutes before cooking` are represented as derived `TemporalEventRelation` values containing offset, direction, and the unresolved textual event referent; Chronos does not invent a timestamp for that event. Meridiem-qualified standalone clock expressions are represented separately as `TemporalTimeOfDay` values with minute/second precision and are not combined with source chronology to invent a calendar date; bare colon-number forms remain unresolved pending calibration against technical notation. Recurrence supports deterministic numeric/word-number intervals and explicit alternation (`every other …`) while leaving ambiguous cadence terms unresolved. Hemisphere-qualified named seasons resolve to meteorological three-month `Season` anchors; unqualified season terms remain detected but unresolved so Chronos never inherits host locale as semantic geography. Safe loose-calendar forms cover common English month abbreviations/ordinals and reference-bound named months/relative-year dates, while locale-ambiguous numeric forms remain unresolved. Detector precision is calibrated with contextual boundary/recurrence/calendar guards and adjacency-aware fuzzy-unit correction so ordinary language/code, ambiguous names/technical terms, numbered lists, and uncontextualized four-digit dimensions do not manufacture temporal inference work. Calendar ranges are currently interpreted in UTC because source provenance has no timezone field. The analysis is not persisted and has no semantic clock. `DreamMemoryContext` carries the complete analysis, while the current classifier/verifier JSON projection remains limited to the compatibility anchor/pattern surface; interval/duration outputs are not silently added to the Dream inference prompt. `chronos::assess` additionally classifies each semantic unit as no temporal material, fully resolved, or unresolved and preserves exact unresolved indication spans. Insomnia runs that assessment over each prepared Memory using authoritative `source_time_ns`; only unresolved residue enters `TemporalInferencer`. The model returns canonical expressions or abstentions, and every non-empty answer must reparse deterministically as the same temporal indication kind before it can be published. Verified nondeterministic results persist on the Memory as body/reference-bound `MemoryTemporalInference`; deterministic Chronos products remain derived. RuntimeHost performs the temporal model call outside REL/PHY locks and revalidates the Insomnia lease before publication. Dream reapplies valid persisted inference through deterministic Chronos when deriving its temporal context.

Candidate retrieval reads the owner-local evaluated-pair ledger to suppress settled pairs but publishes no Dream maintenance state, Graph relationship, or Memory lifecycle change.

### Dream pair classification

`DreamClassifier` is the implemented Milestone B inference seam. It consumes `DreamMemoryContext` pairs or an entire bounded `DreamCandidateSet` and calls a strict structured-output `GeneralEndpoint`. Before the model call, each pair is canonicalized by stable `MemoryId` into A/B order; invoking the same pair from the opposite processing endpoint therefore produces the same model payload and cannot redefine semantic direction.

The current classifier proposes exactly one primary relation per evaluation from `none`, `topical`, `factual`, `causal`, `recurrent`, `duplicate_of`, or `supersedes`. `topical`, `recurrent`, and `duplicate_of` require `undirected`; `factual`, `causal`, and `supersedes` require `a_to_b` or `b_to_a`; `none` requires direction `none`. This one-proposal classifier contract does not yet settle whether persistent Graph state may later carry additional orthogonal relationship kinds for the same pair.

Every non-`none` proposal must contain exactly one short verbatim quote from each Memory. Continuity rejects relation/direction mismatches, duplicate/missing evidence sides, and evidence that is not literal Memory title/content text. Source timestamps and deterministic temporal anchors/patterns are supplied as context but are explicitly not proof of causality, supersession, or recurrence; attached Graph context is supplemental, not proof. Classification is transient and read-only.

### Dream independent verification

`DreamVerifier` is the implemented Milestone C second-pass seam. It receives the same canonical Memory pair plus one non-`none` `DreamPairClassification`, including the proposed relation/direction and the classifier's verbatim evidence. Pair identity is revalidated before the model call, and reverse processing order produces the same verifier payload.

The strict verifier returns three categorical signals: whether the relation is supported, whether its direction/undirected form is supported, and whether the classifier evidence supports the proposal. Each signal is `yes`, `no`, or `uncertain`; Continuity deterministically derives `reject` if any signal is `no`, otherwise `uncertain` if any signal is uncertain, otherwise `accept`. No numeric classifier/model confidence enters the verifier contract.

`DreamVerificationPolicy::default()` verifies only `duplicate_of` and `supersedes`. `broad_semantic()` additionally verifies factual, causal, and recurrent proposals for measurement; topical remains single-pass. Verification uses the Dream model route, remains transient/read-only, and does not publish Graph relationships or mutate Memory lifecycle.

### ArchiveVectorStore
Archive Vectors own one relationship only:
```text
ArchiveVectorSet
├── packed_vector_id
└── ordered FragmentIds
    row N -> fragment_ids[N]
```
Creation/reopen require an existing matrix, exact row count, real unique Archive fragments, and valid content identity. The persistent object contains no compatibility profile or Archive watermark.
The derived Archive-Vector index records the newest Archive creation version among mapped fragments. That value is not persistent identity; it supports generation coverage validation.
### CompatibilityProfileStore
A compatibility profile is an endpoint-independent contract for one usable vector space:
```text
CompatibilityProfile
├── dimensions
├── normalization
├── probe-suite version
├── compatibility-policy version
└── 4 reference vectors
    ├── 2 Query probes
    └── 2 Document probes
```
Provider, model name, route, and revision are not profile fields. They may be provenance elsewhere later, but they do not prove vector compatibility.
`CompatibilityProfileId` content-addresses the exact stored contract artifact. Compatibility itself is checked separately by embedding the fixed probes and comparing corresponding vectors with cosine similarity. Policy v2 requires every probe to reach `>= 0.9998` and requires dimensions, normalization, probe-suite version, and policy version to match. The threshold was calibrated against live same-route drift measured through OpenRouter rather than exact deterministic simulator output.
Establishing a profile probes the endpoint once. If the resulting candidate is tolerantly compatible with an existing profile, the existing profile is reused even when its exact returned floats differ slightly. Otherwise a new profile is stored. Profile creation is clock-neutral.
### VectorGenerationStore
Vector Generations own active vector-population publication:
```text
VectorGeneration
├── compatibility_profile_id
├── archive_vector_id
├── source_archive_version
├── global_version
└── vector_version
```
`vector_version` is a dense local watermark for generation publications. The newest generation for each compatibility profile is current; older generations remain retained and can be resolved at a historical vector-version cut.
Generation publication is the first vector-layer operation that consumes a CVA-global ticket. Source Archive versions cannot regress for a profile, exceed current Archive state, or predate any mapped fragment. Until quantization semantics are defined, published generations must reference `f32` matrices.
### Retrieval
Retrieval is a read-only query layer, not another database. Archive retrieval and Memory-Web retrieval are separate primitives with different populations and purposes. The hot live-transcript path is narrower still: `Cva::search_conversation_branch` searches only one explicitly selected root-to-leaf Archive ancestry using transient default Fragment windows and the existing lexical scorer. It performs no embedding, cross-conversation scan, sibling-branch scan, Fragment publication, or semantic mutation; `ReliquaryRuntimeHost::search_open_session` binds that operation to the currently open session leaf.

`Cva::semantic_search` is the low-level Archive semantic channel: it verifies the supplied endpoint against the selected compatibility profile, embeds the raw query in Query mode, resolves that profile's current Vector Generation, exact-scans its packed Fragment matrix by cosine, drops scores `<= 0`, and maps row ordinals back to Archive fragments. `Cva::search` is the Archive hybrid product path. It independently takes up to 30 lexical and 30 semantic candidates, merges by `FragmentId`, preserves a single available channel's score undiluted, otherwise combines `0.45 × lexical + 0.55 × semantic`, sorts, removes duplicate conversation/ranges, greedily penalizes overlapping fragments from conversations already selected, retains a 30-candidate diversified pool, and returns the first 10. Lexical scoring is the original `0.85 × query-term coverage + 0.15 × bounded term-frequency density` formula.

Owner-local Memory retrieval instead starts from stored Memory-body vectors. `build_memory_retrieval_index` derives a reusable transient profile from current non-archived vectors and current Communities. `retrieve_memories_with_index` routes through Community sub-centroids, performs exact fine search only in selected Communities plus the unclustered residual lane, seeds Graph traversal, and prefers Community locality only within equal Graph depth. The explicit `GlobalExact` mode and automatic no-current-profile fallback preserve the whole-owner Memory-vector reference path. Neither mode performs a model call or semantic mutation.

The current direct `Cva::semantic_search` Archive API still performs compatibility verification for each call because no shared long-lived runtime/capability cache exists yet. Memory-Web retrieval accepts an already-compatible query vector and exposes reusable index lifecycle separately so a runtime/Ego owner can cache it. Retrieval allocates no semantic versions and persists no retrieval state.
## Write lifecycles
### Archive semantic mutation
```text
append Archive payload
    ↓
allocate global version G
    ↓
allocate Archive version A
    ↓
append ArchiveRecordVersion { G, A, record }
```
An unversioned node/branch/fragment payload is inert.
### Compatibility-profile establishment
```text
embed fixed Query + Document probes
    ↓
validate dimensions/normalization
    ↓
compare tolerantly with existing profiles
    ├── compatible -> reuse existing profile
    └── none compatible -> store immutable profile
```
No semantic clock advances.
### Vector generation publication
```text
compatibility profile + packed matrix + ArchiveVectorSet exist
    ↓
append immutable generation payload
    ↓
allocate global version G
    ↓
allocate vector version V
    ↓
append generation metadata { G, V, record }
```
An unversioned generation payload is inert.
### Development generation builder
`build_archive_vector_generation` verifies the supplied endpoint against the selected compatibility profile, snapshots the current Archive watermark, embeds all durable fragments in deterministic `FragmentId` order using Document mode, writes an `f32` packed matrix and Archive-Vector binding, then publishes the generation.
## Reopen
```text
one physical chunk scan
    ├── Container framing/global-ticket validation
    └── Cva dispatches each payload
        ├── Archive (including Episodes)
        ├── Memories
        ├── Graph
        ├── Insomnia operational state
        ├── PackedVectorStore
        ├── MemoryVectorStore
        ├── ArchiveVectorStore
        ├── CompatibilityProfileStore
        └── VectorGenerationStore
```
After the scan, cross-store references are validated in dependency order. Full packed matrices and Archive-Vector mappings are not retained in steady-state indexes.
## Ordering model
Archive, Memories, Graph, and Vector Generations are independently mutable semantic domains:
```text
G100 / A700   Archive mutation
G101 / M12    Memory revision
G102 / R8     Graph relationship mutation
G103 / V20    vector generation
G104 / A701   Archive mutation
```
`G`, `A`, `M`, `R`, and `V` are ordering/watermark integers, not parent relationships. Conversation ancestry remains node-local. Compatibility profiles, vector backing objects, and Insomnia operational coordination are not semantic timeline events.
## Invariants and safety boundaries
- one physical CVA owner and one shared reopen scan;
- no generalized semantic database/root/dependency layer;
- Archive, Memory, Graph, and vector-generation local clocks remain independent;
- each global version is claimed by at most one semantic mutation;
- packed matrices, Memory-Vector bindings, Archive-Vector bindings, and compatibility profiles are immutable backing objects;
- Memory vectors bind immutable `MemoryBodyId` content per compatibility profile; Memory revision metadata cannot invalidate or refresh them;
- Archive Vectors own row-to-fragment identity only;
- compatibility profiles own vector-space compatibility contracts, not endpoint provenance;
- endpoint compatibility is tolerant probe comparison, never provider/model labels or exact probe hashes;
- generations own profile-to-ArchiveVector association, coverage, activation, and vector semantic ordering;
- generation matrix dimensions must match the compatibility profile;
- published generations currently require `f32` matrices until alternate representation semantics exist;
- generation source watermark must cover every mapped fragment;
- the semantic retrieval channel searches exactly one selected profile's current generation and never mixes profile score spaces;
- default hybrid retrieval preserves the original 30-candidate / 10-result policy and `0.45/0.55` lexical-semantic fusion;
- local configuration is current-state machine configuration, not CVA semantic state or an internal history system;
- credential objects are encrypted independently and never become CVA semantic state;
- model routes reference credentials by stable ID; executable switchboards reject missing or wrong-kind credentials;
- model-switchboard routing/authentication is machine-local integration policy and cannot establish vector compatibility.
## Architecture enforcement

Repository-local Pitlord policy maps every authored Rust file under `src/` to exactly one current architectural owner: physical container, Archive/source history, Memory authority, retrieval/vector state, semantic graph, Perception, Chronos, Ego, runtime/inference, config/security, or the public composition facade. These are responsibility boundaries inside the current single core crate, not claims that eleven crates already exist.

The physical source tree now mirrors those owners under `src/container/`, `src/archive/`, `src/memory/`, `src/retrieval/`, `src/semantic_graph/`, `src/perception/`, `src/chronos/`, `src/ego/`, `src/runtime_inference/`, `src/config_security/`, and `src/facade/`; `src/lib.rs` remains facade-owned at the crate root. This is an organizational layout inside one Rust crate, not a crate split, and the existing crate-level module names remain stable through explicit root `#[path]` declarations. Pitlord therefore owns each directory as a coarse region rather than enumerating hundreds of files. Static policy also prevents the deleted generic `WorkspaceMetadata` / `workspace_type` authority from returning outside the explicit legacy migration decoder.

The post-reorganization dependency audit does **not** justify a crate split. Raw `imports,depends-on` analysis sees one eleven-owner cycle because test and benchmark modules intentionally reach back through public facade/runtime APIs. Filtering those test/benchmark/calibration-support source files from Arcana's import evidence leaves 864 cross-owner production imports across 46 owner pairs: `container` and `ego` fall outside the remaining strongly connected component, while Archive, Chronos, config/security, facade, Memory, Perception, retrieval, runtime/inference, and semantic graph remain one nine-owner SCC. Broader call/reference analysis reconnects all eleven owners and therefore provides no stronger extraction signal. Rust import direction is the primary crate-boundary evidence; call-level relationships remain useful for architecture review but are too coupled to helper resolution and internal implementation detail to justify a hard package boundary by themselves. Reliquary therefore remains one core crate. `container` is the only demonstrated low-level extraction seam and `ego` is directionally separable at the import layer, but neither currently has an independent reuse, dependency, or compile-isolation requirement that would pay for a crate boundary. Cross-owner dependency rules should be added only when the implementation first establishes a real directional invariant worth enforcing.

`python scripts/check_architecture.py --refresh` rebuilds disposable Lexicon/Arcana graph evidence and evaluates the policy. The push/PR architecture workflow runs the same refreshed check.

## Code map
| Responsibility | Primary code |
| --- | --- |
| local configuration | `src/config_security/config*.rs` |
| detachable operator CLI | `cli/src/*.rs` |
| encrypted credentials | `src/config_security/credential*.rs`, `src/config_security/config_credentials.rs` |
| model switchboard/provider capabilities/auth binding | `src/runtime_inference/model_switchboard*.rs`, `src/runtime_inference/model_auth.rs` |
| master key / temporary key store | `src/config_security/master_key*.rs` |
| CVA composition/lifecycle | `src/facade/cva.rs`, `src/facade/cva_lifecycle.rs`, `src/facade/cva_*` |
| durable REL/PHY owner identity | `src/container/container.rs`, `src/facade/cva_lifecycle.rs`, `src/facade/phylactery_lifecycle.rs` |
| Project repository revision correlation | `src/archive/project_history_*.rs`, `src/facade/cva.rs`, `src/facade/cva_reconcile*.rs` |
| repository-backed Project file bindings | `src/archive/project_file_binding_*.rs`, `src/facade/cva.rs`, `src/facade/cva_turn_ingest.rs`, `src/runtime_inference/runtime_host_files.rs` |
| physical Container/global clock | `src/container/container*.rs` |
| Archive/history/fragments/Episodes | `src/archive/archive*.rs`, `src/archive/fragment*.rs`, `src/episode*.rs` |
| native turn/file ingestion | `src/archive/turn_ingest_*.rs`, `src/archive/source_attachment_index.rs`, `src/archive/file*.rs`, `src/facade/cva_turn_ingest.rs`, `src/facade/cva_file_memory.rs` |
| normalized interaction/runtime seam | `src/runtime_inference/interaction_model.rs`, `src/runtime_inference/interaction_error.rs`, `src/runtime_inference/interaction_runtime.rs`, `src/runtime_inference/interaction_session.rs`, `src/archive/interaction_stream.rs` |
| Echo execution evidence | `src/archive/echo_*.rs` |
| Memories | `src/memory/memory*.rs` |
| Insomnia extraction/processing/operational scheduling | `src/runtime_inference/insomnia.rs`, `src/runtime_inference/insomnia/**/*.rs` |
| packed matrices | `src/retrieval/packed_vector_*.rs` |
| Memory body/profile row bindings | `src/retrieval/memory_vector_*.rs`, `src/facade/cva_memory_vectors.rs` |
| Archive row bindings | `src/retrieval/archive_vector_*.rs` |
| embedding execution/compatibility | `src/runtime_inference/embedding_endpoint.rs`, `src/runtime_inference/openai_ready_embedding*.rs`, `src/retrieval/compatibility_profile_*.rs` |
| generation publication/history | `src/retrieval/vector_generation_*.rs` |
| exact live-branch lexical retrieval | `src/retrieval/conversation_search*.rs`, `src/archive/fragmenter.rs` |
| exact Archive semantic retrieval | `src/retrieval/semantic_search*.rs` |
| lexical + Archive hybrid retrieval | `src/retrieval/lexical_search.rs`, `src/retrieval/search*.rs` |
| owner-local Memory-Web retrieval / Community routing / traversal | `src/retrieval/memory_retrieval*.rs`, `src/facade/cva_memory_retrieval.rs`, `src/facade/phylactery_memory_retrieval.rs` |
| corpus vector/retrieval smoke | `examples/vector_generation_smoke.rs` |
## Tests

Critical architecture invariants are mapped to focused verification in [Behavioral contracts](behavioral-contracts.md). The primary owner-level suites include Container/REL/PHY lifecycle tests, Project-history and Project-file attachment tests, Archive/conversation/interaction tests, Echo tests, Memory/Graph/Community/retrieval tests, Insomnia/Dream/runtime-host tests, vector/compatibility/retrieval tests, and reconciliation/promotion tests. The [Maintainer map](maintainer-map.md) routes each change area to the narrower verification surface.

## Related docs
- [Storage format](storage-format.md)
- [Local configuration](configuration.md)
- [Repo-local CLI](cli.md)
- [Rust API](api.md)
- [Architectural invariants](invariants.md)
- [Versioning and rollback plan](version-history-plan.md)
- [ADR 0011](decisions/0011-detachable-repo-local-cli.md)
- [ADR 0013](decisions/0013-immutable-memory-vector-bindings.md)
- [ADR 0016](decisions/0016-native-product-surface-and-shared-interaction-runtime.md)
- [ADR 0017](decisions/0017-cva-workspace-and-warlock-host-application.md)
## Notes
Unimplemented behavior is tracked in [Current limitations](current-limitations.md); future implementation sequencing is tracked only in [Roadmap](roadmap.md).
