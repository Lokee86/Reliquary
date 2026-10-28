# Retrieval

Parent index: [Reliquary operator manual](INDEX.md)

## Purpose

Choose the correct retrieval surface without confusing source history, semantic Memory retrieval, and Graph traversal.

## Overview

Reliquary has several retrieval lanes because they answer different questions.

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

The deterministic lexical and Memory routing engine is being extracted to
`reliquary-retrieval`. Root still reads Archive, Memory, Graph, and vector-store
facts, validates the owner snapshot, hydrates source text, and maps service
policy errors to the existing product errors. The extraction preserves the
REL/PHY methods and their result types; remaining verification limitations are recorded in the extraction evidence.

## Stale indexes

A cached retrieval index fails closed with `MemoryRetrievalError::StaleIndex` when relevant inputs change.

Rebuild it when that occurs.

Relevant watermarks include:

- Memory version.
- `memory_graph_version`.
- Community generation.
- selected-profile Memory-vector bindings.
- profile/vector dimensions/configuration.

The service compares the Memory version, Memory-Graph version, Community
generation, selected-profile binding count, and routed subcentroid setting.
Entity-only Graph changes leave the Memory-Graph watermark unchanged.

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
- [Retrieval extraction evidence](../retrieval-service-crate-extraction-evidence-2026-10-28.md)
- [Community routing validation](../community-routing-validation-2026-08-29.md)
- [Graph and Communities](graph-and-communities.md)
- [Local configuration](../configuration.md)

## Notes

Do not add broader retrieval machinery simply because several indexes exist. The current design prefers simple deterministic candidate/routing stages with bounded semantic search.
