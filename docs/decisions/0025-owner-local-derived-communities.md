# ADR 0025: Owner-local derived Graph communities

Parent index: [Architectural decisions](INDEX.md)

## Status

Accepted and implemented — 2026-08-27. Semantic naming amended by [ADR 0031](0031-dream-derived-community-semantic-names.md). Recursive semantic subdivision and Perception processing-neighbourhood use are amended by [ADR 0033](0033-perception-entities-observations-and-ambiguity.md).

## Context

Dream publishes durable semantic relationships into the owner-local Memory Graph for both Reliquary and Phylactery. That Graph is authoritative relationship state, but a large Graph also needs deterministic structural regions for traversal and presentation without asking a model to classify every region.

Community detection must not become a second relationship authority. It must not rewrite Dream edges, create semantic Graph supernodes, consume semantic version tickets, or introduce cross-owner Graph state. A monolithic Leiden baseline established the partition semantics, but synthetic scaling showed that one whole-Graph Leiden invocation becomes needlessly expensive as the Graph grows.

Lexicon already uses a related deterministic scaling pattern: bounded parallel scans produce local results that are combined through a deterministic reduction tree. Community detection adopts that scan-and-merge pattern while preserving all structural edge evidence across shard boundaries.

## Decision

Reliquary persists owner-local **derived community snapshots** over the current Graph.

- A snapshot records a monotonic derived `generation` plus the exact `derived_graph_version` from which it was computed. Under ADR 0036 this field is the owner-local **Memory Graph projection watermark** (`memory_graph_version`), not every semantic Graph mutation. Community generation consumes no `CVAVERS1` semantic ticket.
- A snapshot is current only when both its `derived_graph_version` and its community algorithm version match current state.
- Active oriented Graph relationships project to unordered structural Memory pairs. Multiple active semantic relationships between the same unordered pair collapse to one unweighted structural edge. This projection is clustering input only.
- Algorithm version `1` is the readable legacy monolithic Leiden baseline.
- Algorithm version `2` uses deterministic graph-local scan-and-merge. A BFS scan order is divided into fixed 2,048-node leaf shards. Leaf Leiden runs may execute through a bounded worker pool. Their community summaries reduce through a fixed binary merge tree.
- Every original structural edge enters the reduction exactly once. Same-shard edges enter the leaf run; a cross-shard edge enters the lowest merge parent containing both endpoint shards. Parent summaries preserve child structure as weighted coarse edges and self-loops.
- Every Leiden invocation uses modularity resolution `1.0`, fixed seed `0x4c454944454e0001`, and deterministic internal execution. Worker count controls scheduling only and must not affect the resulting snapshot.
- Reduction depth is logarithmic in leaf-shard count. Total work still necessarily scans the structural Graph; scan-and-merge is not a claim that complete clustering becomes `O(log N)`.
- Merge decisions are irreversible within one pass, so v2 is a hierarchical approximation to one monolithic Leiden run rather than a guarantee of partition identity. Measured modularity and timing are tracked in [Community scan-and-merge benchmark — 2026-08-27](../community-scan-merge-benchmark-2026-08-27.md).
- Leiden and scan-and-merge never mutate Graph authority. Every current Graph node belongs to exactly one community in a current snapshot. REL and PHY are always clustered independently.
- `CommunityId` remains deterministic exact-membership identity: SHA-256 over the durable owner UUID plus sorted member `MemoryId`s. It does not promise continuity after a membership change.
- Human-facing names are outside Community identity and membership. As amended by ADR 0031, Reliquary persists clock-neutral Community-name metadata keyed to exact `CommunityId`, with provenance distinguishing Dream-generated from explicit user-authored names. User naming takes precedence over Dream generation without changing membership, Graph relationships, or traversal semantics.
- A real divergent semantic reconciliation repack may discard Community snapshots and rebuild them from merged Graph authority. A semantic no-op reconciliation preserves canonical bytes, including an existing snapshot.

The Leiden dependency remains isolated behind private community machinery. Reliquary owns the semantic projection, deterministic scan/reduction policy, and persisted derived snapshot contract.

## Consequences

Large owner-local Graphs can use multiple cores for bounded leaf scans and reduction tasks instead of placing the entire Graph into one Leiden invocation. The serial reduction path grows with the number of merge levels rather than the number of shards.

The computation remains a complete Graph organization pass. There is no changed-region invalidation or cached reduction-tree reuse yet. Those optimizations should be added only if real workloads show the complete pass is materially expensive.

Older v1 snapshots remain reopenable because they are derived state rather than semantic authority. Explicit refresh republishes the current v2 partition as the next generation; an old algorithm version is not reported as current even when its Graph watermark still matches.

Community identity still changes when exact membership changes. Deterministic split/merge lineage is derived separately from consecutive snapshots and does not make IDs continuity-preserving; see ADR 0032. Community-aware traversal is implemented separately, while generated/user Community naming and user-over-Dream precedence are governed by ADR 0031.

## Rejected alternatives

### Rewrite or augment Graph during clustering

Rejected. Leiden detects organization implied by existing topology; it does not establish semantic Memory relationships. Persisting synthetic semantic edges or supernodes would create competing Graph authority.

### Use an LLM to define communities

Rejected for the structural layer. Community membership is deterministic Graph-derived state. Optional human-readable naming belongs to presentation metadata and may be model-assisted later without becoming clustering authority.

### Partition by dense node insertion order

Rejected after benchmarking. It was fast but could cut deliberately interleaved semantic regions across every shard and materially reduce modularity. Deterministic graph-local BFS ordering retained the scaling benefit while substantially reducing that sensitivity.

### Implement an approximate home-grown Leiden/Louvain routine

Rejected. Each clustering stage uses the existing Leiden implementation behind a narrow adapter rather than introducing an algorithm that merely resembles Leiden.

### Promise stable IDs across membership changes now

Rejected. Split/merge continuity requires explicit reconciliation semantics. Exact-membership IDs avoid pretending that changed clusters already have a defined durable identity.

## Verification

`src/semantic_graph/community_tests.rs` protects persistence/reopen, freshness, REL/PHY isolation, durable-owner requirements, and v1-to-v2 refresh compatibility.

`crates/reliquary-graph/src/community_scan_merge_tests.rs` protects cross-shard community recovery, parent-level merging, worker-count determinism, and equivalence between stored root quality and modularity recomputed over the final partition on the original structural Graph.

The ignored release benchmark in `crates/reliquary-graph/src/community_scan_merge_bench.rs` generates locality-friendly and deliberately interleaved sparse planted-community graphs and compares v2 wall time and modularity against monolithic Leiden. Frozen results are recorded in [Community scan-and-merge benchmark — 2026-08-27](../community-scan-merge-benchmark-2026-08-27.md).
