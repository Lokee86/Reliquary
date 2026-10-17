# Historical checkpoint transformation verification — 2026-10-12

## Purpose

Record Step 23 / H7's supported physical transformations and explicit conversion rejections for the real native REL checkpoint owner.

## Overview

Physical maintenance preserves `CVAFRS03` verbatim. Original owner/corpus/import/manifest/policy identity, admission/accounting anchors, superseded scoring IDs and frozen input evidence remain unchanged. Compaction may change actual file length and offsets: the checkpoint's original input byte hash/length is freeze evidence, not a condition that a compacted file must reproduce. Reopen still verifies the native publication cohort, immutable source content and original accepted Archive clock prefix.

`src/facade/historical_import_transform.rs` validates each physical candidate against its source before returning success. It compares checkpoint equality, pinned policy, accepted activity/semantic cuts, current canonical score records, and a streaming digest of the ordered retained Freshness policy/receipt/activation, Dream and Insomnia-completion payloads. Length framing makes that digest unambiguous. This disposable digest is never persisted and creates no second ledger or semantic clock. Comparing scores alone is insufficient: two independently accepted +25 receipts can both leave a Memory saturated at +100, while losing either receipt would break exact retry authority.

Existing copy/repack/reclamation owners retain physical mechanics and relocation. The facade adds checkpoint preservation checks; Freshness and Dream retain their existing authorities. Failed candidate validation removes the newly created output. Source files remain untouched. Principal source-identity changes reject before an output is created; an already matching principal is a supported unchanged repack.

| Operation | Step 23 disposition |
| --- | --- |
| Byte copy and passive reopen | Preserve accepted checkpoint and later live receipts/proofs |
| Identical or strict-prefix reconciliation, repeated reconciliation, prefix promotion | Copy the complete accepted side; preserve scores and exactly-once live-use retry |
| Project-backed attachment repack | Remove only eligible attachment backing, relocate physical references, retain checkpoint and receipt/proof payloads |
| Conservative storage reclamation | Remove proven unpublished garbage/free backing; retain all accepted import and later live authority |
| Repeated/regenerated import of an enrolled Archive | Reuse accepted Archive/Episode/Dream identities and checkpoint; no second reconstruction or score reset |
| Missing/conflicting principal rewrite after checkpoint | Reject source-identity conversion before output creation; matching-principal repack remains allowed |
| Divergent histories, including a merge that would reorder accepted activity | Reject before merge output/promotion; no verified score-cut conversion is available |
| Different accepted checkpoint authority for one import | Reject `ImportManifestConflict`; preserve both input files |
| Unsupported reconstruction policy, foreign owner or mismatched retained source corpus | Reject on mount before reconciliation can create an output |
| Legacy owner migration of an identified checkpoint file | Reject as already current; the internal historical-checkpoint migration guard also remains |

Prefix classification remains the existing Container payload-prefix rule. Equivalent compacted replicas that lack a common physical prefix can therefore classify as divergent and reject; this step does not invent a semantic equality/merge framework. A future conversion needs the full validated reconstruction manifest/contribution evidence and verified destination source IDs, rather than copying numerical anchors. The minimal accepted checkpoint does not retain that complete evidence or pre-cut score trajectory.

## Verification

Eight native H7 fixtures construct real Import Episodes, V6 Insomnia publication, settled first Dream passes and a committed `CVAFRS03`. They cover exact copy, pre-checkpoint prefix adoption, post-checkpoint live activity/use, twice-reconciled identical history, prefix promotion, actual attachment removal and orphan reclamation, score-saturation receipt-loss detection, divergent chronology rejection, unchanged sources on failed promotion, conflicting manifests, immutable base policy, regenerated Archive/Episode/Dream retries, principal conversion/no-change repack, and correctly framed owner/corpus/policy transplant rejection. The later live source deliberately has an earlier timestamp: it extends real activity without reordering the checkpoint's proven original prefix.

Focused command: `cargo test --locked -j 2 --lib historical_transform`. Test temporary files are under worktree-local `target/test-tmp` via process-local TMP/TEMP. Required formatting, root checking/full tests, detachable CLI, documentation and refreshed architecture gates accompany this commit. These fixtures use simulated extraction and require no new model-generated corpus.

The full root run passed 1,159 unit tests (one existing ignored test), integration suites and doc tests. CLI formatting/checking and all 19 CLI tests passed. Eight focused H7 fixtures passed; the final rerun also checks the valid same-owner, different pinned-base-policy replica. Documentation structural/change-impact checks and patch hygiene pass. Refreshed Pitlord enforcement reports no violations across six rules (66,409 source nodes, zero relationships).

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Frozen manifest policy](freshness-historical-manifest-policy.md)
- [Atomic checkpoint verification](freshness-historical-checkpoint-verification-2026-10-12.md)
- [Checkpoint recovery verification](freshness-historical-checkpoint-recovery-verification-2026-10-12.md)
- [Storage format](storage-format.md)
- [Current limitations](current-limitations.md)

## Notes

Step 24 still owns the complete manifest/proof/quiescence coordinator; Step 25 owns integrated retained-corpus acceptance and measurement. PHY remains excluded. No shared engineering standard or persisted format changes in this step. Static architecture evidence with zero extracted relationships cannot establish complete dependency coverage; native behavioral fixtures and source guards complement it.
