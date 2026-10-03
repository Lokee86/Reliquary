# Roadmap

Parent index: [Documentation index](INDEX.md)

## Purpose

This document owns future implementation work for Reliquary. Completed behavior does not belong here; current behavior is documented in [Architecture](architecture.md), [Rust API](api.md), [Storage format](storage-format.md), [Repo-local CLI](cli.md), and [Current limitations](current-limitations.md).

## Overview

Reliquary is the purpose-built semantic/storage/runtime library embedded by Warlock. Its semantics do not depend upward on Warlock. Project-file history belongs to Lore or Git through Reliquary's Project Environment; REL/PHY retain independent semantic state and history.

The immediate semantic sequence is now: finish cross-scope Entity identity, then Observation composition—including relational Observations—then Ego. The graph-aware runtime host, cross-owner retrieval substrate, Project Environment ownership, conversation/session ownership, Context Engine policy, and Warlock cleanup that these stages depend on are already complete.

## Current semantic sequence

1. **Cross-scope Entity identity.** The durable per-Entity global UUID substrate and legacy backfill are implemented. Complete the relational-evidence-driven cross-scope comparison/reconciliation lane: deterministic eligible-owner candidate generation, dirty triggering, pair fingerprints/receipts, repair, and Pass 1d against the graph-aware host. Calibrate on frozen Ellis and 28-day corpora before promotion.
2. **Observation composition.** Build Observation persistence, bounded synthesis, and relational-Observation specialization over stable cross-owner Entity identity, preserving active-PHY/authorized-REL privacy and exact provenance.
3. **Ego.** Implement owner-local synthesis scheduling, graph-aware context assembly, deterministic current-REL Cross-chat selection, budgeting, PHY participation, and context projection. Ego is intentionally last so it composes the completed semantic substrate rather than constraining it prematurely.

## Memory Freshness remaining integration and calibration

Connect a genuine accepted-context delivery producer and live Ego routine selection consumer to the verified library seams. Ordinary reads/search hydration remain passive. Obtain time-positioned admission/link/use evidence before historical score-trajectory calibration: the available 14-day, 28-day and Ellis fixtures supply prospective graph topology only. Complete any unresolved repository/calibration gate recorded in [core verification](freshness-core-release-verification-2026-10-03.md). Semantic relevance and import bootstrap (F8/S1–S6) remain a separate deferred milestone.

## Additional backlog

### 1. Retire duplicate project-file VCS responsibilities

Remove or simplify REL machinery whose remaining purpose duplicates authoritative Lore/Git project history.

Candidates include:

- project/blob history retained only for filesystem recovery;
- project-file ancestry, branch, merge, or replay behavior superseded by repository history;
- file-state reconstruction that can resolve through an exact `ProjectFileRef`; and
- whole-project recovery behavior already provided by the project repository.

Do **not** collapse the REL semantic timeline into project-repository history. Preserve Memory revisions, Graph versions, Episode/transcript history, provenance, semantic supersession, vector generations, crash recovery, and other semantic/database-local history.

Project creation/adoption semantics, repository discovery and identity validation, Lore/Git mutation policy, upload materialization, exact historical reads, and repository correlation are Reliquary Project Environment responsibilities under [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md). Warlock may supply machine-local project paths and coordinate/present those operations.

### 2. Re-scope REL/PHY synchronization

Reassess whole-file reconciliation around the smaller remaining problem of divergent semantic databases now that project-file history is external.

Future work should:

- exclude project-file history already authoritative in Lore/Git;
- retain owner-explicit validation for Archive, Memory, Graph, Episode, provenance, and other semantic owners;
- preserve safe promotion/recovery behavior until a simpler replacement is proven;
- treat REL/PHY transport independently from project-repository transport;
- define cross-device conflict behavior without assuming project-repository ancestry resolves semantic conflicts; and
- determine whether cloud-drive conflicted-copy transport remains sufficient or a coordination layer is justified.

The current reconciliation API remains compatibility behavior; do not expand it into a second project VCS.

### 3. Complete the live runtime seam

Finish the host-facing runtime behavior that is not yet covered by the in-process `InteractionRuntime` / `ReliquaryRuntimeHost` boundary:

- adapter reconnect/resume and adapter migration around durable session/message cursors;
- normalized tool, session, and generated-artifact events without transport-specific semantic authority;
- explicit cancellation and shutdown behavior for long-running inference work;
- capability caching where safe;
- bounded memory-control operations needed by hosts; and
- runtime status/diagnostics sufficient for Warlock to expose failures and recovery state.

Do not turn Reliquary into an independently deployed service merely to host these capabilities. Any future IPC/server boundary requires a separate architectural decision.

### 4. Complete the workspace management API

Expose the remaining concrete owner operations Warlock needs without creating a generalized mutable semantic object layer.

Needed surfaces include:

- broader conversation/session management where current transcript primitives are insufficient;
- file inventory, import/export, source provenance, and project-file reference inspection;
- Memory inventory, provenance, lifecycle inspection, and permitted manual lifecycle actions;
- health, verification, statistics, and diagnostics;
- selective import/export of user-owned semantic state; and
- safe unlink/removal only after retention/history semantics are defined.

The management layer composes existing owners; it does not become a new semantic owner.

### 5. Production import and interoperability adapters

Add normalized ingestion for historical and external interaction sources:

- production ChatGPT import;
- Claude/provider export import;
- Codex/Hermes and other structured agent-session imports where available;
- ACP interoperability over the same normalized live interaction seam;
- deterministic re-import/idempotency rules;
- explicit handling of edits, replacements, deletions, and provider-specific branches; and
- artifact provenance that distinguishes the initiating interaction, produced artifact, and artifact evidence.

ACP remains an adapter, not a canonical storage schema. See [ADR 0015](decisions/0015-acp-inline-interaction-stream.md) and [ADR 0016](decisions/0016-native-product-surface-and-shared-interaction-runtime.md).

### 6. External canonical-transcript integrations

Add the reference-only external integration mode defined by the [External conversation index integration plan](conversation-index-integration-plan.md).

This is additive to native Archive-backed operation. For hosts that already own canonical conversation history, Reliquary should:

- consume a durable/reconstructable host change feed asynchronously;
- store stable external message IDs, content hashes, offsets, vectors, cursors, and derived semantic metadata without copying canonical transcript bodies into Archive;
- hydrate source text only transiently through the host's authorized source API;
- add a generalized identifier-only external provenance lane rather than inventing Archive Episode/node identities;
- make lost/corrupt derived indexes rebuildable from host canonical history;
- route external-source semantic extraction through shared Insomnia semantics without persisting a duplicate source transcript;
- return source references from derived search so the host remains responsible for authorization and canonical hydration; and
- expose semantic compaction as an optional proposal service whose result the host validates and commits.

The first target is Hermes, but the Reliquary core contract must remain framework-neutral. Hermes-specific transport belongs in an adapter.

### 7. File usability

Add product-facing file operations without reintroducing REL-owned project-file history:

- standalone add/import and export through management surfaces;
- user-visible path/folder/tree organization;
- rename/move behavior that preserves historical reference meaning;
- extraction pipelines for supported document/file types;
- file-content indexing and retrieval separated from filename metadata; and
- generated-artifact provenance and lifecycle policy.

### 8. Runtime and security hardening

Complete production hardening:

- OAuth token refresh;
- operating-system credential-store backed master-key persistence;
- default OS application/config locations;
- provider retry/backoff and rate-limit adaptation;
- explicit provider/model/credential/reasoning fallback routes per capability;
- runtime observability without leaking secrets or user content;
- crash-safe restart/recovery for host-owned background work; and
- authentication/authorization only if a future local IPC/API boundary is introduced.

### 9. Someday: configurable VCS management

Allow project-history backend selection and project-history management policy to vary independently rather than permanently equating Git with hands-off operation.

Target policy shape:

- backend remains `lore | git`;
- management becomes an explicit project policy such as `manual | assisted | automatic`;
- Git commit, push, and worktree management remain separately configurable capabilities rather than one broad automation switch;
- a practical Git progression may therefore range from observe-only, through managed local checkpoint/commit with human push, to explicitly authorized managed commit + push;
- Lore may remain the integrated/default managed backend while still satisfying the same higher-level revision/provenance contract.

Managed Git must preserve repository ownership and unrelated user state:

- never implement automation as an unconditional `git add .`;
- record the operation's base revision and capture only the operation-owned delta;
- preserve unrelated pre-existing dirty files and the user's live index;
- prefer plumbing/temporary-index techniques when needed to commit an owned delta without perturbing unrelated staged state;
- do not label dirty working-tree bytes as immutable `HEAD` content; exact provenance must distinguish working state from committed revision state;
- make repository mutation operations idempotent with stable operation IDs and payload hashes so retries cannot manufacture duplicate history;
- keep `push` an independently authorized escalation beyond local commit;
- keep worktree creation/reuse/removal as a separate opt-in policy, with repository identity based on lineage rather than machine-local checkout paths; and
- never remove dirty or otherwise unreconciled work as part of automatic cleanup.

Above the backend seam, Reliquary should consume the same durable revision abstraction regardless of provider: repository identity, exact revision/file resolution, working-state capture when authorized, diff/change attribution, and `ProjectRevisionCorrelation`. Managed Lore may materialize a Lore revision while managed Git may materialize a Git commit; semantic ownership remains in REL/PHY either way.

Recovery must also be backend-neutral. If repository capture and semantic correlation do not complete atomically, persist enough idempotent operation state to reconcile the exact repository revision later rather than inferring from whatever the current checkout happens to contain.

This is deliberately deferred product work, not a requirement for the current Project Environment cutover. The current conservative Git behavior remains valid until an explicit managed-Git policy and its dirty-state, index-isolation, retry, recovery, and worktree tests are designed and accepted.

## Scope and ownership evolution

Reliquary already has typed durable scope identity; future work is about how those owners are composed and governed.

Near-term scope work should:

- expose editing/inspection surfaces for the already-implemented REL dependency topology without weakening cycle rejection, dependencies-before-dependent ordering, or sibling isolation;
- keep durable ownership separate from retrieval visibility and mutation authority;
- define scope creation/editing and hierarchy-management surfaces;
- extend learned-state routing beyond the current conservative User/Project boundary only when the destination authority rules are explicit;
- keep governed Organization state distinct from learned observations; and
- define explicit cross-file Memory/source export and lineage semantics.

ADR 0034 now defines relationship-specific knowledge as **relational Observations**, not a Connection REL class or a sibling Relationship semantic object. The existing sparse `RelationshipStore` is implemented in both REL and PHY with owner-qualified Entity participants, Memory evidence, revisions/local clocks, reopen, local-reference validation, Entity-merge retargeting, migration, divergent reconciliation, and physical reclamation. Treat that implementation as transitional relational-profile persistence. Remaining work is to converge it with the Observation owner before synthesis, then implement unified Observation lifecycle/provenance/receptors plus active-PHY/authorized-REL relational-Observation composition/privacy. Existing legacy Connection-typed REL identity remains compatibility-only.

## Reliquary / Phylactery product transition

Remaining product/compatibility work:

1. warn when filename hints disagree with authoritative internal type/scope;
2. define cross-file Memory/source export and lineage semantics between REL and PHY; and
3. decide whether the internal/back-compat `Cva` terminology should eventually be removed from public-facing APIs and documentation.

Do not implement `.phy` as a Project Reliquary with provenance fields merely made nullable. Shared mechanics and separate semantic validation remain required.

## Insomnia follow-up

Routine prompt tuning on the adversarial fixture remains frozen. Future work is validation or a new measured capability boundary:

- implement the [explicit Memory commit plan](insomnia-explicit-memory-commit-plan.md), upgrading the existing `create_memory` seam so an explicit retention request closes the current uncovered Episode tail, queues it immediately, and gives that Insomnia run a hard success contract requiring at least one accepted canonical Memory with provenance to the initiating turn;
- expose that same narrow operation through the shared live runtime/tool adapter for explicit user- or model-initiated retention without granting either caller direct Memory-write authority;
- run the full 66-Episode gold-v3 corpus as milestone confirmation;
- recalibrate worker concurrency and model/reasoning cost for the selected production route mix; and
- add targeted verifier/repair, selective voting, deterministic clause preprocessing, ambiguity routing, supersession resolution, provenance verification, or stronger/fine-tuned selection only when production failures justify the added inference.

Do not resume benchmark-specific prompt squeezing merely to chase stochastic fixture misses.

## Chronos

Modularize and expand the current Dream temporal machinery into the shared Chronos subsystem defined by [ADR 0035](decisions/0035-chronos-shared-temporal-semantics.md) and the [Chronos subsystem plan](chronos-subsystem-plan.md).

Near-term work should:

- expand/calibrate the implemented indication detector and bounded temporal-vocabulary normalizer from measured recall/false positives rather than broad spell correction;
- keep the implemented deterministic grammar conservative: qualified seasons and safe loose-calendar forms are covered, while further grammar additions should come from measured unambiguous cases rather than locale/hemisphere guesses;
- keep deterministic Chronos output derived/unpersisted by default;
- calibrate the implemented unresolved-only inference route from measured need/cost: canonical model answers are deterministically same-kind verified before publication, and abstention remains valid;
- preserve the implemented Memory inference binding to exact `MemoryBodyId + source_time_ns`, including reconciliation/migration behavior and stale-on-binding-change semantics; and
- add Perception Observation consumption plus any additional Dream validity/ordering outputs only where measurement demonstrates need.

Chronos owns source/valid-time interpretation mechanics. Wall-clock transaction/knowledge time is already implemented and remains owned by the timestamped global/container version stream.

## Echo retrieval surface

Expose bounded source-scoped Echo expansion to higher-level context assembly.

Requirements:

- upstream source/provenance selection must determine what Echo is eligible;
- Echo must remain cold by default and non-authoritative;
- no global reasoning-trace search index should be introduced by default; and
- provider continuation state must remain separate from historical reasoning evidence.

See [ADR 0014](decisions/0014-echo-historical-reasoning-traces.md).

## Dream follow-up

Treat further Dream architecture as measurement-driven rather than continuing unconditional inference expansion.

Potential future work includes:

- bounded reconsideration only for observed unresolved/ambiguous cases;
- shared Chronos temporal ordering/validity outputs for Dream only where measured reasoning needs them, without changing Dream's Memory-to-Memory ownership;
- derived temporal acceleration only if measured candidate cost warrants it;
- additional relationship provenance/evidence persistence where product inspection requires it; and
- new relation classes only with explicit publication and lifecycle semantics.

The shipped design and validation history belong to current architecture/reference and retained validation material, not this roadmap.

## Perception

Implement the referential-Entity / propositional-Observation architecture defined by [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), with ADR 0034's relational-Observation specialization and privacy rules, following the detailed [Perception subsystem plan](perception-subsystem-plan.md).

Entity-mention enrichment and owner-local Entity processing are implemented. Remaining work is:

1. complete cross-scope Entity identity (Pass 1d): the global UUID substrate/backfill is implemented; add relational-evidence-driven eligible-owner candidate generation, dirty triggering, pair fingerprints/receipts, reconciliation/repair, and audit against frozen Ellis and 28-day corpora;
2. define the unified Observation owner and converge the sparse REL/PHY RelationshipStore into a relational-Observation profile keyed by Observation identity rather than an independent semantic authority;
3. implement fully processed **Memory and Observation propositional anchors**; Entity creation remains traversal/referential state and does not independently anchor Observation synthesis;
4. implement staggered synthesis scheduling: immediate **Exploitation** for new anchors, then per-mode cadence C; **Exploration** is offset by C / 2 and then recurs on the same cadence;
5. implement whole-Web bootstrap while the authorized propositional Web fits, then the four Exploitation candidate lanes: Entity bridge, source neighbourhood, Web-structure/regional neighbourhood, and derivation neighbourhood;
6. implement distinct candidate-generation and context/inference budgets, bounded Exploitation overflow, recent Exploitation/Exploration comparison ledgers, and multiple narrow comparison jobs per anchor round;
7. implement Exploration lanes for Exploitation overflow, semantic similarity, lexical similarity, cross-region sampling, and random sampling, with optional cheap Decision/Jev routing probes;
8. implement separate **ordinary Observation synthesis** and **relational Observation synthesis** model contracts under either Exploitation or Exploration; relational synthesis requires at least two distinct durable Entity participants and may not consume relational Observations as relational-synthesis evidence;
9. implement staged formation: discovery selection -> proposal -> reasoning-directed evidence expansion -> deductive/inductive/abductive/abstractive derivation -> independent verification -> publication, with exact many-to-many lineage and transitive-overlap accounting;
10. implement high-recall Observation receptors plus strict pairwise **propositional anchor <-> existing Observation** contribution, including Observation anchors with bounded support-lineage hydration;
11. add target-Observation mutation thresholds/reconsideration, class-sensitive wall-time staleness policy, persistent Entity/Observation ambiguity, and deterministic runtime clarification injection; and
12. extend derived Communities into a multi-resolution hierarchy only where measured routing/retrieval value justifies it. Communities remain one Web-structure/regional signal rather than the primary Observation candidate mechanism.

Perception must preserve these boundaries:

- Memories and Observations are propositional reasoning nodes; Entities are referential/traversal nodes and do not enter synthesis as standalone evidence;
- newly published Observations may become anchors for higher-order reasoning after their own required processing completes;
- lower-level Graph/Entity/Community mutations do not recursively schedule synthesis by themselves;
- whole-Web bootstrap is permitted only while the authorized propositional Web fits the configured context budget;
- outside bootstrap, Exploitation requires an independent structural/provenance/derivation reason for comparison; embedding similarity alone is not an Exploitation justification;
- Exploration is intentionally wide-ranging and uses bounded recent comparison history rather than a permanent settled-pair ledger or a theoretical "ever Exploitation-eligible" blacklist;
- candidates omitted by Exploitation budget remain recoverable, including through the Exploitation-overflow Exploration lane;
- candidate-generation budgets and model-context/inference budgets are distinct;
- Communities are not hard inference boundaries and are not the primary Observation selector;
- no exhaustive anchor-by-all-Observations contribution scan;
- no exhaustive Entity-pair relational-Observation path and no automatic relation per Entity pair;
- no relational Observation used as evidence/anchor for new relational-Observation synthesis;
- no non-active PHY relational-Observation traversal through shared REL Entities;
- no routing receptor or Decision/Jev probe as evidentiary authority;
- no deletion/invalidation of Observations merely because confirming evidence failed to arrive; and
- no general curiosity/open-question framework beyond Entity/Observation ambiguity in the initial subsystem.

Scale/quality gates should measure receptor recall, contribution false positives, whole-Web/bootstrap cost, per-lane Exploitation yield and starvation, Exploitation-overflow recovery, Exploration diversity/repetition/bridge yield, recent-ledger behavior, comparison-context cost, reasoning-specific precision, evidence-expansion effectiveness, cascade depth/fan-out, duplicate rejection, transitive-provenance double-count prevention, relational-synthesis gating, and class-sensitive stale-Observation lifecycle behavior before tuning thresholds or adding broader inference.

## Memory-web and retrieval integration

Future integration work:

- attach reusable `MemoryRetrievalIndex` lifecycle to Ego/runtime so repeated queries reuse derived indexes and rebuild deterministically when stale;
- compose owner-local REL and PHY retrieval above those primitives without creating cross-owner Dream/Graph authority;
- compose the effective relational-Observation view from active-PHY plus authorized active-REL Observations, preserving Observation-owner visibility and portable shared REL state without creating a universal persisted Relationship graph;
- measure release-mode latency against `GlobalExact` during rollout;
- consider explicit lower-cost routing modes only if production economics justify them;
- tune the implemented Community-lineage continuation/material-change thresholds only from real archive behavior; continuity-preserving Community IDs remain unnecessary while derived lineage is sufficient;
- evaluate multi-resolution Community routing for coarse -> normal -> fine retrieval/Ego composition after Perception C lands, while keeping every Community level derived rather than an independent semantic authority;
- add incremental Community maintenance only if measured scan-and-merge cost becomes material; and
- preserve explicit user Community names as derived semantic metadata only, never Memory-Web authority; current lineage inheritance must remain conservative around ambiguous splits/merges.

## Freshness integration and retained source-progress work

Wire only real accepted-context and live Ego consumers when their host contracts exist. Preserve owner-specific authorization, protected keep lanes, full-small-Web selection, searchable Dormant Memories and independent factual truth. Complete historical score-trajectory calibration only after genuine activity/admission/link/use evidence is available. F8 semantic relevance/import bootstrap is deferred separately; PHY Freshness still needs an authorized activity clock.

Retain independently justified Archive progress/checkpoint materialization and Ego REL/PHY summary refresh rules. Do not reintroduce a per-Memory review scheduler or mandatory Perception stale-Memory audit. The [core verification](freshness-core-release-verification-2026-10-03.md) records the accepted library gates and remaining external integration/measurement limits.

## Ego

The first Ego substrate is implemented in this branch. REL/PHY persist owner-local Anchors and cached web synthesis; PHY additionally persists Personality and multiple stable Identity documents with one explicit active selection. Identity records have stable IDs, independent revisions/tombstones, explicit no-op-safe activation, legacy single-Identity compatibility, and a compatibility accessor/mutator for the active Identity. These durable records deliberately do not yet perform inference, scheduling, cross-owner assembly, or prompt injection.

The target project Memory-Web Summary is now explicitly **user-relative**. Shared evidence remains REL-owned, including conversation records/tails, Memories, vectors, Graph/Communities, and chronology. REL conversation records carry PHY identity, so Ego can derive the active user's recent conversation tails from shared REL state. Those tails should seed multiple recent-activity semantic centroids that deterministically route older/current REL Memories into a bounded working Web. Cold state remains searchable and exclusion from the working Web is not a semantic invalidation claim.

The summary pipeline is staged rather than one-shot: (1) broad mandatory-coverage Memory-Web audit, (2) separate candidate selection, (3) source-grounded summary generation for selected candidates, and (4) reconciliation/final assembly. The audit itself does **not** summarize. Its job is discovery/segmentation. The audit processes controlled, likely overlapping batches while exposing full-REL `memory_search`; the controller must ensure every working-Web Memory is surfaced as mandatory core material at least once before completion.

Refresh is owner-specific and queued. REL-backed project summaries use a durable incoming-turn activity clock: one count for each previously unseen logical user or agent turn successfully accepted into that REL, independent of physical stored-turn de-duplication, compaction, or ingest retries. `W` is a recent-turn window used for conversation-tail relevance, not the refresh interval. A fresh REL queues its first Ego generation once it has 25 eligible durable Memories. Thereafter refresh is due after percentage growth from the last successful source turn count, capped by an absolute maximum turn delta. PHY-local Ego freshness instead uses the PHY Memory version/mutation index: it becomes eligible on the first useful Memory mutation and then refreshes after percentage Memory-version growth capped by an absolute mutation delta; raw Graph churn does not independently drive PHY freshness. Becoming due queues Ego rather than invoking it immediately. The processing coordinator must first complete the semantic work required through the marked Episode/source boundary; for REL-backed work this means Insomnia through that Episode and all Dream work required by that Insomnia output. The marked boundary prevents newer activity from starving a due refresh. There is no wall-clock age backstop. The target project-summary perspective belongs with PHY even though its evidence is REL-owned; exact PHY-owned per-REL summary persistence/keying remains future work.

Current Ego status:

- durable owner-local Ego persistence is implemented for PHY Identity/Personality, PHY/REL Anchors, and cached owner-local Memory-Web synthesis;
- multi-Identity PHY support is implemented in this branch, including stable Identity IDs, one explicit active selection, swapping, independent revisions/tombstones, and legacy single-Identity compatibility;
- the target Memory-Web Summary policy is designed at the architecture level, but recent-tail centroid routing, working-Web selection, mandatory audit traversal, candidate selection, summary inference, reconciliation, activity accounting, target PHY persistence, and refresh scheduling are not implemented;
- Personality ownership and evidence boundaries are settled, but synthesis mechanics, refresh policy, user-edit interaction, output shape, and budget remain unresolved;
- deterministic Cross-chat selection policy is designed but not implemented;
- Ego has not yet consumed the implemented REL dependency closure and host-owned PHY to assemble context packages; cross-REL summary inclusion, including separately authorized reverse/dependent selection, is designed but unimplemented; and
- final Ego context assembly, runtime scheduling, budgeting calibration, Cross-chat materialization, and host-facing context projection remain unimplemented.

Next, Ego should:

- complete the [Archive-owned REL activity clock Phase 1](rel-activity-clock-implementation-plan.md), then calibrate the recent-turn window `W`, REL percentage/cap refresh cadence, and active-PHY selection of recent REL conversation tails;
- derive and calibrate multiple recent-activity centroids plus deterministic Memory relevance/working-Web admission, using Chronos/lifecycle keep lanes while keeping cold state searchable;
- implement the mandatory-coverage audit controller with adaptive batches, overlap/context, persistent discovered-thread state, and full-Web `memory_search`;
- implement candidate selection separately from audit and summary generation;
- implement source-grounded candidate summaries plus final reconciliation, with explicit tests preventing summary inference from silently becoming Observation inference;
- resolve PHY-owned per-REL Memory-Web Summary persistence/keying over the current substrate;
- design Personality synthesis mechanics separately: Personality is PHY-owned, persisted in PHY, derived only from PHY behavioural evidence such as communication/process/relationship preferences and recurring behaviour, and never from REL/project state; generic user biography remains ordinary PHY Web state rather than Personality evidence;
- implement deterministic Cross-chat context from existing REL conversation-compaction records, ordered by most recent conversation activity rather than creation time, within the remaining Ego injection budget;
- consume the graph host's active-REL dependency closure in deterministic dependency order rather than rebuilding topology inside Ego;
- select relevant existing active-PHY per-REL summaries from authorized dependency scopes and separately authorized reverse/dependent scopes without automatic sibling access, cross-owner synthesis, or an independent context budget;
- project the host-owned Phylactery through explicit role/privacy policy; and
- keep source/provenance access available without flooding the default prompt.

The Memory-Web Summary rationale, measurements, staged audit/candidate/summary architecture, and calibration sequence are recorded in [Ego Memory-Web summary plan](ego-web-synthesis-plan.md). Cross-chat selection and budgeting are recorded in [Ego Cross-chat context plan](ego-cross-chat-context-plan.md). Bidirectional authorized summary selection (distinct from synthesis and from dependency inheritance) is recorded in [Ego cross-REL summary inclusion plan](ego-cross-rel-summary-inclusion-plan.md).

Ego context-assembly policy is Reliquary-owned under the graph-aware host boundary established by ADR 0037. Warlock's Ego document is a host-facing integration contract; it must not become a second source of semantic policy.

## Storage, scale, and historical recovery

Keep physical optimization measurement-driven behind existing semantic boundaries:

- mapped/segmented vector scanning and ANN acceleration;
- persistent lexical acceleration only if reopen/query measurements justify it;
- quantized searchable representations;
- explicit vector-generation retirement;
- REL/PHY packing, compaction, retention, and recovery; and
- derived-state rebuild/invalidation policy.

Whole-REL historical restore/branching must be defined in semantic-database terms rather than reintroducing project-file VCS. See [Versioning, historical cuts, and rollback](version-history-plan.md).

## Product acceptance gates

### Usable local product

A non-developer can create/open a Warlock project, converse through Warlock, work with files, browse conversations and durable knowledge, and understand basic provenance without CLI use.

### Interoperable product

Equivalent normalized interaction history can enter through Warlock-native execution and at least one external adapter without protocol-specific semantic records.

### Durable live product

Long-running capture, background processing, reconnect, crash/restart, and provider failures have explicit tested behavior and do not silently lose acknowledged source events.

### Expandable semantic product

New semantic owners remain purpose-built, use stable cross-owner IDs, and do not require a generalized REL root/dependency framework.

## Open decisions

- Exact Ego projection/retention policy across the already-implemented active-REL dependency closure and host-owned PHY.
- Cross-file Memory/source lineage and export permissions among REL and PHY owners.
- Whether `Cva` remains only an internal/back-compat term.
- How learned-state ownership expands beyond User/Project without conflating ownership with governance or authorization.
- Normalized interaction vocabulary for tool/session/artifact events beyond completed turns.
- REL/PHY synchronization and conflict transport across devices.
- Which semantic mutations are safe to expose as direct user actions.
- Adapter failure policy and privacy controls for automatic capture.
- Artifact provenance vocabulary across uploaded, generated, imported, and provider-managed artifacts.
- Archive checkpoint representation, packing/compression choices, and retention policy.
- Whole-REL restore/timeline terminology and retention semantics.
- Exact recent-turn window `W`; REL refresh fraction and maximum turn delta; PHY Memory-version refresh fraction and maximum mutation delta; and pending Ego refresh/coalescing policy. REL bootstrap is fixed at 25 eligible durable Memories, while PHY bootstrap begins on its first eligible Memory mutation.
- Calibrate unified Memory Freshness event frequency, duplicate-component reinforcement, 10-turn decay, outside-origin-Community propagation and score distribution; settle PHY-specific owner-local activity only when justified. Protected Ego keep lanes and relevance weighting remain independent of factual truth.
- Personality behavioural-evidence selection details, refresh policy, user-edit authority, output shape, and budget; ownership and PHY-only evidence scope are settled.
- Ego context-budget calibration around the approximately 20% usable-input ceiling, including deterministic Cross-chat allocation after Identity/Personality/Anchors/Web synthesis; current-session compaction remains outside Ego.

## Explicitly not planned

The current roadmap does not include:

- embedding Lore as the general physical substrate for REL/PHY;
- migrating Memory/Graph/vector/Echo storage onto Lore;
- maintaining a hidden Lore repository when Git is the selected project-history authority;
- a second permanent uploaded-file blob store;
- a virtual/copy-on-write filesystem or filesystem driver inside Reliquary; or
- a mandatory independently deployed Reliquary service.

## Related docs

- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)
- [Versioning and rollback plan](version-history-plan.md)
- [ADR 0014](decisions/0014-echo-historical-reasoning-traces.md)
- [ADR 0015](decisions/0015-acp-inline-interaction-stream.md)
- [ADR 0016](decisions/0016-native-product-surface-and-shared-interaction-runtime.md)
- [ADR 0017](decisions/0017-cva-workspace-and-warlock-host-application.md)
- [ADR 0019](decisions/0019-cloud-backed-cva-reconciliation.md)
- [ADR 0027](decisions/0027-warlock-project-repositories-and-reliquary-storage-boundary.md)
- [ADR 0028](decisions/0028-project-folder-and-repository-bootstrap-contract.md)
- [ADR 0029](decisions/0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md)
- [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [ADR 0035](decisions/0035-chronos-shared-temporal-semantics.md)
- [Chronos subsystem plan](chronos-subsystem-plan.md)
- [Perception subsystem plan](perception-subsystem-plan.md)

## Notes

This file is future-only by policy. When a roadmap item ships, remove it from this document and document the resulting behavior in the current-state owners instead.
