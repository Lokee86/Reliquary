# Roadmap

Parent index: [Documentation index](INDEX.md)

## Purpose

This document owns future implementation work for Reliquary. Completed behavior does not belong here; current behavior is documented in [Architecture](architecture.md), [Rust API](api.md), [Storage format](storage-format.md), [Repo-local CLI](cli.md), and [Current limitations](current-limitations.md).

## Overview

Reliquary is the purpose-built semantic/storage/runtime library embedded by Warlock. Its semantics do not depend upward on Warlock. Project-file history belongs to Lore or Git through Reliquary's Project Environment; REL/PHY retain independent semantic state and history.

The immediate semantic sequence is now: finish cross-scope Entity identity, then Relationship/Observation composition, then Ego. The graph-aware runtime host, cross-owner retrieval substrate, Project Environment ownership, conversation/session ownership, Context Engine policy, and Warlock cleanup that these stages depend on are already complete.

## Current semantic sequence

1. **Cross-scope Entity identity.** Complete global Entity UUID sets, deterministic eligible-owner candidate generation, dirty triggering, pair fingerprints/receipts, repair, and Pass 1d against the graph-aware host. Calibrate on frozen Ellis and 28-day corpora before promotion.
2. **Relationship/Observation composition.** Build Relationship visibility/synthesis and Observation composition over stable cross-owner Entity identity, preserving active-PHY/authorized-REL privacy and provenance.
3. **Ego.** Implement owner-local synthesis scheduling, graph-aware context assembly, deterministic current-REL Cross-chat selection, budgeting, PHY participation, and context projection. Ego is intentionally last so it composes the completed semantic substrate rather than constraining it prematurely.

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

## Scope and ownership evolution

Reliquary already has typed durable scope identity; future work is about how those owners are composed and governed.

Near-term scope work should:

- expose editing/inspection surfaces for the already-implemented REL dependency topology without weakening cycle rejection, dependencies-before-dependent ordering, or sibling isolation;
- keep durable ownership separate from retrieval visibility and mutation authority;
- define scope creation/editing and hierarchy-management surfaces;
- extend learned-state routing beyond the current conservative User/Project boundary only when the destination authority rules are explicit;
- keep governed Organization state distinct from learned observations; and
- define explicit cross-file Memory/source export and lineage semantics.

Relationship-specific semantic state is reactivated by [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md), but **not** as a Connection REL class. The deterministic sparse Relationship owner is now implemented in both REL and PHY: owner-qualified Entity participants, owner-qualified Memory evidence, revisions/local clocks, reopen, local-reference validation, Entity-merge retargeting, migration, divergent reconciliation, and physical reclamation are covered. The remaining B2 work is semantic synthesis/materialization, lifecycle/relationship-local derived state, and runtime composition/privacy enforcement for active PHY plus authorized active RELs. Existing legacy Connection-typed REL identity remains compatibility-only.

## Reliquary / Phylactery product transition

Remaining product/compatibility work:

1. warn when filename hints disagree with authoritative internal type/scope;
2. define cross-file Memory/source export and lineage semantics between REL and PHY; and
3. decide whether the internal/back-compat `Cva` terminology should eventually be removed from public-facing APIs and documentation.

Do not implement `.phy` as a Project Reliquary with provenance fields merely made nullable. Shared mechanics and separate semantic validation remain required.

## Insomnia follow-up

Routine prompt tuning on the adversarial fixture remains frozen. Future work is validation or a new measured capability boundary:

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

Implement the Entity/Relationship/Observation semantic layer defined by [ADR 0033](decisions/0033-perception-entities-observations-and-ambiguity.md), [ADR 0034](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md), and the detailed [Perception subsystem plan](perception-subsystem-plan.md).

Entity-mention enrichment and owner-local Entity pass 1 are implemented. The remaining sequence is:

1. complete cross-scope Entity identity (Pass 1d): global Entity UUID sets, eligible-owner candidate generation, dirty triggering, pair fingerprints/receipts, reconciliation/repair, and audit against frozen Ellis and 28-day corpora;
2. complete the ADR 0034 Relationship semantic layer above the existing sparse REL/PHY Relationship storage: synthesis/materialization, relationship-local derived state, cross-owner visibility, and active-PHY/authorized-REL privacy enforcement;
3. implement Observation persistence and bounded multi-Memory extrapolation with exact support/derivation lineage;
4. add separately embedded high-recall Observation routing receptors and strict pairwise `Memory <-> Observation` contribution inference over receptor/entity/dependency/Relationship-routed candidates rather than an all-Observation scan;
5. add mutation-threshold and wall-time Observation reconsideration plus persistent Entity/Observation ambiguity and deterministic runtime clarification injection; and
6. extend derived Communities into a multi-resolution semantic hierarchy only where measured routing/scale evidence justifies it, preserving lineage/naming/inspection semantics and bounded local processing neighbourhoods for oversized irreducible leaves.

Perception must preserve these scaling boundaries:

- no exhaustive `new Memory x all Observations` inference path;
- no exhaustive Entity-pair Relationship inference path and no automatic Relationship-per-Entity materialization;
- no non-active PHY Relationship traversal through shared REL Entities;
- no forced/fake Leiden subdivisions or super-community groupings solely to meet context/presentation budgets;
- no promotion of scan-and-merge reduction intermediates into semantic hierarchy; multi-resolution structure must be derived independently from graph/community state;
- no routing receptor as evidentiary authority;
- no deletion/invalidation of Observations merely because confirming evidence failed to arrive; and
- no general curiosity/open-question framework beyond Entity/Observation ambiguity in the initial subsystem.

Scale/quality gates should measure receptor recall, pairwise false positives, multi-resolution Community/local-neighbourhood coverage, hierarchy stability/lineage quality, inference cost growth, and stale-Observation lifecycle behavior before tuning thresholds or adding broader inference.

## Memory-web and retrieval integration

Future integration work:

- attach reusable `MemoryRetrievalIndex` lifecycle to Ego/runtime so repeated queries reuse derived indexes and rebuild deterministically when stale;
- compose owner-local REL and PHY retrieval above those primitives without creating cross-owner Dream/Graph authority;
- compose the effective Relationship graph separately from active-PHY Relationships plus authorized active-REL Relationships, preserving Relationship-owner visibility and portable shared REL state;
- measure release-mode latency against `GlobalExact` during rollout;
- consider explicit lower-cost routing modes only if production economics justify them;
- tune the implemented Community-lineage continuation/material-change thresholds only from real archive behavior; continuity-preserving Community IDs remain unnecessary while derived lineage is sufficient;
- evaluate multi-resolution Community routing for coarse -> normal -> fine retrieval/Ego composition after Perception C lands, while keeping every Community level derived rather than an independent semantic authority;
- add incremental Community maintenance only if measured scan-and-merge cost becomes material; and
- preserve explicit user Community names as derived semantic metadata only, never Memory-Web authority; current lineage inheritance must remain conservative around ambiguous splits/merges.

## Ego

The first Ego substrate is implemented in this branch. REL/PHY persist owner-local Anchors and cached web synthesis; PHY additionally persists Personality and multiple stable Identity documents with one explicit active selection. Identity records have stable IDs, independent revisions/tombstones, explicit no-op-safe activation, legacy single-Identity compatibility, and a compatibility accessor/mutator for the active Identity. These durable records deliberately do not yet perform inference, scheduling, cross-owner assembly, or prompt injection.

Memory-Web synthesis now has a concrete design direction. REL synthesis should use a bounded REL-local activity window rather than calendar age, so dormant projects do not decay and activity in another REL cannot displace their state. Objective preprocessing is limited to explicit validity such as archived/historical exclusion and Dream-owned duplicate collapse; categories/types remain synthesis structure rather than deterministic relevance filters.

Refresh is intentionally batched. Let `W` be the synthesis activity window, `N` a considerably smaller activity-unit check interval, and `M` the accumulated Memory-Web mutation threshold. Every `N` activity units, Ego performs only a cheap deterministic change check; it resynthesizes from the current `W`-unit window only after the accumulated mutation delta reaches `M`. Below `M`, the delta carries forward. There is no age-based refresh backstop: `W` bounds synthesis input and is not a maximum synthesis lifetime.

The exact activity-unit definition plus calibrated `W`/`N`/`M` values remain measurement work. PHY must be measured separately before inheriting the REL policy; the current 28-day PHY is much smaller than the REL stress case.

Current Ego status:

- durable owner-local Ego persistence is implemented for PHY Identity/Personality, PHY/REL Anchors, and cached owner-local Memory-Web synthesis;
- multi-Identity PHY support is implemented in this branch, including stable Identity IDs, one explicit active selection, swapping, independent revisions/tombstones, and legacy single-Identity compatibility;
- Memory-Web synthesis policy is designed but synthesis inference, activity accounting, and refresh scheduling are not implemented;
- Personality ownership and evidence boundaries are settled, but synthesis mechanics, refresh policy, user-edit interaction, output shape, and budget remain unresolved;
- deterministic Cross-chat selection policy is designed but not implemented;
- Ego has not yet consumed the implemented REL dependency closure and host-owned PHY to assemble context packages; and
- final Ego context assembly, runtime scheduling, budgeting calibration, Cross-chat materialization, and host-facing context projection remain unimplemented.

Next, Ego should:

- implement and calibrate REL-local activity units against the existing 28-day high-activity stress fixture;
- implement initial and batched-refresh Memory-Web synthesis using the activity-relative policy;
- design Personality synthesis mechanics separately: Personality is PHY-owned, persisted in PHY, derived only from PHY behavioural evidence such as communication/process/relationship preferences and recurring behaviour, and never from REL/project state; generic user biography remains ordinary PHY Web state rather than Personality evidence;
- implement deterministic Cross-chat context from existing REL conversation-compaction records, ordered by most recent conversation activity rather than creation time, within the remaining Ego injection budget;
- consume the graph host's active-REL dependency closure in deterministic dependency order rather than rebuilding topology inside Ego;
- project the host-owned Phylactery through explicit role/privacy policy; and
- keep source/provenance access available without flooding the default prompt.

The Web-synthesis rationale, measurements, rejected alternatives, and calibration sequence are recorded in [Ego Memory-Web synthesis plan](ego-web-synthesis-plan.md). Cross-chat selection and budgeting are recorded in [Ego Cross-chat context plan](ego-cross-chat-context-plan.md).

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
- Exact REL-local activity-unit definition and measured `W`/`N`/`M` Ego Web-synthesis calibration.
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
