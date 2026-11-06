# M04 independent historical-proof re-verification — 2026-11-06

**Status: historical-revision defect corrected and independently retested; pending final GM gate.** Earlier GM independent audit is preserved as the defect oracle. Rebuild runtime supports native REL/PHY 2.1 only; reference oracle `Reliquary-reference` `f62b1fb4`.

## Implementation evidence

- The historical owner proof now hashes a schema/version-tagged canonical JSON value (ordered semantic arrays and sorted object keys) instead of Rust Debug text. Owner ID, file kind, selected semantic cut, retained provenance and all accepted revisions are bound to SHA-256.
- Perception historical projection retains all Entity, mention and Relationship revisions and terminal/prune evidence through the selected cut; it no longer only hashes the last value per stable identity.
- Archive historical projection retains ordered Branch revisions as well as turns, Episodes, fragment definitions, imported receipts, tombstones and activity cuts.
- Graph publication history, Community history, private Ego Identity/Personality/Anchor/Synthesis revision history and replayable Freshness evidence are included as supported; bounded record count uses the retained-history extent, not just current entity counts.
- Independent reopen, owner-kind boundaries, temporal/future cuts, grouped Memory and Freshness receipt boundaries and unavailable/pruned payload semantics remain verified and no physical offsets are asserted as semantic clocks.

## Adversarial checks

- `gm_audit_historical_proof_distinguishes_retained_intermediate_entity_revisions` **passed** after formerly failing: same first/final Entity revisions, different intermediate revisions must yield different proofs.
- `phy_proof_distinguishes_converged_relationship_revision_histories`, `phy_proof_distinguishes_converged_graph_publication_histories`, `phy_proof_distinguishes_converged_ego_revision_histories_and_reopens_stably`, `phy_proof_binds_cross_identity_perception_acceptance_order` **passed**.
- Newly authored independent `rel_proof_binds_intermediate_archive_branch_revisions` **passed**: two RELs with equal final Branch head and identical accepted turns but different intermediate Branch revisions yield distinct historical digests.
- `cargo test -p reliquary-owner-session historical_proof --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **8 passed** before the additional Archive Branch test, which passed separately; repeat in final GM full-suite verification.

## GM dependencies

This evidence does not imply arbitrary restoration, format conversion or generalized time travel. M01–M03 transformed-history tests, full owner and workspace tests, format/architecture gates and independent combined matrix must pass on a committed baseline before GM is accepted. Original `docs/m04-historical-read-proof-parity-acceptance-2026-11-06.md` retains its superseded acceptance history as the reason for this re-verification.
