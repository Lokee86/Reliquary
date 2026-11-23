# Application workflows

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Describe the product workflow owner for coordination across semantic services, owner sessions, and external evidence.

## Overview

`reliquary-application` owns named product workflows: conversation and native import publication, memory management, retrieval selection, Dream publication, entity resolution workflows, freshness transitions, vector evaluation/publication coordination, maintenance, history, project correlation, and context preparation. It validates caller and semantic inputs, binds proposals to accepted owner evidence, rechecks staged work, and invokes owner-session operations for durable changes.

Application is the product-policy composition layer. It does not own physical format, store replay, provider transport, principal authentication, or the durable writer handles. Semantic crates can produce bounded proposals or decisions; Application determines whether and how a named product workflow publishes them.

## Workflow and state

A typical workflow reads a current owner snapshot, prepares bounded work, validates source identity and provenance, and then commits through an owner-session writer. For instance, native ingest binds extracted candidates to an accepted Archive episode and its turns, checks evidence bounds and identity, and routes candidates by ownership. Publication revalidates the accepted source before committing through owner-session. Retrieval returns selected evidence to the caller. Delivery or Memory mutation requires its own workflow.

Archive hybrid ranking gets lexical match counts from the captured generation's inverted index, whether loaded from a sealed REL frame or rebuilt from source. It combines their scores with exact cosine across that generation before selecting the top results. It returns both score components and the fragment's source range. Application projects the current owner's readable Memory candidates before semantic ranking. Default semantic ranking checks all available vectors for the bound compatibility profile. Community routing can be selected explicitly by workflows that want a narrowed candidate set.

Cross-owner workflows keep each mutable authority local. When an operation affects REL and PHY, it uses explicit receipts, copy/reopen verification, or reconciliation rather than exposing a shared store. Retry behavior uses stable request/owner identities and current cuts; stale preparations are rejected or recomputed instead of applied over newer state.

Inspection projects a Memory summary for one ID at an exact owner cut. It shares the paginated list's lifecycle, canonical status, and Community projection.

Community recompute solves the captured graph and calculates coverage and modularity. Preview returns the report. Apply sends that same partition and its captured versions to the REL writer.

Context preparation prepends attributed dependency-synthesis messages before the primary request and counts them under its existing source and compaction budgets. The transcript and source view used for checkpointing contain the existing conversation evidence.

## Failure, recovery, and invariants

The key invariants are: no semantic output is treated as truth without validation; source evidence must match the owner’s accepted records; mutation is explicit and owner-local; bounded inputs are rejected before expensive reads or provider work where possible; and durable publication goes through a typed owner-session method. Recovery is performed by rereading/replaying owner state and retrying the same stable workflow identity, not by rewriting store files in Application.

## Code map and protecting tests

- Workflow composition and validation: [`crates/reliquary-application/src/lib.rs`](../../crates/reliquary-application/src/lib.rs).
- Workflow modules include [`conversation_workflow.rs`](../../crates/reliquary-application/src/conversation_workflow.rs), [`ingest_publication.rs`](../../crates/reliquary-application/src/ingest_publication.rs), [`dream_publication.rs`](../../crates/reliquary-application/src/dream_publication.rs), [`management_workflow.rs`](../../crates/reliquary-application/src/management_workflow.rs), [`verified_copy_workflow.rs`](../../crates/reliquary-application/src/verified_copy_workflow.rs), and [`context_workflow.rs`](../../crates/reliquary-application/src/context_workflow.rs).
- Unit tests in `lib.rs` include forged-source rejection, bounded evidence checks, owner-lease idempotency, and reopen recovery. Workflow-local tests cover stale Dream preparation, project-source validation, entity reopen/retry, and copy divergence. Native import publication and source replay fixtures open new RELs through the current owner-session initializer.
- Public facade and external consumer contract are rooted at [`src/lib.rs`](../../src/lib.rs) and the external integration suite under [`tests/external-consumer`](../../tests/external-consumer).

## Extension seams

Put an operation here when it coordinates distinct owners, translates semantic results into named product policy, or needs to validate provenance across a workflow. Put a single-store replay invariant in that store or owner-session; put provider routing in Switchboard; put host authorization and process lifecycle in Gateway.

## Related docs

- [Owner sessions](owner-session.md)
- [Semantic processing](semantic-processing.md)
- [Gateway and host lifecycle](gateway.md)
- [Storage and durable state](storage.md)

## Notes

This page describes shared workflow ownership; individual user and operator procedures belong to their guide/reference owners.
