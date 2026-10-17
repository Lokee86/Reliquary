# Agentic Memory retrieval verification — 2026-10-16

## Purpose

Record the implementation, real host receiver evidence, frozen-corpus disposition, hard-cut audit, and final integration gates for the 45-step [linear plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md).

## Overview

Reliquary implements the REL-only three-tool controller, complete lexical/exact-semantic candidate pool, passive Freshness ordering, full declined-set refinement, concurrent one-hop traversal, strict model contracts, canonical delivery manifests, and confirmed owner-grouped accepted-use receipts. The actual requesting-context receiver and exact-manifest restart recovery are implemented in the isolated Warlock candidate `cf41c6690e089cab4a00950d6048ad6828eaeab7`.

Reliquary is consolidated on `main`. Steps 38-40 have been rerun on main: functional scenarios pass on all three frozen corpora, while the measured evidence-recall comparison is poor. Evidence closure does not imply useful recall preservation or deployed host qualification. Historical Step 45 feature-candidate gates below are labelled separately from the current main rerun.

## Implementation and host evidence

- Reliquary implementation through Step 36: `638992aff30508e52e597b4955ed6a18a286fcdc`; detailed phase gates are in the [execution record](agentic-memory-retrieval-baseline-2026-10-16.md).
- Warlock receiver candidate: `cf41c6690e089cab4a00950d6048ad6828eaeab7`, following three-tool integration `9633ab8`.
- The retained Warlock test executable was rerun during closeout: all 8 `chat_memory` and all 5 `chat_task_context_tests` passed. Its SHA-256 was `abd362a7e50903eae0ed495eadb001df4d2003f558ff60dbe6c9eb8a7dd4c215`.
- This rerun exercises the already-built integration candidate; it is not a fresh whole-host build or a production pin change. Host documentation check passed.
- Actual transfer, cancellation/mismatch before transfer, uncertain acknowledgment after transfer, exact retry, and full host/gateway restart recovery are covered. Generic Echo ToolResult, provider success, transcript admission and ConversationExecution are not consumption proof.
- The original manifest and digest survive through the existing private `ToolContextAccepted` Echo kind. Completion keeps SelectedIDs after actual delivery even if an existing accepted-use receipt fails. Retry retains the same owner-specific identity and captured cut.

The [host seam record](agentic-memory-host-handoff-seam-2026-10-16.md) owns the exact source paths and receiver protocol.

## Frozen fixture protocol

The harness `scripts/agentic_memory_fixture_acceptance.py` hashes every authoritative REL/PHY plus `AUTHORITATIVE.txt` and `SHA256SUMS.txt`, verifies frozen published REL/PHY hashes, copies each REL into the active worktree, and invokes the ignored native acceptance runner only on those copies. Source hashes are checked again in a finally block. Evidence includes the test-binary and harness-source hashes.

The three phases are `inventory`, `functional`, and `fidelity`. The native scenario runner exercises the production Gateway APIs with deterministic stored-vector replay and decision endpoints, never new general inference. Functional scenarios cover Freshness versus raw top ten, repeat one-hop traversal, complete refinement without search, concurrent overlap, already-provided fallback, failed/cancelled handoff, and success plus exact retry.

The evaluation-only fidelity harness uses at most twelve deterministic rare-term probes from retained Memory data, a full-corpus evidence oracle and an idealized retained document vector. It records selected/missed refs, candidate presentations, call counts, local elapsed time and evidence recall. It does not grade generated answers or establish live-model/service quality.

Main rerun source: `16812786757bf75ab05ee831e8bf5277cc4c6c7b`. All three phases execute on all three native corpora, with no corpus-data exceptions. All twelve authoritative REL/PHY/manifest hashes remain unchanged before and after every phase. The previous `blocked_by_corpus_format` artifacts came from an older feature reader and are superseded; `CVAFRS03` is the native historical checkpoint supported on consolidated main. The additional legacy-prefix mount gap was corrected before these runs, without rewriting frozen inputs.

Step 38 passes all mandatory scenarios on every corpus: Freshness differs from raw semantic top-ten presentation; active one-hop anchors have 52 / 70 / 51 neighbors and both successive calls return ten disjoint neighbors; refinement reconsiders 30 declined candidates with zero additional query embeddings; concurrent anchors retain two independent reports and one shared neighbor, with 12 / 19 / 16 unique returned bodies; already-provided, failed/cancelled handoff, successful completion and byte-stable exact retry all pass.

Step 39 completed 36 deterministic probes, but useful evidence recall is not preserved in this comparison:

| Corpus | Active Memories / current vectors | Probe evidence hits / broad oracle | Micro recall | Decision calls / presentations | Median local elapsed |
| --- | ---: | ---: | ---: | ---: | ---: |
| first14d | 1016 / 1016 | 0 / 56 | 0.00% | 36 / 240 | 322.7 ms |
| first28d | 2191 / 2191 | 1 / 48 | 2.08% | 37 / 249 | 944.1 ms |
| ellis | 703 / 703 | 0 / 24 | 0.00% | 36 / 240 | 253.3 ms |

The aggregate is **1/128 evidence occurrences (0.78%)**, with one nonempty probe out of 36. Every corpus uses twelve query retrievals, twelve refinements and twelve query embeddings. Only the 28-day corpus obtains approved anchors, producing one graph call; the other corpora produce none. Local elapsed ranges are 300.0-429.0 ms, 741.3-5777.2 ms and 225.4-338.3 ms respectively. These concurrent-host diagnostic timings include local receipts and are not service SLO measurements. Exact selected references, misses and per-probe counts/timings are retained in the JSON.

The observed failure is candidate exposure. The semantic lane includes every compatible vector-bearing Memory; query preparation retains candidate IDs and sorts by Freshness then owner-qualified ID before presenting ten bodies. Lexical/cosine scores do not order that presentation. Because every active Memory has a current vector in these corpora, the initial ten are query-independent for a fixed Freshness snapshot and SelectedIDs set. Refinement only revisits the already-declined presentation, and no approved seed means the requester cannot start the bounded graph expansion in this harness. Even replaying the target document vector therefore does not establish access to the expected evidence. This is consistent with the implemented frozen policy, not a reason to alter it silently during acceptance. Relevance eligibility/order and how the requester discovers further candidates require a separately reviewed design change and a new comparison.

The broad oracle has evidence recall 1.0 by construction. Rare literal-term probes and idealized document-vector replay stress evidence exposure; answer recall remains null, and no live model, representative user-task accuracy or generated-answer quality is measured. Step 39's dated comparison and Step 40's reproducible evidence closure are complete; this is a negative fidelity result and does not qualify production recall or deployment.

Reproduce from the main root with `python scripts/agentic_memory_fixture_acceptance.py --binary bin/agentic-main-fixture-1681278.exe --phase inventory`, then the same command with `functional` and `fidelity`. Build the library test executable using `cargo test --lib --no-run -j2` with `CARGO_TARGET_DIR=bin/agentic-main-cache`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0` and `TEMP`/`TMP=bin/agentic-test-tmp` (all resolved inside this repository), then copy the resulting executable to the retained path before parallel build checks. Binary SHA-256: `11659550016dc92cfa608fc6f61dca3ae055db71803502b2e99e1566ae1af72b`. The reports bind the full code commit, binary and harness hashes, exact invocation, input hashes and disposable-copy paths. Retained results: [inventory](../benchmarks/agentic-memory-fixture-inventory-2026-10-16.json), [functional](../benchmarks/agentic-memory-fixture-functional-2026-10-16.json), [fidelity](../benchmarks/agentic-memory-fixture-fidelity-2026-10-16.json).

## Hard-cut and ownership audit

Primary review and a bounded Luna source audit found one canonical controller/candidate/decision/completion path. Controller state contains exactly CurrentQuery, SearchedIDs and SelectedIDs. Operation-local staging is discarded on error. Query pools are complete before the Freshness cut; refinement uses only the declined set; graph decisions gather before state commit and union independent approvals while retaining anchor reports. The shared vector decoder/cosine and existing Freshness writer remain authoritative. Existing passive search remains separate.

The final architecture gate initially identified five unclassified new public Gateway APIs. The coverage guard now classifies the three preparation APIs and conversation-scoped Echo read, and separately enforces exact-confirmed coordinated completion. It recognizes `record_accepted_memory_use` as a write and rejects it in read/preparation classifications. All 92 positive/negative guard fixtures pass; all 166 public APIs are classified.

Two audit observations were resolved without changing the frozen protocol:

- A missing active compatibility profile fails Gateway query atomically, as already frozen by the Step 18 tests. Lower-domain lexical-only populations remain supported; this does not imply the Gateway may invent an embedding compatibility profile.
- Anchor count and therefore concurrent fan-out are not globally capped by this design. Each anchor retains its ten-candidate cut, and there is no new turn budget. Large anchor lists remain an operational sizing concern for the embedding host.

The real production consumer lives in the separate Warlock candidate; the absence of a host caller inside the storage/runtime crate is expected call direction.

## Main rerun repository checks

Core formatting and `cargo check --quiet -j2` pass. `cargo test --quiet --no-fail-fast -j2` executes all targets: 1,346 library tests pass, 16 fail and five are ignored; all 141 integration tests across 23 targets pass; zero doc tests fail. The process exits 101 for the library target. Fourteen of the sixteen library failures pass in focused runs outside the sandbox (local HTTP mocks, hard links and native Lore repository operations). These reruns do not constitute a fresh all-tests nonsandbox pass. The independent retained fixture executable also completes the full library suite with 1,347 passed, 15 failed and five ignored; its thirteen environment-sensitive failures pass in focused nonsandbox reruns, leaving the same two unresolved failures.

Both remaining failures reproduce outside the sandbox and use unchanged main Cva paths, without entering the modified Gateway identity conversion:

- `historical_generation_refresh_changes_proof_but_never_fragment_event_identity`: `import scoring is not an isolated provisional Dream history` at `historical_manifest_tests.rs:93`.
- `legacy_typed_rel_migration_preserves_identity_and_state`: `semantic Freshness requires REL UUID` at `migration_tests.rs:117`.

No unrelated historical/F8 or UUID-less migration change is included. The focused agentic suite passes (80 passed, one ignored); the native legacy typed-REL mount/journal/subscription/reconnect regression and both prefix tests pass. The final Cargo library executable is retained as `bin/agentic-main-gates-c2b2d4c.exe`, SHA-256 `e1ca34b231311f8754b54724e075ca1143e6ab386e7a085482b6b39a42e79a1e`; the fixture reports retain their separately identified executable and source commit. Build/test output and TEMP/TMP remain inside main under `bin/`. A preceding Cargo attempt could not execute its missing cached test executable; restoring the verified retained executable and allowing Cargo to rebuild produced the complete run above. No machine cleanup policy changed.

CLI checks were not repeated: this correction adds a private Gateway identity seam, changes no public signature or storage format, and the detachable CLI has no dependency on these Gateway mount/journal/stream paths. Both documentation-policy modes report only five existing Ego findings: four missing inventory sections and the index link to the absent Ego Memory-Web Summary implementation plan. The two pre-existing Ego diffs are unchanged. Useful retrieval recall and production deployment remain unqualified.

`python scripts/check_architecture.py --refresh` passes on the settled tree: 17 areas, 23 rules, zero violations; 92 coordination fixtures and 11 external-boundary fixtures pass, with 166 public APIs classified. Snapshot `sha256:1fd6d2d2150632772ed0864b31449be63eda742beb5dad634cf47cebe16c755b` records 73,924 source nodes and 2,389 policy relationships. Every one of its 980 Rust-analysis input content hashes matches the final files, including both final fixture/admission test edits. All authoritative source and harness hashes were checked again after the refresh. No Rust source changed afterward. No shared standard, policy rule, storage format or semantic owner changed.

## Historical Step 45 feature-candidate repository gates

Locked CLI formatting/check/test passed: all 21 tests. Core formatting and cargo check passed. Full cargo test passed: 1,227 library tests, three ignored; 141 integration tests across 23 targets; zero doc tests and zero failures. Both documentation policy modes report only the four pre-existing missing sections in the Ego inventory, with no retrieval findings.

The final core gate uses `CARGO_TARGET_DIR=C:\!bin\workspace\Reliquary-agentic-retrieval\bin\agentic-core-final-cache` and `TEMP`/`TMP=C:\!bin\workspace\agentic-test-tmp`, then `cargo fmt --all -- --check`, `cargo check --quiet -j2` and `cargo test --quiet -j2` against this worktree's manifest. This fresh physical cache avoids the earlier missing generated dependency archives; no source workaround or machine cleanup-policy change was introduced.

The first whole-suite run finished with 1,224 passed, three failed and three ignored. The existing external-source plan installer uses raw MoveFileExW paths; the longer worktree-local TEMP made its generated destination 266 characters and caused Storage/Conflict. The tests, helper and installer are unchanged from baseline 596f5ce. All seven external tests pass with the shorter workspace TEMP and identical source/binary; the full suite then passed under that environment (job job_8LNsKwIMm5PHMvS6sHttTdRB, exit 0). This is an existing Windows path-length constraint, not an agentic runtime change.

`python scripts/check_architecture.py --refresh` passed: 17 areas, 23 rules, no violations; 92 coordination guard fixtures and 11 external-boundary guard fixtures passed. Refreshed snapshot: `sha256:268ff9b01f3749e5cc028848dee1046b0ae984b0930f502ed8613b7f3e20aae1`. Arcana records 77,755 nodes, 197,085 edges, 16,352 resolved calls and 17,711 possible calls. No Rust source changed after this refresh. Dynamic field calls to `runtime.cva.collect_memory_candidate_pool` and `runtime.cva.record_accepted_memory_use` remain unresolved in the generated call graph (builtin-target/dynamic-target); direct source, enforced guard classifications and runtime tests establish these edges. The graph is current but does not independently prove those calls. No unexpected ownership violation was found.

The main rerun above supersedes the historical format blocker. Useful recall and deployment remain release qualifications even when code/guard gates pass.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, design/specification, host seam and shared documentation rules.
- Updated: current architecture/API/status/Freshness records, index, behavioral/coverage maps, linear ledger, execution record and this verification report.
- Not affected: unrelated Ego edits and storage-format semantics.
- Compliance check: the current main rerun of both structural and change-impact modes reports only five unrelated pre-existing Ego findings: four inventory sections and the index link to the absent Ego Memory-Web Summary implementation plan. No retrieval findings remain.
- Known documentation gaps: Ego inventory lacks Purpose, Overview, Related docs and Notes; `docs/INDEX.md` links to the absent `ego-memory-web-summary-implementation-plan.md`.

## Architecture impact

- Standards added or changed: no shared standard or Pitlord policy change.
- Ownership or boundary impact: one host-owned turn controller; passive REL preparation; exact receiver-confirmed coordinated existing accepted-use writer; no second journal or Freshness implementation.
- Pitlord enforcement impact: refreshed enforcement passes; source coordination coverage now admits the new APIs explicitly and rejects preparation writes.
- Other verification impact: main fixture/scenario/hash checks and current architecture enforcement pass; the main full-test findings above remain explicit. Prior host/CLI evidence is historical.
- Known architectural gaps: dynamic graph call-resolution limits, uncapped anchor fan-out sizing, poor measured frozen-corpus evidence recall and deployment qualification remain explicit.

## Related docs

- [Linear implementation plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)
- [Architecture specification](agentic-memory-retrieval-implementation-plan-2026-10-16.md)
- [Design](agentic-memory-retrieval-design-2026-10-15.md)
- [Execution record](agentic-memory-retrieval-baseline-2026-10-16.md)
- [Host handoff seam](agentic-memory-host-handoff-seam-2026-10-16.md)
- [External consumption contract](freshness-external-consumption-contract.md)

## Notes

No original main-worktree Ego edits, Warlock graph/UI work, or authoritative source fixtures are part of this change. Generated build and semantic-analysis caches are disposable. Documentation findings and corpus limitations must remain explicit in the final disposition.
