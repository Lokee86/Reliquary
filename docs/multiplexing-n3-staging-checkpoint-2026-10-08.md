# Multiplexing N3 staging checkpoint — 2026-10-08

Parent: [Next execution wave](multiplexing-phase5-next-parallel-execution-plan-2026-10-07.md)

## Purpose

Record the exact accepted E/F/G-import/G-external integration and remaining gates for final Reliquary convergence and the locked Warlock dependency cut.

## Overview

Status: **E5, F-core, G-import and G-external R0–R4 integrated and functionally verified on N3 staging; final N3 acceptance remains gated.**

Branch: `feat/multiplexing-n3-staging`. Worktree: `C:\!bin\workspace\Reliquary-n3-staging`.
Accepted N0 base: `999fbf7d85c9f892a1bf979293588033bcc1484b`.
E/F/import integration: `f139ba3b9b30c4a29faf6bb0fb9088f3e4e31911`.
Named E/F export cut: `760ce855cc443055739b45160ed05412e3f04c10`.
G-external integration: `b8a373b1b0bd50ccb4ade95bec24693c38eee547`.

| Lane | Accepted source |
| --- | --- |
| E5 | `874851ac4c81fb9bb29f897fefe421c335fac7d4` |
| F-core | `13df1ff14ea8d7bfb1777289968ff7d5c87b3998` |
| G-import | `99b9fa5e83f9c0052d5ceef0952a47275b3903cc` |
| G-external R0–R4 | `7c63eb8f0891c8086993d885025c290888a0fdb4` |

E adds native/full-host-contract live adaptation and narrow P3 cancellation/tool/artifact seams. F remains stateless management composition. G-import retains pure normalization and existing Archive/Insomnia publication ownership. G-external keeps host canonical history outside REL, with reference-only source snapshots, independently authorized source evidence, rebuildable search and shared transient Insomnia processing. Ordinary Memories and body-bound identifier-only provenance retain existing REL/PHY ownership.

Merge resolution preserves all E/F named exports, both adapter factories, native import rules and the external-source owner/rules. Gateway registers the accepted external modules alongside E/F. Fresh message authorization applies to search and completed processing replay; source sequencing fences private publication. Host callbacks, including metadata getters, run outside owner/admission closures. Canonical docs carry the combined current contracts.

The accepted N0 convergence branch and Warlock pins remain unchanged.

## Named export cut

All 31 E/F names remain exported through the Gateway → runtime_host → crate-root chain with private implementation modules. Existing downstream live/management consumers import them explicitly. Native import contracts keep `reliquary_memory::imports`; external contracts use explicit named exports under `reliquary_memory::external_source`. The external downstream consumer verifies authorized reference search/hydration, message access denial and source-body-free reopen. No duplicate DTO or compatibility-forwarding owner is introduced.

## Verification

| Gate | Result |
| --- | --- |
| Root formatting and locked check | Passed |
| Unrestricted root tests, including example compilation | 1,156 unit and 137 integration tests passed; none ignored |
| CLI formatting, locked check and tests | Passed; 19 tests |
| P2/P3 enforced ownership and P3 fixtures | Passed; 7 fixtures |
| P4 coverage | Passed; 75 fixtures, 161 public APIs |
| Management ownership guard | Passed; 25 fixtures |
| External ownership guard | Passed; 11 fixtures |
| Pitlord policy validation | Passed; 17 areas, 22 rules |
| Native Archive roundtrip smoke | Passed; 6 nodes, 2 branches, 2 fragments and 8 expanded references preserved after reopen |
| Documentation policy and whitespace | Both policy modes, unstaged and staged diff checks passed |
| Fresh architecture graph | Incomplete; exit 124 after 600 seconds during Rust analysis |

Logs are retained in `target/n3-g-external-verification/`: `runtime.json`, `tail.json`, `architecture.json`, individual command logs and the prepared native Archive graph fixture. Locked Cargo commands use one build job, one test thread and the independent D verification cache with stripped debug information. Existing compiler/linker warnings remain.

The first matrix wrapper used a nonexistent management-guard filename after root/CLI/P2/P3/P4 had passed. The corrected tail executes the existing `scripts/check_workspace_management.py`, external guard, policy validation, Archive smoke and documentation gates. This orchestration error required no source repair and does not replace any runtime result.

The fresh architecture refresh explicitly selected Rust, matching the authored Rust ownership policy under src/. Direct Python guards execute independently. The Rust adapter remained active but did not publish complete evidence within 600 seconds; the full subprocess tree was terminated. Arcana synchronization and graph-based Pitlord enforcement did not run. Direct guard/policy validation does not substitute for fresh graph evidence, and no Python dependency graph is claimed.

## Previous evidence

The E/F/G-import named-export cut passed 1,138 unit, 136 integration and 19 CLI tests. Its fresh graph attempt timed out after 240 seconds during Python analysis. G-external owning-lane acceptance passed 1,117 unit, 129 integration and 19 CLI tests; its Rust-only graph attempt timed out after 300 seconds. Earlier cache disk-exhaustion failures occurred before assertions and were superseded by successful unrestricted runs. The combined matrix above is the current functional evidence.

## Remaining convergence gates

| Gate | Current state | Required next outcome |
| --- | --- | --- |
| G-external R0–R4 | Accepted owner cut integrated and combined functional matrix passed | Core integration gate closed; host-specific adapters remain separate |
| B/C execution-only provider | E verifies the full ConversationExecution host contract; production Warlock does not advertise a durable pending queue | Resolve truthful execution-only readiness, stable ID/digest reconciliation and cooperative cancellation |
| Exact task-root retrieval and attribution | E supplies native accepted generation/tool/file receipts; broader C composition needs acceptance | Verify authorized C/A/F composition against this converged surface |
| Fresh architecture graph | Rust analysis timed out after 600 seconds before Arcana/Pitlord | Complete current Lexicon/Arcana evidence and graph-based Pitlord enforcement |
| Final C matrix and pins | C-close and historical Reliquary/Arcana pins remain unchanged | Advance accepted convergence, pin the exact revision, align Arcana and run every C gate |

F5 selective import/export remains optional at N3. G3/G5 host-specific adapters and external polling/profile migration/reconciliation/proposals remain separate tails. P6-H5 is separately accepted on N0; post-D port and measurements remain P6-R work.

## Resume sequence

1. Resolve B/C production provider conformance, actual C composition acceptance and fresh architecture enforcement; verify resulting source changes in their owning lanes.
2. Advance the N0 convergence line only after combined acceptance.
3. Pin Warlock to the exact accepted Reliquary revision and matching Arcana; verify one Arcana identity, locked Rust/provider/admission/conversation fixtures, frontend build and actual product acceptance.
4. Keep D deletion gated by final C/E/F acceptance; P6-R remains post-D.

## Documentation impact

- Inspected: canonical architecture/invariants/storage/maintainer/limitations/roadmap/API, accepted E/F/G records, current checkpoint, next-wave plan and shared standards.
- Updated: merged external-core current contracts, G plan/roadmap scope, this checkpoint and next-wave status.
- Not affected: historical owner-lane/N0 acceptance records, Warlock source/pins, host-specific adapters and native codec meanings.
- Compliance check: structural and changed-from origin/main policy modes and staged/unstaged whitespace checks passed.
- Known documentation gaps: actual C/provider, final convergence/pin and release acceptance remain explicit.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: preserve E recipient, F composition, native import/Archive and external reference-index owners; MemoryStore retains semantic Memory authority and Gateway retains authorization/sequencing/delivery.
- Pitlord enforcement impact: combined 17 areas/22 rules, native-import and external-source protections, 75 P4 fixtures plus management/external guards.
- Other verification impact: unrestricted merged root/CLI matrix, downstream named consumers, source authorization/recovery/privacy and native Archive reopen smoke.
- Known architectural gaps: fresh Rust graph analysis timed out after 600 seconds before Arcana synchronization and graph-based Pitlord enforcement; production provider/consumer and final pin gates remain.

## Related docs

- [E5 acceptance](multiplexing-p5e-e5-acceptance-2026-10-08.md)
- [F-core execution](multiplexing-p5f-core-execution-2026-10-08.md)
- [G-import execution](multiplexing-p5g-import-execution-2026-10-08.md)
- [G-external acceptance](multiplexing-p5g-external-acceptance-2026-10-08.md)
- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)

## Notes

This staging integration does not complete D deletion, final Warlock pinning or the Phase 5 release.
