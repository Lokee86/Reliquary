# Dream service extraction evidence

## Purpose

This record tracks implementation and verification against the [specification](dream-service-crate-extraction-spec-2026-10-28.md). The starting HEAD was `494e1c2a` on `refactor/crate-first-restart`. The pre-existing Graph evidence edit was not modified.

## Overview

The Dream service owns candidate ranking, classifier and verifier contracts and calls, bounded pair/frontier evaluation, publication decisions, duplicate and canonical planning, lifecycle intents, and Community naming policy. Root supplies authorized facts and retains Graph and Memory writes, current snapshot and user-name checks, pair history, first-cycle settlement and processing epochs. Dream Store remains the durable first-cycle journal owner.

The direct Dream dependencies are Chronos, Graph, Memory Core, Switchboard and Vectors, plus serde_json. Dream Store and Owner Lifecycle do not depend on Dream service. Root public classifier and verifier types are re-exported; processor and Community namer remain owner-facing adapters.

Verification so far: the Dream service passed 14 unit and 9 external-consumer tests; Dream Store passed 17 tests. The locked offline workspace all-targets check passed. The standalone CLI check and all 32 CLI tests passed, as did workspace and CLI formatting checks and `ddocs check --indexes --links`. The pre-change classifier filter passed six tests. A restricted full-library run completed with 1,451 passes, 15 failures and 5 ignored; the failed local loopback and filesystem cases passed with the required local access, and the three external gateway cases passed with a shorter repository-local temporary path. The full rerun with local access and a short repository-local temporary path passed: 1,466 passed and 5 ignored. The five ignored cases are the hash-pinned retained REL corpus, a release measurement, authoritative agentic fixtures, and the two subprocess crash-harness cases. Inference tests use deterministic simulated endpoints; no live-model fidelity claim is made. The CVAFRS07 historical corpus remains independent and was not run.

The shared docs policy checker failed in the main checkout on an existing long Windows path under `target`. In a clean worktree containing this change it reported 28 pre-existing findings in seven other documents, versus 32 at the starting commit, with no findings in the changed Dream documents. Both whole-repository and changed-from checks reported the same 28. The installed Pitlord has no `docs` subcommand. The architecture runner passed the multiplexing guard self-tests (92 fixtures), P4.8 coverage (166 public APIs) and external ownership guard (11 fixtures); the full Lexicon/Arcana refresh was interrupted after extended Rust analysis without a refreshed snapshot, and remains unverified. A Pitlord check against the October 26 snapshot reported no violations but is stale and is not current acceptance evidence.

Within the final full-library run, the named Dream modules passed the following counts: classifier 6, verifier 5, candidates 7, duplicate 8, canonical 8, lifecycle 7, processor 9, publisher 9, Community naming 5, temporal 10, cooldown 17, and Phylactery Dream 4. These counts are module-name matches within the full run, not separate filtered invocations. The full run also exercised historical Dream pair/proof/processing reopen and import cases.

The direct Cargo tree contains only the five permitted lower Reliquary crates listed above. A source audit found one service classifier, verifier, candidate ranker, duplicate planner, lifecycle planner and naming evaluator; Root retains owner adapters. A search of the service source found no Cva, Phylactery, Container, GraphOwner, MemoryStore, FreshnessStore, DreamPairStore or DreamFreshnessJournal references.

## Baseline and contract inventory

At `494e1c2a`, the Dream crate was empty and Root had no direct dependency on it. The pre-change `dream_classifier_tests` filter passed 6 tests. The pre-extraction library test binary subsequently passed the `dream_` filter (168 tests) and the `chronos` filter (4 tests) with local access and a short worktree-local temporary path. The classifier and verifier schema files retain exactly their baseline Git blob IDs (`95c27509b44e8985e3fe235b66d718e2cdfdaba1` and `60b2811210a027d71e163f2bd418bba02d8903e8`, respectively). Their public pair values now use the same Memory Core `MemoryId` and are re-exported by Root.

`DreamProcessor` and `DreamCommunityNamer` remain Root-owned because their public methods accept Cva or Phylactery. The Root pair-history and pair-codec files remain unchanged. REL processing retains its Begin, publication-root, accepted-cut and Freshness settlement steps; PHY retains its distinct path without those REL settlement operations. Neither path replays inference from durable pair receipts.

The baseline CLI suite, focused cases outside the `dream_` filter and external retained corpus were not run before editing. Their prior state cannot be inferred from the final checks; those baseline witnesses remain unverified.

## Notes

The clean baseline CLI build was interrupted during dependency compilation and remains unverified. The refreshed architecture check remains unverified. The complete focused baseline outside the `dream_` filter remains unverified. The historical CVAFRS07 corpus is an independent gate and has not been run by this extraction.

## Related docs

- [Dream subsystem](subsystems/dream.md)
- [Dream service extraction specification](dream-service-crate-extraction-spec-2026-10-28.md)
