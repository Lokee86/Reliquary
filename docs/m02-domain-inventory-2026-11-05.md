# M02 domain inventory — 2026-11-05

M02 is in progress. This inventory is not acceptance evidence.

| Reference contract | Rebuild starting point | Remaining proof |
| --- | --- | --- |
| Terminal Memory bodies in REL and PHY (`cva_history_prune.rs`, `phylactery_history_prune.rs`) | Owner-session verifies tombstones, shared bodies, source cut and pointer-free frame families; emits a retention manifest and reopens the copy. | Extend closure beyond the currently admitted frame families and verify historical cuts alongside other selected domains. |
| Retired-profile packed vectors (both reference prune facades) | Vector store can retire profiles and represent pruned packed backing; owner-level pruning and proof are absent. | Prove every vector reference belongs to selected retired profiles, including shared backing, before replacing payload with durable evidence. |
| Terminal REL Archive conversation content (`cva_history_prune.rs`) | Archive persists source turns, attachments and provenance; the Memory-only prune rejects unknown carriers. | Establish terminal conversation lifecycle and prove Memory, Episode, attachment, vector and retained source references before removing content. PHY must reject this domain. |
| Terminal Entity and Relationship descriptions (both reference prune facades) | Perception state is owner-local; Memory-only prune does not remove its descriptions. | Establish terminal lifecycle and revision-level markers with independent reopen and historical proof. |
| Reclamation (`storage_reclamation.rs`) | Verified copy and source proof exist; there is no general relocation pass. | Inventory reachable objects and inbound offsets per domain; relocate retained references and fail closed on unknown carriers. Verify semantic clocks, histories and retrieval. |
| Profile repair (`cli/src/repair_cmd.rs`) | Profile store can list and retire profiles; application has vector reads. | Plan exact active profile set and coverage owner-locally, then retire on a separately verified candidate without inference or re-embedding. |

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
