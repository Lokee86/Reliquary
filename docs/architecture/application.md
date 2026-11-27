# Application workflows

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe the product workflow owner for coordination across semantic services, owner sessions, and external evidence.

## Overview

`reliquary-application` owns named product workflows: conversation and native import publication, memory management, retrieval selection, Dream publication, entity resolution workflows, freshness transitions, vector evaluation/publication coordination, maintenance, history, project correlation, and context preparation. It validates caller and semantic inputs, binds proposals to accepted owner evidence, rechecks staged work, and invokes owner-session operations for durable changes.

Application is the product-policy composition layer. It does not own physical format, store replay, provider transport, principal authentication, or the durable writer handles. Semantic crates can produce bounded proposals or decisions; Application determines whether and how a named product workflow publishes them.

## Workflow and state

A typical workflow reads a current owner snapshot, prepares bounded work, validates source identity and provenance, and then commits through an owner-session writer. For instance, native ingest binds extracted candidates to an accepted Archive episode and its turns, checks evidence bounds and identity, and routes candidates by ownership. Publication revalidates the accepted source before committing through owner-session. Retrieval returns selected evidence to the caller. Delivery or Memory mutation requires its own workflow.

Copied Hermes imports validate the ordered Archive source and the host-redacted Echo together. A nonempty Echo projection participates in the copy digest. Under the mounted writer lock, the workflow publishes Archive turns, syncs each source-keyed Echo event, and then commits the checked receipt. An interrupted epoch accepts the same snapshot and sequences on retry. Exact reads require an accepted receipt and can walk a bounded Archive ancestry range across copied suffixes.

Committed delete and rewind use the mounted writer's copied-source publication. The source scope carries the authorized profile, session and principal. Archive range and Echo projections honor the current source removal cut, including for previously issued references and indexed passages. Observation preparation and review check eligible provenance before sending evidence to the decision endpoint.

Archive hybrid ranking gets lexical match counts from the captured generation's inverted index, whether loaded from a sealed REL frame or rebuilt from source. It combines their scores with exact cosine across that generation before selecting the top results. It returns both score components and the fragment's source range. Application ranks readable Memories and verified current Observation claims in the same owner population. The agentic decision tool evaluates either kind through the same candidate offer. Claim document embeddings use the accepted Memory profile and remain bound to their exact Observation citations. Default semantic ranking checks all available vectors for the bound compatibility profile. Community routing can be selected explicitly by workflows that want a narrowed candidate set.

For temporal requests, Application applies the source-time or Chronos predicate to the captured population before ranking or offering candidates. The shared adapter verifies stored body-bound interpretations for Dream and search. Archive ranking uses eligible turn text for partial fragments because the packed vector spans the full fragment.

Cross-owner workflows keep each mutable authority local. When an operation affects REL and PHY, it uses explicit receipts, copy/reopen verification, or reconciliation rather than exposing a shared store. Retry behavior uses stable request/owner identities and current cuts; stale preparations are rejected or recomputed instead of applied over newer state.

Native import Dream retries a conflicting lifecycle proposal up to twice on the same pinned Memory frontier before any later Memory can publish. If the conflict persists, Application evaluates the final judgments in pinned candidate order, retains the compatible decisions, and records each incompatible pair as rejected. The validated effects and pair outcomes are published through the same grouped owner commit before the next Memory advances. Provider errors and stale owner evidence still follow their existing failure paths.

Inspection projects a Memory summary for one ID at an exact owner cut. It shares the paginated list's lifecycle, canonical status, and Community projection.

Inspection selects the retained revision metadata before loading its payload. Application propagates a failed leased Memory or Archive read as a verification error. A recorded tombstone or pruned body retains its typed availability.

Community recompute solves the captured graph and calculates coverage and modularity. Preview returns the report. Apply sends that same partition and its captured versions to the REL writer.

For cross-REL Entity identity, Application exposes the Entity resolver’s owner-local key index to Gateway for paged candidate lookup. Gateway supplies one authorized owner pair at a time. Application prepares and evaluates one pair-specific judgment from the bounded evidence supplied by Gateway. Missing independent evidence settles the pair as unresolved. When both sides can support a judgment, Application invokes the Entity resolver and turns its checked answer into a receipt for the recording REL. During inspection, Application follows each citation to the corresponding owner's retained evidence. A mention citation recovers its exact Memory excerpt after checking the resolved Entity, mention revision or migrated source ordinal, and Memory body. A Relationship citation recovers the grounded quote from its cited Memory.

Application captures audit work from one REL. It rechecks Fresh Memory references for Ego topics and hydrates selected sources and resolved Entities for Perception Synthesize. Synthesize returns a typed claim for receptor preparation, the evidence back-check and native Verify. A related evidence match enters batched contribution. Pending heads use the same draft verification pipeline and supply the typed prior claim when resynthesizing. Accepted drafts and progress go through the owner-session Observation writer. [Audit work](../../crates/reliquary-application/src/perception_workflow.rs) · [Draft and review](../../crates/reliquary-application/src/observation_drafts.rs)

Application resolves graph slots through the scoped owner at the captured cut. PHY capture plans bounded Ego-only batches and resumes oversized source fields at their exact UTF-8 byte offsets. The PHY evidence reader returns exact text with the accepted revision's source time and temporal metadata for later stages. [PHY capture](../../crates/reliquary-application/src/perception_workflow/phy_discovery.rs)

Context preparation prepends attributed dependency-synthesis messages before the primary request and counts them under its existing source and compaction budgets. The transcript and source view used for checkpointing contain the existing conversation evidence.

## Failure, recovery, and invariants

The key invariants are: no semantic output is treated as truth without validation; source evidence must match the owner’s accepted records; mutation is explicit and owner-local; bounded inputs are rejected before expensive reads or provider work where possible; and durable publication goes through a typed owner-session method. Recovery is performed by rereading/replaying owner state and retrying the same stable workflow identity, not by rewriting store files in Application.

## Code map and protecting tests

- Workflow composition and validation: [`crates/reliquary-application/src/lib.rs`](../../crates/reliquary-application/src/lib.rs).
- Workflow modules include [`conversation_workflow.rs`](../../crates/reliquary-application/src/conversation_workflow.rs), [`ingest_publication.rs`](../../crates/reliquary-application/src/ingest_publication.rs), [`dream_publication.rs`](../../crates/reliquary-application/src/dream_publication.rs), [`management_workflow.rs`](../../crates/reliquary-application/src/management_workflow.rs), [`verified_copy_workflow.rs`](../../crates/reliquary-application/src/verified_copy_workflow.rs), and [`context_workflow.rs`](../../crates/reliquary-application/src/context_workflow.rs).
- Unit tests in `lib.rs` include forged-source rejection, bounded evidence checks, owner-lease idempotency, and reopen recovery. Workflow-local tests cover stale Dream preparation, project-source validation, entity reopen/retry, and copy divergence. Native import publication and source replay fixtures open new RELs through the current owner-session initializer. The native import publication fixture verifies that sibling Memories from one extraction share a historical Dream population on full owner replay.
- Public facade and external consumer contract are rooted at [`src/lib.rs`](../../src/lib.rs) and the external integration suite under [`tests/external-consumer`](../../tests/external-consumer).

## Extension seams

Put an operation here when it coordinates distinct owners, translates semantic results into named product policy, or needs to validate provenance across a workflow. Put a single-store replay invariant in that store or owner-session; put provider routing in Switchboard; put host authorization and process lifecycle in Gateway.

## Related docs

- [Reliquary source and binary distribution terms](../../README.md#license)
- [Owner sessions](owner-session.md)
- [Semantic processing](semantic-processing.md)
- [Gateway and host lifecycle](gateway.md)
- [Storage and durable state](storage.md)

## Notes

This page describes shared workflow ownership; individual user and operator procedures belong to their guide/reference owners.
