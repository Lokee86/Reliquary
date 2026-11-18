# Leiden Community recomputation specification

Parent index: [Planning](./INDEX.md)

## Purpose

Restore the original Leiden recomputation in native Reliquary 2.1. The current Community writer derives its expected membership from stored graph links. Those links can include an archived Memory. Recompute needs an exact partition of the active graph Memories at the time of publication.

## Overview

`reliquary rel community recompute <REL>` previews the partition with read access. `--apply` computes against the current input and saves under the existing `ManageRel` grant. Both use the same Root workflow through `WorkspaceManagement` and the Gateway. The CLI reports eligible and assigned Memory counts, Community sizes, and modularity. Apply also reports the accepted generation and whether it changed.

## Solver and input

The Graph crate owns the pure scan-and-merge calculation. Port the solver, shard planner, workers, and weighted reduction from `f431401e` with the original `leiden-rs` 0.8.1, fixed seed, resolution 1.0, and 2,048-node shards. Keep the graph-local scan order and stable output across worker counts. Application invokes the solver through the existing immutable model boundary.

The owner session builds the input from the selected REL's stored graph Memory closure. A Memory is eligible when its current head is readable and active under the existing lifecycle rule. Retired, superseded, and terminal Memories fall outside that set. Sort stable Memory IDs before assigning solver indices. Each active structural link between two eligible Memories contributes one undirected pair, deduplicated across direction and relation kind. An Entity association can place a Memory in the existing closure. It contributes no structural edge. Eligible Memories with no structural neighbors form singletons. Empty input yields an empty partition and zero modularity.

## Save contract

1. Capture owned input and the owner identity, history generation, and Memory, Graph, and Community versions under the Gateway's REL read. Solve after releasing that lock.
2. On apply, reacquire the authorized `RelWriter` and compare the captured versions. A changed input or Community generation returns the existing stale error before append. The public operation computes its own partition.
3. In the writer, use the same eligibility function as input capture. Require the union of groups to equal that set and require each Memory to appear once. Canonicalize member and group order and derive Community IDs from the current owner and membership using the existing ID function.
4. Sync one Community frame before returning success and use the normal owner-session write recovery behavior. An identical current partition at the same Graph version reuses its generation. A stale result or invalid partition appends nothing.

Only the Community generation is published. The operation reads the existing Memory and Graph state and uses no model route. The original Community IDs and names continue to apply when membership is identical.

## Storage and compatibility

Read both existing 2.1 forms. Take the `RELCMP01` codec and frame recognition from `dca06ffa`, including replay, graph-version validation, and maintenance classification. Write `RELCOM01` if the validated membership covers the full stored graph closure. Use `RELCMP01` for a valid proper subset of that closure. A recomputed partition can be complete for active Memories and still need the subset record.

Active eligibility is an exact save-time check. The captured Memory version detects a stale write, and reopen validates the stored membership under the existing Graph-cut rules for its frame type. Record history retains the prior generations and memberships, including the fixture's original subset. Fresh-history rewrite uses the matching publisher for valid projected subset memberships it already carries forward, under its existing authority checks. This is a change within the existing 2.1 record families.

## Completion

The public Root and CLI path can preview, apply, and reopen the same partition on one inactive 28-day copy. Every eligible Memory belongs to one Community, archived Memories have no new assignment, and the original 12-community generation remains readable. The [execution plan](leiden-community-recomputation-plan-2026-11-18.md) defines the checks and the acceptance record.

## Related docs

- [Current limitations](../limits/current-limitations.md)
- [Semantic processing](../architecture/semantic-processing.md)
- [CLI reference](../reference/cli.md)
- [Development workflow](../development/workflow.md)

## Notes

The delivery surface is the Reliquary core, CLI, and existing 2.1 persistence.
