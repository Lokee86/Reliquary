# REL/PHY owner-session design — source-grounded next step (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status: proposed target design, not approved for implementation.** This is the next design stage after [system/storage options](architecture-redesign-foundation-2026-11-03.md), derived from [implementation inventory](current-system-inventory-2026-11-03.md) and live source. Product priorities: maintainability and extensibility; existing behavior and performance are preservation constraints. **Naming policy:** use REL rather than CVA in target code, as recorded in [ADR 0043](decisions/0043-rel-terminology-and-wire-identity.md). No Rust code, stored data or current format has been changed by this document.

## 1. Verified present architecture and its costs

- `src/facade/cva.rs` defines one `Cva` with **35 directly exposed `pub(crate)` fields**. These include Container, Archive, Memory, Graph, derived Community/lexical indexes, Insomnia, Dream, import, Freshness, Ego, Entity/Relationship, vector, project-file and external-source state. `freshness_due` and `freshness_notifications` additionally mix runtime scheduling/cache with persistent owner state.
- `src/facade/phylactery.rs` defines a separate `Phylactery` with **19 directly exposed fields**, reusing many semantic domains without Archive/conversation/Insomnia work and without REL-only Freshness activity.
- `src/facade/cva_lifecycle.rs` and `phylactery_lifecycle.rs` reconstruct mutable stores and validate records; the actual long `Cva::open` function handles grouped Insomnia/external Memory publication, Freshness, Dream and cross-owner proofs in Root. The lower `reliquary-owner-lifecycle` crate currently does header inspection, physical replay, record classification and partial lower-owner replay, **not** the canonical open aggregate.
- `src/runtime_inference/interaction_runtime.rs` owns `cva: Cva`; its `accept_turn` and session scheduling directly mutate/sync this aggregate. The real Gateway in `src/runtime_inference/runtime_host_gateway.rs` owns REL and PHY executions through Root runtime types. An `Arc<Mutex<InteractionRuntime>>` is held by the REL execution. The declared dedicated Gateway crate is still empty.
- `Cva::append_node_with_principal` writes Archive and triggers Freshness accepted-activity work in the same broad receiver. `Phylactery::tombstone_memory` mutates Memory and updates Graph exclusion. These two methods demonstrate why a blind `Cva -> Rel` rename **cannot** be an ownership redesign.
- Existing `RelMetadata`, `RelSemanticCut`, durable owner IDs and `FileKind::Reliquary` already use domain-oriented naming. `CvaError`, `cva_*` modules and many exposed internal references are the legacy naming surface.
- Existing code enforces meaningful semantics: REL-only Archive provenance, typed PHY privacy, physical publication envelopes, grouped completion, local semantic clocks, explicit maintenance/import fences, historical unavailability, exact identity. Preserve those contracts.

## 2. Compare two concrete owner-session interface shapes

### Shape A: a single large REL object with many direct methods

```rust
pub struct Rel { /* container + all REL stores, private */ }
impl Rel {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RelError>;
    pub fn accept_turn(&mut self, turn: AcceptedTurn) -> Result<TurnReceipt, RelError>;
    pub fn publish_memory(&mut self, command: MemoryCommand) -> Result<MemoryReceipt, RelError>;
    pub fn historical_memory(&mut self, key: HistoricalMemoryKey) -> Result<HistoryResult, RelError>;
    // ... more public domain and workflow methods ...
}
```

- **Strength:** simple for callers, minimal type plumbing, easy private storage and write sequencing.
- **Failure mode:** a growing public API carries Domain, Gateway and maintenance operations on one receiver. Future Observation, Ego and provider features inflate the object; direct in-place workflows recreate the old Root knot under a new name.
- **Verdict:** inadequate as the entire design, though a small public product facade over a true owner session remains useful.

### Shape B: opaque, owner-local session + typed scoped reads/commands + named application workflows

```rust
pub struct RelSession { /* private per-owner durable state, physical lease */ }
pub struct PhySession { /* private PHY state, typed owner kind */ }

impl RelSession {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RelOpenError>;
    pub fn identity(&self) -> RelIdentity;
    pub fn read_memory(&mut self, query: MemoryRead) -> Result<MemoryView<'_>, MemoryError>;
    pub fn read_archive(&mut self, query: ArchiveRead) -> Result<ArchiveView<'_>, ArchiveError>;
    // Typed read and commit operations, not a generic mutable stores() bag.
}

// Above the owner, a named coordinator prepares and commits one use case:
pub fn accept_conversation_turn(
    owner: &mut RelSession,
    authorized: AuthorizedTurn,
) -> Result<AcceptedTurnReceipt, IngestError>;
```

Illustrative shapes only; names, borrowing, error types and signatures are **not** implementation-ready. A `MemoryView<'_>` cannot outlive its owner lock. Model inference operates on bounded extracted value data, not a borrowed view across `await`. Some hot-path reads may require scoped iterators instead of owned allocations.

- **Strength:** per-owner authority is narrow and stable; domain semantics remain independently deep; use cases are identifiable and testable; adding a new service need not add dozens of unrelated methods.
- **Tradeoff:** cross-domain publication needs dedicated named operations, not unrestricted `with_stores_mut`. Rust borrowing around Container and multiple stores needs a concrete field-level proof.
- **Verdict:** **recommended for detailed interface design**. May use a tiny public `Rel` API delegating into the sole `RelSession`; **no second mutable aggregate**.

Do not add `dyn Store`, a generic registry, universal mutable `OwnerContext`, trait per domain, or a universal coordinator simply to make Shape B compile. Deep explicit operations and a few real cross-domain workflows are preferable.

## 3. Proposed source-to-target field ownership

These are **dispositions** for all 35 REL and 19 PHY fields; physical placement (which crate) is deferred until contracts pass.

| Existing field(s) | Target authority / lifecycle | Rationale or constraint |
| --- | --- | --- |
| `container` | Private REL/PHY session: physical Container lease | Shared physical engine; separate typed owner sessions. No consumer receives raw write access. |
| `import_write_admission` (REL only) | REL exclusive admission scope, owned by session | Only authorized import coordinator obtains/consumes a scoped write permit; no general bypass. |
| `archive`, `conversation_compactions`, `interaction_streams`, `project_history`, `project_files`, `rel_metadata` (REL only) | Archive, conversation-evidence and project-reference domains within REL state | Project VCS remains independent; durable source/stream semantics must not move into Gateway caches. |
| `echo` (REL only) | Independent Echo evidence owner within REL | Never turn into a duplicate Archive transcript. |
| `memories` | Owner-local Memory state in each session | Exact identity, provenance/revision and tombstones remain Memory-owned. |
| `lexical_index` | Owner-local rebuildable query acceleration | No semantic authority; preserve distinction between REL Archive indexing and PHY retrieval rules. |
| `graph`, `communities` | Graph state + derived Community projection per owner | Graph mutation validates typed endpoint identity; Communities must track exact current Graph/Memory population. |
| `duplicate_index`, `dream_pairs`, `processing_epochs` | Domain-specific Dream/processing evidence owned per REL/PHY session | Durable owner-local receipts and history; Dream classifier/scheduler stays outside persistence. |
| `insomnia` (REL only) | Insomnia durable work/attempt/completion owner | Completion and Memory/Freshness grouped publication is an application transaction, not an arbitrary direct Store call. |
| `packed_vectors`, `memory_vectors`, `archive_vectors` (REL only), `compatibility_profiles`, `vector_generations` (REL only) | Vector/profile/generation owners and derived indexes | Preserve embedding generation, profile validation, accepted row identities and explicit arch-vs-memory applicability. |
| `ego` | Ego durable owner per scope | Future Ego synthesis is a semantic service, not this store's responsibility. |
| `entities`, `entity_resolutions`, `relationships` | Entity/mention resolution evidence and transitional relational records | Future relational Observations converge per ADR 0034; existing Relationship data must not be silently discarded. |
| `freshness`, `dream_freshness` (REL only) | Freshness owner and Dream first-cycle durable evidence | Maintain current accepted-use, first-cycle, settlement history and replay rules. |
| `freshness_due`, `freshness_notifications` (REL only) | **Runtime/service scheduling and ephemeral notification state**, rebuilt/derived from durable owners | Do not move higher-level `FreshnessDueDispatcher` into the low owner session. This is the confirmed Root back-edge. |
| `import_epochs` (REL only) | Import epoch durable owner under REL session | Readiness/reconciliation is an upper named workflow, not store-local inference. |
| `owner_maintenance`, `rewrite_lineage` | Owner-local retention/lineage evidence | Full rewrite, promotion, conflict reconciliation, and cross-owner policy remain explicit upper workflows. |
| `external_index` (REL only), `external_publications` | Scope-appropriate external evidence/index owners | Keep authoritative publication receipts distinct from disposable search index and host-owned reference transcripts. |
| `profile` (PHY only) | PHY profile durable owner | No fake REL Archive/episode fields; cannot rely on another owner's turns. |

Not every field deserves a crate. All fields must have **one** canonical mutable home, but several can form cohesive domain modules. No public direct field accessor. A private session implementation may borrow multiple owned fields internally for a verified atomic operation.

## 4. Typed session and command contracts

**Identity and scope**
- `RelSession` opens only `FileKind::Reliquary`; `PhySession` only `FileKind::Phylactery`. Durable owner ID, physical head, local revision clocks and read/write lease status are explicit, not ambient.
- A session is the only mutable in-process aggregate for its mounted owner ID. Gateway caches only handles, not duplicate domain authority. Unmounted/offline owner opens are separately authorized from live gateway operations.
- REL dependency topology and PHY selection belong to an authenticated Instance/Gateway request, never to the per-file session's global mutable selection.

**Read surfaces**
- Domain read queries produce scoped owner-qualified facts, bounded read cursors or borrowed views. Historical requests specify exact cut type and preserve `PayloadUnavailable`/tombstone distinctions.
- Gateway/semantic services use authorized projections; no `&mut Container` or `&mut MemoryStore` escapes to consumers.
- Cross-owner reads fan out on independent owner locks; collect only permitted facts into model/service input. Do not hold a REL+PHY lock during inference or compose private PHY facts into REL-visible stream events.

**Single-domain commit**
- Domain owner validates its mutation semantics; session validates typed file, expected owner ID/cut, physical lease and publication envelope. Commit returns exact receipt, sync guarantees and retry identity.
- `PhySession` must reject REL-only source payload and cannot claim REL turn-based Freshness. Existing PHY direct Memory API behavior is migration baseline even if future explicit Memory-commit policy changes it.

**Grouped owner-local commit**
- A **named** operation combines separately validated facts (for example Insomnia completion + Memory birth/route + Freshness birth) into the existing synchronized grouped carrier. Failed admission makes no partial semantic publication.
- Grouped replay and historical reconstruction must reconstruct all participating owner states from the original carrier and preserve exact identities and historical availability. No generic transaction manifest demanded of ordinary individual writes.

**Cross-file commit**
- REL-to-PHY routing is a typed, idempotent **two-owner workflow**, not a fictional multi-file atomic transaction. Revalidate grants, prepare one write at a time, retain exact owner-qualified references, and recover interrupted acknowledgments from durable operation receipts.
- No single model call is made while holding either owner's durable mutation lock. Authentication is rechecked at commit boundaries.

**Open/recovery, history and maintenance**
- Typed `open` validates current header and scans in physical order. Concrete domains validate own records; grouped carriers and cross-domain references are authenticated at the correct upper assembly layer *before* a writable session is admitted.
- An invalid complete frame fails closed. Incomplete tails require explicit repair. Unknown/retired formats reject before mutation; never add permanent legacy normal-open dispatch.
- Maintenance produces a validated transformed copy, then separately verifies/reopens before promotion. No arbitrary mutable owner bag available to pruning/reconciliation code.

## 5. Exact integration crosswalk for the first implementation design

| Current behavior and anchor | Proposed new owner / coordinator | Acceptance proof needed |
| --- | --- | --- |
| REL open/initialize, `cva_lifecycle.rs` | `RelSession` owner composition, lower domain replay and explicit grouped proof coordinator | Open current corpus, replay grouping, same source IDs/cuts/bytes, no upper-service backreach |
| PHY open/initialize, `phylactery_lifecycle.rs` | `PhySession`, shared physical helpers but typed PHY domain set | Reject REL frames, retain private provenance and all PHY owners |
| Conversation accepted turn, `cva.rs` + `interaction_runtime.rs` | Ingestion workflow over `RelSession` then bounded runtime Freshness scheduling | Same turn receipt, activity clock, idempotent acceptance and sync |
| Insomnia publication, `insomnia/processor/application.rs`, `insomnia/completion.rs` | Insomnia semantic evaluator + named REL/PHY publication workflow over owner commits | Exact grouped records, external PHY handoff/retry and birth proof |
| Dream/Perception/Freshness publishing, Root runtime/facade modules | Independent semantic evaluators + domain-specific commit workflows | Identical graph, identities, receipts, due scheduling and history |
| Historical reads, `facade/historical_*`, `reliquary-history` | Read-only historical composition over owner-local proofs | Exact cut/unavailable/tombstone oracle, no semantic service import |
| Import/rewrite/reconcile, `imports/`, `rewrite/`, `facade/cva_reconcile_*` | Explicit offline/exclusive owner workflows; lower-domain validators | No loss of source proof, current-format hard cut, preserved promotion safety |
| Gateway/instance, `runtime_host_gateway*`, `interaction_runtime.rs` | Gateway holds per-owner session handles and typed workflows, no `Cva` fields | Concurrent principals/branches, revocation, queue/recovery, bounded locks |
| Public API / CLI, `src/lib.rs`, `cli/src` | Canonical `Rel`/REL API and unchanged `Phylactery` public identity | Consumer compilation and conformance; no obsolete `Cva` exports/aliases at final gate |

## 6. Next validation required before freezing the architecture

1. Make a **method-by-method** classification of all public `impl Cva`/`impl Phylactery` methods, including cross-module extensions, by owner domain vs workflow vs facade vs Gateway. The 35/19 field classification above is a starting inventory, not that method-level crosswalk.
2. Prove one concrete **hot-path** read, one **grouped Insomnia** commit, one **cross-REL/PHY** routing operation and one **historical** read under Shape B using actual current signatures/borrows. Do not assume `MemoryView` is feasible until proven.
3. Decide the exact private mutable owner aggregate implementation and physical crate map only after those proofs. New Observation and Ego features are extension exercises, not implemented in this refactor.
4. Establish focused tests for open/recovery, grouped publication, historical cuts, PHY privacy, multi-instance Gateway, provider-outside-lock behavior and frozen performance workloads.
5. Sequence the **REL naming hard cut** against the owner-session migration and independently verified current-only wire/corpus conversion. See [ADR 0043](decisions/0043-rel-terminology-and-wire-identity.md).

**Stop rule:** No implementation extraction or mass rename based on this draft; the whole system architecture remains under design. In particular, renaming the current `Cva` struct to `Rel` with its fields still `pub(crate)` would preserve the defect.
