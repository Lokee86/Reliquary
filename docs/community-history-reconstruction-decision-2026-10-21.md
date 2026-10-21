# Community history reconstruction decision

Parent index: [Documentation index](INDEX.md)

Status: retain accepted membership snapshots and semantic names; no membership
codec/writer/upgrader change is justified by coordinate-only reconstruction.

## Purpose

Test the exact reconstruction claim before removing membership history that
participates in Dream/Freshness proofs, import evidence, lineage and names.

## Overview

The required claim is that owner UUID, historical Memory-graph cut, generation,
algorithm version, seed, resolution, quality and all explicitly retained policy
inputs uniquely recover the exact accepted sorted membership lists and Community
IDs. Re-running today's deterministic builder is insufficient if the accepted
owner contract admitted another valid historical partition.

The claim fails under the existing contract. A synthetic PHY with three Memory
nodes and two Topical edges was copied at the same graph cut. One copy received
a structurally valid one-community partition; the other received three singleton
communities. Both snapshots used generation1, graph cut1, algorithm2,
seed5495879477844901889, resolution1 and quality0. Community IDs were independently
computed from the same owner UUID and sorted literal memberships. Both owners
reopened successfully and exposed exactly their respective snapshots.

Thus even identical recorded coordinates and quality do not determine exact
accepted membership. Keeping extra data sufficient to distinguish arbitrary
accepted partitions retains membership information rather than proves it
reconstructible from Graph. No size-reduction claim is made.

## Consumer inventory

| Owner/consumer | Accepted fact that must survive |
| --- | --- |
| `community_store.rs` validation and replay | Historical Memory endpoints, sorted unique coverage, owner-derived Community IDs, generation and literal memberships; validation does not require equality with the builder output |
| `community_scan_merge.rs`, its plan/reduce/workers | Current algorithm uses graph node order, structural edges, shard policy and available worker configuration; historical algorithm1 is also accepted |
| `cva_freshness.rs` | Exact retained generation/graph proof pair, snapshot encountered at the accepted physical prefix, and members used by initial Dream graph proofs |
| `freshness_dream_journal.rs`, `cva_dream_journal_replay.rs` | Pending/completed source proofs keep original graph and Community evidence at the physical Begin prefix |
| `community_lineage.rs` | Intersections, split/merge/continuation identity and predecessor membership required by lineage/name inheritance |
| `community_codec.rs`, `dream_community_naming*.rs` | Exact non-recomputable name, source, contract, baseline Community ID and representative Memories |
| `imports/reconciliation.rs`, `reconciliation_interval.rs` | Literal normalized snapshot evidence and rebased destination graph/generation coordinates |
| `historical_manifest.rs`, `historical_import_tests.rs` | Exact snapshot/membership proof projection and current-versus-stale claims |

The inventory is grounded in direct source inspection. Grimoire discovery was
unavailable in this session, so no graph-provider completeness claim is made.
The explicit counterexample alone suffices to refute uniqueness.

## Decision and verification

Retain `CVACOMM1` membership snapshots and `CVACNAM1` name/evidence history under
the existing owners. Do not replace them with only graph cuts or rerun the
current builder during migration. Existing temporary owner transport continues;
no new Community converter or runtime compatibility path is added.

The independent public-owner probe reopened both complete physical copies and
asserted their exact different memberships. Its source lives in repository-local
verification scratch. The durable regression
`recorded_coordinates_do_not_uniquely_reconstruct_accepted_membership` passed in
the 2,096-test library run. Existing algorithm1 refresh, name lineage and
Dream/Freshness tests remain part of the proof-boundary integration gate.

## Related docs

- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Storage format](storage-format.md)
- [Architecture](architecture.md)

## Notes

No raw membership measurement is required for a rejected compaction strategy.
This decision does not relax owner validation or turn derived current community
views into new semantic authority. It preserves accepted historical evidence.
