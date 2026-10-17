# Historical Freshness production disposition — 2026-10-13

Parent index: [Documentation index](INDEX.md)

## Purpose

Close C12 by auditing the converged implementation against ADR 0039 and the frozen H1 contract, using the completed C1–C11 evidence at `cff0abd`.

## Overview

**Disposition: H1–H8 are complete and merge-ready as supported explicit, offline/exclusive, single-owner REL reconstruction. Historical reconstruction deliberately remains explicit/offline and is not a mounted or automatic migration path. PHY Freshness is not part of this release.** Live F8 is intrinsic REL behavior and local-only; there is no enable/disable switch.

This disposition supersedes the historical release block in the dated [Step 26 review](freshness-combined-production-review-2026-10-12.md). Its original diagnosis and the partial Step 22/24/25 records remain historical evidence. It does not merge a branch, enable a runtime lane, change policy constants, or write fixture scores.

## Audit and H1–H8 acceptance

| Gate | Final source and evidence | Disposition |
| --- | --- | --- |
| H1 identity, policy and owner proof | `historical_manifest.rs`, `historical_manifest_encoding.rs`, H1 native golden/drift tests; schema 2/policy 3, digest `f72f73b74a30a59de228c3a14a0e9344314c61724116efe3d822f22a25ca1fdb` | Complete: owner-derived source/cohort/graph/Community/vector/replacement proof; no caller-authored authorization or digest substitution |
| H2 timeline and availability | `cva_historical_freshness.rs`, `historical_planner.rs`, accepted-clock/cohort tests | Complete: contiguous accepted Archive activity is the only REL clock; timestamps cannot reorder it; missing/conflicting required evidence is unsupported or rejected |
| H3 authorized propagation | `historical_propagation.rs`, native semantic matrix and bridge oracles | Complete: strongest contribution once per source event/recipient; future/ineligible authorized bridges remain traversable without receiving credit; live semantic propagation remains disabled |
| H4 canonical semantic/Dream events | `historical_events.rs`, `historical_semantic.rs`, `historical_dream.rs`, native event/Dream matrix | Complete for current generated imports: mixed-fragment availability/identity, whole-cohort exclusions, actual first-cycle proof and no-link precedence; missing ordinary Dream proof remains unsupported |
| H5 chronological scorer | `historical_scorer.rs`, saturation/remainder/order/parallel oracles | Complete: existing -100/+100 per-event bounds and lazy decay; deterministic recipient/position/event ordering |
| H6 native checkpoint/recovery | `cva_historical_import.rs`, `historical_import_input.rs`, `historical_recovery_tests.rs` | Complete: private owner freeze/prepare/commit, input-drift rejection, CVAFRS03 append/sync, exhaustive checkpoint/use byte prefixes, passive reopen/due recovery and live-safe exact retry |
| H7 integration and preservation | `historical_reconstruction.rs`, `configured_runtime_historical.rs`, embedding/import/transform tests | Complete for offline/exclusive RELs: complete-generation barrier, optional bounded source-only embedding, cancellation/retry, already-enrolled preservation, copy/repack/reclamation/prefix reconciliation; unsupported divergent conversions reject |
| H8 retained acceptance/release | All three native C10 runs and final C11 root/CLI/docs/architecture checks in the [verification record](freshness-retained-normalization-verification-2026-10-13.md) | Complete for explicit opt-in: reproducible hashes, nonempty native contributions, coverage/graph/checkpoint/reopen/CPU/process-memory measurements and passing correctness gates |

The final production entry points accept only operation options. `Cva::reconstruct_historical_freshness` requires exclusive offline ownership and complete generation. The explicit endpoint/configured runtime operation supplies bounded missing-fragment embedding when requested. It does not generate Memories or rerun Insomnia/Dream for reconstruction. Fill completes before the final owner freeze; cancellation cannot report failure after accepted checkpoint publication. The mutable owner and byte digest are revalidated before commit. No second historical checkpoint store, persistent event queue, clock or migration registry remains.

Source inspection confirms the production historical scorer sorts contributions in memory and implements no disk-spill path. C10 reports whole-audit CPU, peak working set and peak pagefile; allocator-only temporary peaks and observed spill high-water are not separately instrumented. These are explicitly scoped measurements, not a throughput SLO or proof for arbitrary larger corpora.

## Semantic quality and activation policy

The retained measurements demonstrate correct native acceptance and deterministic replay, not labelled real-world relevance quality. The historical reconstruction deliberately uses the completed authorized graph rather than rebuilding past graph versions. Availability and cohort exclusions govern recipient credit, while historically future graph nodes may bridge traversal. That is the accepted approximation, even with genuine initial Dream proof.

| Corpus | Native actual-proof reconstruction: Fresh / Stale / Dormant | Stored checkpoint including labelled legacy approximation: Fresh / Stale / Dormant |
| --- | --- | --- |
| 14-day | 693 / 285 / 145 | 693 / 285 / 145 |
| 28-day | 764 / 27 / 2,056 | 2,841 / 0 / 6 |
| Ellis | 719 / 3 / 3 | 720 / 3 / 2 |

The 28-day difference shows strong sensitivity to historical legacy graph approximation. Neither distribution has human relevance labels that establish which default best serves users. **Keep reconstruction explicitly requested and default activation off.** This is a product policy decision, not an incomplete implementation gate. No threshold/scoring change is justified by these distributions alone.

The one-time normalizer and all fixture-only hooks are deleted. Its labelled legacy approximation remains in the normalized authoritative fixture checkpoints and the losslessly retained proof archive. It never became actual first-cycle proof. Current ordinary imports require genuine completed Dream evidence for that lane; absent proof does not automatically fall back to arbitrary final adjacency. There is no maintained pre-release migration framework.

## Supported boundaries and remaining work

- Single durable REL owner, native imported corpus, settled Insomnia/first-cycle Dream generation, verified source provenance and compatible current vectors. Unsupported or unisolatable evidence rejects rather than manufacturing history.
- Existing accepted checkpoints resolve exact retry before new owner-input checks and preserve subsequent legitimate live activity/use. Regeneration cannot award historical events again.
- Provider work stays outside mounted owner locks. The supported operation takes an exclusively held offline REL; it is not a Gateway authorization bypass or mounted management API.
- PHY Freshness remains deferred until an independently approved owner/activity-clock design. No REL counter, wall clock or PHY Memory version is borrowed.
- General relevance labels, semantic-policy upgrades, broader divergent conversions and arbitrary scale/SLO evidence remain separate work. Automatic or mounted historical migration is intentionally not part of normal runtime operation. Existing mounted Dream/Freshness integration limits remain documented independently.

## Verification

The audited Rust source is unchanged from `cff0abd`. C11 passed 1,266 library tests, 138 integration tests, 19 CLI tests, both exhaustive native recovery tests, all 10 design oracles, Archive roundtrip, formatting/locked builds, both documentation-policy modes and patch hygiene. All three explicit C10 native acceptance runs passed. Refreshed architecture enforcement passed 75 P4 fixtures, 161 API classifications, 11 external-ownership fixtures and 23 rules on snapshot `dd2d545418a3103ed844209427941ba294ff4fde8eee39c6a29267ff01993b99` (70,932 source nodes / 2,379 relationships).

C12 changes documentation and release disposition only. Both required documentation checks and patch hygiene are rerun on this disposition; native tests and architecture analysis are reused because no source, API, format, ownership or runtime behavior changes.

## Documentation impact

- Inspected: ADR 0039, H1/design/linear/completion plans, current architecture/API/storage/invariants/limitations, source coordinator/manifest/events/scorer, retained measurements and final verification.
- Updated: canonical C12 disposition, supported explicit offline REL status, completed H1–H8/C1–C12 markers and current docs/index/roadmap; dated blocked/partial evidence is preserved with supersession links.
- Not affected: Rust/CLI behavior, persisted bytes, fixture scores/hashes, live opt-in/default settings and broader Ego/PHY work.
- Compliance check: both structural/change-impact documentation checks and staged/unstaged patch hygiene passed on the final C12 tree.
- Known documentation gaps: no historical release-status gap remains; measurement/activation/mounted/PHY limits are explicit.

## Architecture impact

- Standards added or changed: none; accepted ADR 0039/H1 scope is implemented, with no new format or migration support.
- Ownership or boundary impact: no source change; existing Archive/Memory/Insomnia/Dream/Graph/Community proofs remain inputs and Freshness remains the only score/checkpoint owner.
- Pitlord enforcement impact: reuse the current passing source snapshot; no ownership rule/source changes require a refresh.
- Other verification impact: reviewed committed C1–C11/native corpus evidence; C12 documentation checks are separate from previously executed source tests.
- Known architectural gap relevant to Freshness scope: PHY activity semantics remain outside this release. Historical reconstruction intentionally stays offline/explicit rather than becoming mounted/default runtime migration.

## Notes

Merge-ready describes the reviewed feature scope; it does not assert that a merge or deployment occurred. The former experimental model does not become a production path. The initial frozen H1 descriptor remains version 3; this disposition introduces no new policy/version.

## Related docs

- [ADR 0039](decisions/0039-unified-deterministic-memory-freshness.md)
- [H1 policy](freshness-historical-manifest-policy.md)
- [Historical design and H1–H8 phases](freshness-historical-import-reconstruction-spec-2026-10-03.md)
- [C1–C12 completion ledger](freshness-historical-completion-execution-plan-2026-10-12.md)
- [C10/C11 verification](freshness-retained-normalization-verification-2026-10-13.md)
- [Current limitations](current-limitations.md)
