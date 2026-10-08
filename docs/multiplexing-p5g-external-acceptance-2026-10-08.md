# Multiplexing P5-G external-core acceptance — 2026-10-08

Parent: [P5-G interoperability](multiplexing-phase5g-interoperability-2026-10-06.md)

## Purpose

Record framework-neutral external canonical transcript R0–R4 implementation and functional acceptance on its owning branch, before exact-commit N3 import.

## Overview

Branch: `feat/multiplexing-p5-external`. Accepted N0 base: `999fbf7d85c9f892a1bf979293588033bcc1484b`. The commit containing this record is the accepted owner cut. N3 import and final Warlock pins have their own convergence gate.

The host retains canonical text, source authorization and transcript lifecycle. REL persists bounded descriptors, IDs, hashes, UTF-8 offsets, lexical term hashes, packed vector bindings and synchronized cursors. Feed gaps require explicit manifest rebuild. Reference search validates a fresh authorized manifest before delivery; namespace access alone cannot authorize each candidate. Changed evidence fails stale and absent authorized rows fail closed.

External processing shares Insomnia ledger/ownership/wording, enrichment and Chronos machinery. It publishes ordinary REL/PHY Memories and identifier-only body-bound provenance without Archive nodes, Episodes, Fragments or Echo source copies. Private publication holds the source REL sequencer across source-cut validation and the recipient write. Frozen destination-bound plans and owner-local receipts recover partial PHY/REL publication; these are not a cross-owner transaction.

The public `external_source` module uses explicit named exports. All host callbacks, including namespace/version getters, stay outside owner locks and admission leases. Original grants, mount epochs and source cuts fence installation and delivery.

## Acceptance evidence

| Concern | Evidence |
| --- | --- |
| Durable reference index and cursor | Reopen/no-op replay; interrupted preparation retains the old committed cut, then retries exactly once |
| Source lifecycle | Hash replacement, visibility/rewind/compaction inactivity, reactivation and conversation deletion replace/suppress source rows |
| Corruption and rebuild | Actual physically valid Container with a corrupt logical index reopens, retains semantic Memories/provenance, requires rebuild and reopens the repaired index |
| Canonical body ownership | Raw REL bytes contain no full canonical index fixture body; no native source history is fabricated |
| Search authorization | Fresh message authorization, stale hashes before index refresh and index mutation during final host validation fail closed |
| Semantic processing | Shared project/private derivation; source chronology; no fake native provenance; atomic owner-local receipts and reopen |
| Private recovery | Frozen-plan partial recipient retry without inference; missing private receipt fails closed; foreign PHY evidence is inaccessible |
| Admission and callback boundary | Host hydration can revoke grants without holding an admission lease; original grant fences installation; metadata getters covered by source guard |
| Public consumer | Downstream source trait, verified Gateway admission, vector search, UTF-8 hydration, message authorization and reference-only reopen |

## Verification

| Gate | Result |
| --- | --- |
| Root formatting and locked check | Passed |
| Unrestricted root tests, including example compilation | 1,117 unit and 129 integration tests passed; none ignored |
| External recovery and index tests | 11 passed |
| Gateway external tests | 7 passed within the root suite |
| CLI formatting, locked check and tests | Passed; 19 tests |
| Direct ownership guards and fixtures | P2/P3 enforced; 7 P3, 46 P4 and 11 external fixtures passed |
| Pitlord policy validation | 16 areas, 9 rules validated |
| Native Archive graph-corpus smoke | Passed: 2 nodes, 1 branch |
| Documentation policy and whitespace | Both policy modes and diff check passed |
| Fresh architecture graph | Timed out after 300 seconds in Rust analysis; enforcement outstanding |

Commands use locked dependencies, one Cargo build job and one test thread. Focused filters assert nonzero execution. Logs are retained in `target/g-external-acceptance/`: `final-runtime.json`, `tail.json`, individual logs and prepared native Archive JSONL/smoke output.

The first CLI test build stopped before assertions with Windows disk exhaustion (error 112). Only task-generated cache outputs were relocated with SHA-256 verification and original-path links. An independent cache copy on D was then used for successful CLI and unrestricted/final verification; older caches/backings remain available.

The bounded architecture refresh selected Rust because the ownership policy covers authored Rust under `src/`; Python guards execute directly. It timed out after 300 seconds while the Rust adapter was actively analyzing, and its subprocess tree was terminated. Arcana synchronization and graph-based Pitlord enforcement did not complete. Policy validation/direct guards do not substitute for fresh graph evidence; no Python dependency graph is claimed.

## Remaining limits

Host polling and Hermes/ACP/additional framework adapters, compaction proposals, embedding-profile migration, automatic frozen-plan pruning and external-state divergent reconciliation remain separate work. Source deletion suppresses source evidence rows and does not automatically withdraw independent semantic Memories. Physical Container/vector corruption retains existing strict recovery.

A namespace uses one profile; manifests are bounded to 8,192 messages. Adapters must bound transport/provider allocation before returning responses. Search fails the request if any candidate is outside the fresh authorized manifest rather than returning a partial filtered ranking. Host changes after the check still require fresh authorized canonical hydration. Frozen plans require protected/retained Gateway operational state in addition to REL/PHY files.

## Documentation impact

- Inspected: canonical architecture, invariants, storage, maintainer map, limitations, roadmap/API/development, G integration plans and shared standards.
- Updated: current external-core facts, bounds and ownership in canonical docs; graduated R0–R4 in plans/roadmap; this acceptance record and Pitlord operations.
- Not affected: historical N0/E/F/G-import evidence, Warlock source/pins, host-specific adapters and native codec meanings.
- Compliance check: both documentation policy modes and whitespace checks passed on the final documented tree.
- Known documentation gaps: final N3/C/Warlock/release acceptance remains separately gated.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: one external-source owner holds reference contracts and rebuildable index state; MemoryStore retains Memory authority; Gateway retains grants, sequencing, delivery and operational recovery plans.
- Pitlord enforcement impact: explicit external-source area, three native-history prohibitions, 11-fixture callback/publication guard and P4 classification for nine external methods.
- Other verification impact: root/CLI, downstream public contract, logical corruption/reopen/repair, cursor interruption, privacy/lifecycle/recipient recovery, native Archive smoke and direct ownership guards.
- Known architectural gaps: fresh Rust graph analysis timed out before Arcana/Pitlord enforcement; combined N3 and actual provider/consumer gates remain outstanding.

## Related docs

- [Architecture](architecture.md)
- [Rust API](api.md)
- [Storage format](storage-format.md)
- [Current limitations](current-limitations.md)
- [External integration plan](conversation-index-integration-plan.md)

## Notes

Functional acceptance is scoped to the owning branch; fresh graph and combined N3 acceptance remain separate gates.
