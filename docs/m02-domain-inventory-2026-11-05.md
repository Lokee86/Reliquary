# M02 domain inventory — 2026-11-05

M02 is in progress. This inventory is not acceptance evidence.

| Reference contract | Rebuild starting point | Remaining proof |
| --- | --- | --- |
| Terminal Memory bodies in REL and PHY (`cva_history_prune.rs`, `phylactery_history_prune.rs`) | Owner-session verifies tombstones, shared bodies, source cut and pointer-free frame families; emits a retention manifest and reopens the copy. | Extend closure beyond the currently admitted frame families and verify historical cuts alongside other selected domains. |
| Retired-profile packed vectors (both reference prune facades) | Vector store can retire profiles and represent pruned packed backing; owner-level pruning and proof are absent. | Prove every vector reference belongs to selected retired profiles, including shared backing, before replacing payload with durable evidence. |
| Terminal REL Archive conversation content (`cva_history_prune.rs`) | Archive persists source turns, attachments and provenance; the Memory-only prune rejects unknown carriers. | Establish terminal conversation lifecycle and prove Memory, Episode, attachment, vector and retained source references before removing content. PHY must reject this domain. |
| Terminal Entity and Relationship descriptions (both reference prune facades) | Perception state is owner-local; Memory-only prune does not remove its descriptions. | Establish terminal lifecycle and revision-level markers with independent reopen and historical proof. |
| Reclamation (`storage_reclamation.rs`) | Copy workflow removes orphan standalone Memory bodies and unmapped packed vectors in admitted frame families. The vector store relocates generation receipts. | Extend admission to other proved families and cover more reachability classes, including superseded indexes. A source with an unactivated generation currently fails normal REL replay and requires a separate recovery proof. |
| Profile repair (`cli/src/repair_cmd.rs`) | Read-only plan freezes the source and the exact active set to retain. REL verifies a complete current Archive generation and both owner kinds verify current Memory coverage. Execution appends retirements and a repair evidence record to an exact staged copy. | Combine with verified retired-vector pruning once its owner proof exists. |

The reference checks terminal status, rejects duplicate targets and repeated
pruning, and compares owner identity and semantic clocks after independent
reopen. Its 2.0 framing and physical offsets are not a 2.1 implementation
contract. Graph and Ego destruction require separate demonstrated contracts;
no such contract is assumed by this inventory.

Promotion safety correction: keep the exclusive owner lease through creation
of the original's hard-linked backup and the replacement attempt. If hard
links are unavailable, promotion stops with the source intact. The earlier
rename-first workaround released the lease before moving the original and did
not satisfy the frozen-source guarantee. The existing two terminal Memory
prune/promotion tests pass with hard-link access enabled.

## 2026-11-06 verification checkpoint

Profile repair checks owner authority, duplicate selections, stale source cuts,
current vector coverage, and destination collisions. The PHY fixture proves
that retirement preserves the original file as an exact prefix of the candidate
and that the retained row remains readable. The REL coverage fixture also
checks that newly accepted Archive activity invalidates a prior generation.

The REL reclamation fixture removes one unmapped packed matrix before an
accepted generation. Reopening verifies the relocated generation receipt,
preserved Memory history and retained vectors. A second pass emits an exact
copy, and an unknown carrier fails before output. Both workflows stage, sync
and independently reopen candidates before a no-replace hard link. A filesystem
that denies hard links refuses publication with the source still leased.

The architecture guard, formatting check, locked offline workspace build,
and serialized locked offline workspace tests passed. The test run used
local mock-server and hard-link access.

M02 acceptance still requires owner-level retirement-backed packed-vector
pruning, terminal Archive and Entity/Relationship pruning, broader reclamation
proofs, and failure-injection coverage at the copy, sync, and reopen boundaries.

## 2026-11-06 Step 3 — terminal semantic descriptions

The Entity and Relationship description-pruning slices are implemented and
owner-session verified. Entity summaries and native 2.1 Relationship summaries
are removed only after terminal tombstone proof, exhaustive retained revision
descriptors and independent owner reopen. Both REL and PHY are covered;
Relationship is first-class Perception data, not a Graph-edge description.
Entity pruning refuses active mentions, Graph edges and Relationship participants.
The complete evidence and deliberate 2.1 differences are in
`m02-semantic-description-prune-verification-2026-11-06.md`.

**M02 itself remains open**: broader reclamation reachability and fault
injection are not yet accepted, and separately verified single-domain prune
copies do not imply a combined multi-domain prune/reconciliation contract.
