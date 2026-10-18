# Tombstone, import and history final verification

Parent: [Documentation index](INDEX.md).

## Purpose

Record the final acceptance of execution steps 29–68, including retained artifact promotion and practical limits. Earlier checkpoint records remain historical evidence.

## Overview

The integration starts at main `959e98c556603a0578e39362441c2d3f2901187f`, retains accepted completion commit `0d0abeb` and source-only replay commit `3ca0ddf`, and completes ordered owner replay, transform preservation, REL history pruning, activation hardening and retained repair. The implementation landed on main as `0b58de4`; its final closure commit adds optional CLI pruning, final evidence and completed execution pointers. Steps 29–68 are accepted within the explicit boundaries below.

## Implementation evidence

- Steps 29–33: owner-derived attributed readiness; coordinator-only completion authority; pending/failed recovery; sequential nonempty imports and native-only zero-marker behavior.
- Steps 34–35: finite physical replay preserves the left prefix, grouped Memory semantic records/bodies/routing and publication births, accepted Freshness effects, Dream and Perception evidence, processing checkpoints, local retry journals and attachments. Native destination reconstruction/completion regenerates authenticated coordinates. Same-ID normalized owner evidence deduplicates; conflicts reject before output. The seven reconciliation cases include nonzero output, initial processing and a byte-identical repeated merge. Transform tests compare exact native/legacy epoch authority and settlement records through reopen, repack and conservative reclaim.
- Steps 37–50: concrete terminal-owner descriptors authorize transformed-copy Archive, Memory, Entity, Relationship and retired-profile numeric pruning. Reopen authenticates marker/manifest membership and retained structural evidence. Active/shared references and authenticated import prefixes block removal. Retirement alone leaves numeric bytes readable. Missing payload without authorization remains corruption.
- Steps 51–52: Archive activation compares exact current FragmentIds. Memory requires complete current-body coverage. Historical partial rows remain inert; owner UUID and completed epoch boundaries gate operational activation. Explicit bounded historical fill uses the private import scope and does not perform premature live scoring. Legacy migration uses historical publication.
- Step 53: `rel repair SOURCE OUTPUT --keep-profile ID [--prune-retired] [--reclaim]` creates a new verified copy, frames eligible legacy history and retires other profiles. No provider call or re-embedding is performed. Existing outputs reject. Optional --prune-retired uses the existing transformed-copy owner operation; --reclaim then performs conservative maintenance.
- Step 64: direct source search finds no `ImportCheckpoint` or `import_checkpoint` runtime symbol. Persisted CVAFRS03 decoding is isolated compatibility behavior. Owners retain their existing clocks and persistence; Container does not gain semantic authority.

## Retained artifacts and retrieval

Steps 54–62 repaired and promoted all three authoritative RELs in `reliquary-fixtures/local/authoritative`. [Machine-readable repair evidence](../benchmarks/retained-rel-repair-2026-10-18.json) records source/output hashes, profiles, sizes and unchanged PHY hashes. Original RELs and metadata are backed up in the integration worktree's `.verification-tmp/authoritative-before-promotion` directory.

| Fixture | Canonical profile prefix | Promoted REL SHA-256 | Bytes |
| --- | --- | --- | --- |
| first14d | `38b2c3d1` | `d3a0a8023a91690fd7f9def8ab4b9f8095cb116a422f9cacf564576fd2e41055` | 31,853,182 |
| first28d | `e012fa51` | `de4754f9c7c03022f218dcd3e88a496ce84cc7b6e01e66d3e124278fcfd03d7a` | 80,056,100 |
| Ellis | `3aad70f4` | `a14e6b7499731dbf397a2d7d1758c393889b1bc53677b32b9e577fefd7b7a59e` | 19,561,460 |

The promoted form is **retired-only**. Optional destructive pruning/reclaim was deliberately not selected: the prefix-bound legacy frame authenticates the complete original source prefix. Every original byte is retained, recovered bytes are zero, and files grow only by compatibility framing and retirement records. This is the step-59 optional-cleanup decision, not an unperformed required migration. Fixture provenance and SHA256SUMS are updated; pre-existing PHY changes are preserved.

All three promoted fixtures also pass a fresh copy-only repair retry with the final seven-clock verifier: no new frame, retirement or byte is appended. [Final reopen evidence](../benchmarks/retained-rel-final-reopen-2026-10-18.json) records the unchanged hashes. The fixture repository already had uncommitted normalized REL/PHY/provenance changes before this work. Promotion is persisted there without sweeping that pre-existing fixture work into a source commit.

Steps 57–58 use the frozen gold with one explicitly selected canonical profile, before and after repair. [Before](../benchmarks/ellis-canonical-before-repair-2026-10-18.json) and [after](../benchmarks/ellis-canonical-after-repair-2026-10-18.json) have **identical results for all 27 cases**, including ranks, top-ten IDs and work counts. The surrogate-query comparison uses retained related-Memory vectors and performs no inference. Community-routed retrieval remains weak: 14/27 hits at ten, MRR 0.17813; hybrid obtains 27/27 at ten, MRR 0.39150. Repair introduces no regression. These are distinct from the prior live natural-language baseline, which used the redundant profile; its absolute metrics are not interchangeable with this deterministic parity comparison. Retrieval quality improvements are outside this lifecycle repair.

## Verification

Current-source acceptance:

- Library: **1,464 passed, 3 failed, 5 ignored**, 468.22 seconds, serial execution. Both exhaustive historical byte-prefix recovery tests pass. The three failures below already occurred in the accepted pre-change baseline; no task-related failure remains.
- Integration: **141 passed**, 23 test targets, zero failed.
- CLI: **23 passed**, zero failed, including copy-only repair, existing-output refusal and optional retired-vector prune followed by reclaim. A too-small numeric fixture initially failed the required shrink check; the test now uses realistic 1024-dimensional payloads rather than weakening that check.
- Library `cargo check --tests --locked --offline`: passed. CLI cargo check --manifest-path cli/Cargo.toml --locked --offline also passed. Both structural and changed-from-origin/main documentation policy modes passed. Final structural analysis follows below.
- Root and CLI formatting plus Git whitespace checks: passed. Documentation tests: zero tests, successful execution.
- Source guards: 92 multiplexing fixtures, 166 classified public APIs, 11 external-owner fixtures and production ownership boundaries pass. Full Lexicon refresh, Arcana sync and Pitlord enforcement pass: **23 rules, 58,617 source nodes, 433 relationships, no violations**. Snapshot: `sha256:b77a5b6f2c08d95eeafb46380a27207aa2fa35d4183353fa1ec893cb5e4810f3`. Analysis completed normally after about sixteen minutes; CLI-only closure does not change the policy's authored `src` evidence.

Pre-existing unrelated failures, all at the external test fixture's retained authority conflict (`runtime_host_gateway_external_tests.rs:118`):

1. `runtime_host::gateway::external_tests::external_completed_window_requires_fresh_message_authority_on_replay`
2. `runtime_host::gateway::external_tests::external_insomnia_routes_private_output_and_replays_without_inference`
3. `runtime_host::gateway::external_tests::external_partial_phy_commit_resumes_frozen_plan_and_missing_receipt_fails_closed`

The same names occur in the prior `3ca0ddf` broad result. This satisfies step 66's explicit separation clause; it is not a green full-library claim. Local mock HTTP and file-link tests were run with their required host access. Temporary output and Cargo caches stayed inside the worktree. No cache maintenance service was running when these final gates began, and none was started or reconfigured.

## Boundaries

This acceptance covers exclusive local REL operations. Mounted import completion, external PHY output authentication and PHY destructive pruning remain unsupported. Open epochs and outstanding source work cannot be reconciled. Unsupported payload families and incompatible destination owner populations reject rather than invent evidence. Pruning refuses authenticated source prefixes, grouped embedded Memory bodies, principal Entity descriptions and a second prune pass over an already pruned owner. None of these refusals grants a proof bypass.

## Documentation impact

- Inspected: repository/shared guidance, architecture, invariants, storage format, maintainer map, limitations, roadmap, version-history plan, ADR 0002, native import, API, behavioral contracts and all three active execution documents.
- Updated: canonical behavior/format/API/ownership/limitations, ADR 0040, execution pointers, this record, fixture provenance and benchmark evidence. The broken Ego index link and missing structural headings were repaired.
- Not affected: provider normalization schemas, compatibility policy v2 identity, unrelated Ego design work, retained PHY bytes and other workspace benchmark artifacts.
- Compliance check: structural and changed-from-origin/main documentation policy both passed; root/CLI formatting and whitespace passed.
- Known documentation gaps: operational and unsupported maintenance boundaries are explicit above; no stale source-only replay claim remains in canonical behavior docs.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: import/CVA coordinates ordered intervals and completion; concrete Archive, Memory, Perception, Graph, Dream, Freshness and vector owners validate their own evidence. Lifecycle remains vocabulary only. Container remains physical mechanics.
- Pitlord enforcement impact: full refreshed evidence passes all 23 rules, with no architecture violations or Arcana compatibility warnings.
- Other verification impact: full library, integration, CLI, transform, pruning, activation, retained-copy and retrieval-parity evidence is recorded above.
- Known architectural gaps: the explicit unsupported boundaries above and the three existing external-host test failures remain. No large-owner performance claim is added.

## Integration and cleanup

Four clean obsolete implementation worktrees were removed. Nine dirty obsolete worktrees were first saved as durable Git commits under `refs/archive/tombstone-import-20261018/`, with original staged/unstaged patches retained in `.verification-tmp/obsolete-worktree-archive`, then removed. Only ignored Cargo target caches were discarded. The [recovery manifest](../benchmarks/tombstone-import-worktree-archive-2026-10-18.json) identifies every snapshot. Original branch refs remain recoverable. Main and the active integration worktree remain; the latter retains verification logs and exact pre-promotion backups.

Steps 36/50 fast-forward the canonical source into main. All twelve original dirty/untracked main files were backed up byte-for-byte before integration. Existing Ego design and inventory edits, the domain inventory edit and unrelated benchmark outputs remain intact and uncommitted; the Ego inventory additionally retains the documentation-policy headings. The benchmark example gains explicit canonical-profile/hash selection, with its original source preserved in the backup. Earlier task-plan progress is superseded by this acceptance while its original files remain backed up. Both documentation-policy modes also pass in the main checkout with the preserved user edits. There is no abandoned singleton owner or active obsolete implementation checkout.

## Related docs

- [Remaining execution plan](tombstone-import-history-remaining-execution-plan-2026-10-17.md)
- [Execution contract](tombstone-import-history-execution-contract.md)
- [Current limitations](current-limitations.md)
- [ADR 0040](decisions/0040-terminal-tombstones-and-import-epochs.md)

## Notes

Luna delegation was used earlier in the execution. The remaining delegated lanes reached their usage limit; the primary agent completed implementation and verification directly. Existing user changes are preserved separately from the source integration.
