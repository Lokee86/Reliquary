# Rebuild implementation progress

Parent index: [Documentation index](INDEX.md)

## Purpose

Record incomplete capability slices and their verification.

## Overview

The independent rebuild is at `C:\!bin\workspace\Reliquary-rebuild`. Only the capability JSON marks acceptance; tests on scaffolds do not demonstrate product completeness.

- C01: current-only container header, frames, leases, streaming scan and explicit repair; nine focused tests pass offline.
- C02: Archive turn/branch/Episode replay, source identity, accepted activity, Echo and source-bound normal/project attachments have ten passing tests. Semantic publication, external VCS validation, import and other evidence remain incomplete.
- C03: initial Memory body/revision/provenance/history/tombstone replay; four tests pass offline. The pinned Memory ID golden now matches; expanded canonical fields now survive replay; global clocks, grouped publication, cross-owner authorization and prune proofs remain open.
- C08: an owner-scoped policy/event replay slice passes five tests; import settlement, cross-owner authority and FRS07 corpus proof remain open.
- C09: an Insomnia work/receipt slice passes two tests; owner-session maintenance, import and Dream evidence passes 23 tests. Claims are durable here while transient in the reference; authenticated admissions and integration remain open. External-index hints now have owner, source hash and span checks, but namespace authorization remains unproven.
- C10/C11: separate private owner aggregates are under review; caller-supplied admissions and scope checks remain unverified.
- S01: deterministic temporal parser and Switchboard routing/HTTP slices exist. Chronos detection, normalization and parsing pass 13 focused tests; matching, resolution and inference remain open. Switchboard eight tests pass with local loopback access; three HTTP mock tests fail to connect in the restricted environment. Full semantic and provider behavior remains to be verified.
- C04: typed graph relations and Community snapshots have three passing tests; authoritative cross-owner closure and derivation remain open.
- C05: Entity and mention persistence passes three tests; body-span proof, lifecycle and integration remain incomplete.
- C06: Ego Identity, Personality, Anchor and caller-supplied synthesis persistence has four passing focused tests covering owner binding, privacy and reopen; Memory watermark validation and integration remain open.
- S02: source-quote, routing-span and evidence validation pass four tests; model extraction, trusted Episode hydration and publication remain open.
- S03: pure Dream relation/lifecycle and naming helpers pass five tests; model classification, verifier and workflow publication remain open.
- S04: pure Entity evidence, ambiguity and guarded merge proposals pass five tests; model decisions, admission and owner integration remain open.
- S05: pure lexical, file/raw-turn, hybrid, semantic and graph retrieval slices pass 22 tests; source hydration, full candidate controller, embedding compatibility and authorization remain open.
- S06: pure live and historical score, F8, propagation and scheduling slices pass 68 tests; owner evidence and workflow integration remain open.
- S07: context budgets/assembly passes five tests, vector validation/inference passes six, and the configured embedding transport passes local mock tests with loopback access; packed-publication and workflow wiring remain open.
- S08: Git current-revision, private-index snapshots and historical project-reference lookup pass six tests with Lore disabled; default Lore compile and tests remain unverified after stalled builds, and uploads and broader integration remain open.

## Related docs

- [Capability plan](architecture/implementation-rebuild-capabilities-2026-11-03.json)
- [Workspace setup evidence](rebuild-workspace-setup-evidence.md)
- [Identity and corpus evidence](rebuild-f03-identity-and-corpus-evidence-2026-11-03.md)

## Notes

Passing focused tests are not full acceptance gates. The source corpus is not converted; release and cutover are not authorized by this progress record.





C07 has passing reference identity goldens and eight vector persistence tests; global publication, owner provenance validation and bounded payload caching remain open.


An early Ego test run used the Windows default temporary directory rather than the repository. Its test files were removed by the test helper; the helper now uses only a repository-local temporary directory. This did not use C:\tmp.












On 2026-11-04, a dangling unfinished Dream model module was removed to restore compilation. PHY scope replay now scans bounded frame prefixes and rejects 17 REL-only record families without mutation; owner-session tests pass 29/29. These checks do not authenticate caller-supplied admissions or establish complete storage acceptance.


On 2026-11-04, Container gained physical version reservation and replay with current REL-prefixed records (12 focused tests). Owner-session gained a grouped Insomnia carrier codec and structural checks (34 focused tests). Outer V7 framing was compared to the reference, but nested Memory bytes differ and per-record global versions, authorized grouped commit, projection and crash recovery remain open. Dream model classification and verification pass eight focused tests. Entity model evaluation passes seven tests; recurrence-gated admissions fail closed until verified same-identity proof exists. Insomnia stage binding checks pass eight tests. Gateway grant and mount work remains in progress. The full workspace test was interrupted after a stalled Lore build and is unverified.

Gateway registry and explicit owner-grant checks are drafted. Its Instance constructor is crate-private because trusted authentication/grant admission is not implemented; live mounts and final runtime tests remain open. A locked offline workspace check and package-DAG comparison passed after the latest integration; the full workspace test has not completed.


On 2026-11-04, the actual historical FRS07 implementation at `c6010fd30dd52acb81c667f7aed99b831307e49f` reopened all six disposable corpus copies with expected hashes and owner heads; copied bytes remained unchanged. This is not the full semantic fingerprint and does not repair the pinned F01 runtime. Grouped same-owner Insomnia commit and ordered Memory/Freshness/receipt replay have focused tests including stale lease and injected post-sync recovery; cross-owner routing and OS-level fault injection remain open. Gateway has trusted admission and a reduced owner queue with explicit Held state, but no complete host dispatch or streaming integration. V01, V02, V03 and V04 now have preparation/selection slices only; their authenticated writes and end-to-end gates remain open.


On 2026-11-04, Gateway writer-mounted queue finalization and crash-retry received integration coverage, branch preflight and submitter checks were hardened, and current Gateway suite passed 25 tests. Host callbacks remain crate-private until an adapter can be registered through a trusted boundary. Routed PHY source proof and externally supplied Memory references remain fail-closed pending sealed candidate evidence; no cross-owner V01 acceptance is claimed. The full history oracle remains unverified after link-time disk exhaustion; six historical FRS07 copies did reopen with expected heads and bytes.


Latest full locked offline workspace test with local loopback access failed in `lore_history::tests::lore_checkpoint_is_idempotent_and_reads_archived_revision`: Lore historical file read returned status 88. The failure reproduced with normal filesystem access. A temporary output-path experiment was reverted because its verification build stalled; the root cause remains unresolved. Other targeted suites, including 25 Gateway tests and the owner-session tests, passed. No full product gate is accepted.
