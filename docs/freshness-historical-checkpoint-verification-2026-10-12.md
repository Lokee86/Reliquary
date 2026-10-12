# Historical import checkpoint verification — 2026-10-12

## Purpose

Record Step 21's atomic replacement boundary for historical REL Freshness reconstruction.

## Overview

`Cva::freeze_historical_import` captures the complete native import input; `FrozenHistoricalImport::prepare` scores disposable historical contributions; `Cva::commit_historical_import` publishes one complete `CVAFRS03` payload. The operation belongs to the existing Freshness owner. It allocates neither Memory revisions nor global semantic versions and creates no permanent historical event queue.

The first supported shape is deliberately narrow: one identified REL, complete Import Episodes with native V6 Insomnia completion/birth proofs, settled first-cycle Dream passes, default base policy, no prior semantic activation or accepted use, and a chronology matching actual Archive activity. Ordinary enrollment and unrelated Memories reject. Structured provenance conversion is unsupported. Current nonarchived, nonsuperseded knowledge/canonical Memories receive replacements; unsupported inactive members remain untouched only when provisional Dream effects are isolated from the replacement set. A mixed effect set rejects the attempt.

The full original cohort, source-corpus digest, canonical import ID, supplied manifest digest, fixed H/F8 policy digests, input length/hash and global/Archive/Graph/activity cuts travel with the final records. Admission uses the maximum necessary provenance/Import Episode evidence for the original publication cohort. Input bytes are hashed again immediately before append; any intervening mutation invalidates the attempt, including same-turn changes. The prepared record map and due dispatcher are validated offside before durable publication; the live projection switches only after append and sync.

Original birth/admission receipts and Dream proofs remain intact. Only listed provisional Dream scoring identities are excluded from canonical score replay, and checkpoint records replace provisional scores/admission anchors. Old accepted receipt retries still resolve against retained authority. Normal later Dream, semantic and genuine +25 accepted-use events apply above this baseline, including events accepted at the checkpoint's activity cut after publication. Exact checkpoint retries resolve before drift checks and never replace later live state. A differing manifest or final record map rejects as `ImportManifestConflict`.

This API accepts a canonical manifest digest and caller-produced contributions; it does not construct or validate the full manifest's graph/Community/vector/event evidence. The Step 24 coordinator must perform those checks and quiesce runtime workers. The facade verifies native publication/cohort/source/clock/input authority, while contribution-proof validation remains that coordinator's obligation. This is an explicit development API, not automatic importer activation.

## Verification

Native fixtures exercise complete import publication, historical admission/tail decay, atomic score replacement, unchanged Memory revision and settled Dream journal, passive reopen, exact retry, same-cut accepted use, live receipt retry, conflicting manifests, input drift, unprocessed tail, live completion and unfinished generation rejection. Storage fixtures exercise every truncated payload prefix and every one-byte corruption, unchanged records on rejection, signed -100 replacements, excluded provisional Dream effects, original event retry identity and post-checkpoint continuation. The sparse scorer also verifies one origin event can credit multiple recipients once each; duplicate identity is scoped to recipient plus event.

The full root gate passed 1,148 unit tests (one existing ignored test), integration suites and doc tests. After the final separation of full Graph input cuts from Memory Graph query cuts, unit/integration/doc tests were rerun; a storage fixture verifies that unrelated full Graph progress cannot invalidate a valid Memory Graph query. CLI formatting/checking and all 19 CLI tests pass. Documentation structural/change-impact checks and patch hygiene pass. Architecture refresh enforces six rules without violations (66,096 source nodes, zero relationships); the absence of extracted relationships limits static dependency evidence.

Step 22 owns the broader crash-prefix/due-index recovery matrix; Step 23 owns transformation support. Divergent reconciliation and legacy migration currently reject accepted checkpoints explicitly. Earlier-than-checkpoint score queries reject because this compact snapshot does not retain reconstructed historical trajectories.

## Related docs

- [Linear implementation plan](freshness-semantic-historical-linear-implementation-plan-2026-10-11.md)
- [Frozen manifest policy](freshness-historical-manifest-policy.md)
- [Storage format](storage-format.md)
- [API](api.md)
- [Architecture](architecture.md)

## Notes

Historical reconstruction remains REL-local and optional. Physical bytes, global/Archive/Graph versions and wall time are input evidence, never additional decay clocks. Full retained-corpus acceptance and performance measurement remain Step 25. Static architecture refresh may report no extracted relationships; source guards and runtime tests complement that limited graph evidence.
