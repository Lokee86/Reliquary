# Retrieval

Parent index: [Reliquary operator manual](INDEX.md)

## Purpose

Choose the correct retrieval surface without confusing source history, semantic Memory retrieval, and Graph traversal.

## Overview

Reliquary has several retrieval lanes because they answer different questions.

## Public composition boundary

The existing standalone `Cva`/`Phylactery` search methods remain supported. Public adapters live under `src/facade/`; Retrieval retains lexical policy, semantic selection and scoring/ranking over concrete owner reads. See [Phase 6 evidence](../dependency-convergence-phase6-execution-2026-10-23.md) for the source/API preservation record.

## Archive search

Use:

```text
search_archive(query, limit)
```

when you need historical source text from the REL Archive.

Archive search is:

- owner-wide across the REL Archive.
- lexical.
- read-only.
- model-free.
- bounded.
- unavailable on PHY because PHY has no Archive.

Use exact conversation/branch APIs when the user is asking about one known conversation path rather than owner-wide history.

## Memory-Web retrieval

Both REL and PHY expose:

```text
build_memory_retrieval_index(profile_id, subcentroids_per_community)
retrieve_memories_with_index(index, query_vector, config)
retrieve_memories(query_vector, profile_id, config)
```

The reusable index is transient derived state.

It combines:

- current non-archived Memory vectors.
- the selected Compatibility Profile.
- current Community routing when available/current.
- exact fine search inside the selected routing set.
- configured fallback behavior.

## Stale indexes

A cached retrieval index fails closed with `MemoryRetrievalError::StaleIndex` when relevant inputs change.

Rebuild it when that occurs.

Relevant watermarks include:

- Memory version.
- `memory_graph_version`.
- Community generation.
- selected-profile Memory-vector bindings.
- profile/vector dimensions/configuration.

Entity-only semantic Graph writes do **not** stale a Memory retrieval index.

## Vectors and profiles

Before semantic Memory retrieval, the owner needs a compatible embedding profile and Memory-vector coverage.

Configured runtime/CLI flows can establish/reuse profiles and build missing vectors. Development flows also provide simulated deterministic endpoints.

Do not assume a vector from one profile/model is interchangeable with another profile.

## Graph traversal

Use Graph traversal when you want relationships, not semantic nearest-neighbor retrieval.

- Memory reasoning/traversal: Memory projection APIs.
- Knowledge/Entity traversal: semantic Graph APIs.

## Provenance expansion

A retrieved Memory is a semantic answer candidate, not necessarily all evidence needed to answer a provenance question. Use its source references to recover supporting Archive/Episode context when needed.

PHY Memories retain identifier-only references to originating REL source identity; they do not contain copied REL transcript payloads.

## Related docs

- [Rust API](../api.md)
- [Community routing validation](../community-routing-validation-2026-08-29.md)
- [Graph and Communities](graph-and-communities.md)
- [Local configuration](../configuration.md)

## Notes

Do not add broader retrieval machinery simply because several indexes exist. The current design prefers simple deterministic candidate/routing stages with bounded semantic search.
