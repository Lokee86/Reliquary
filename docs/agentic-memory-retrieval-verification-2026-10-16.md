# Agentic Memory retrieval verification — 2026-10-16

## Purpose

Record the implementation, real host receiver evidence, frozen-corpus disposition, hard-cut audit, and final integration gates for the 45-step [linear plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md).

## Overview

Reliquary implements the REL-only three-tool controller, complete lexical/exact-semantic candidate pool, passive Freshness ordering, full declined-set refinement, concurrent one-hop traversal, strict model contracts, canonical delivery manifests, and confirmed owner-grouped accepted-use receipts. The actual requesting-context receiver and exact-manifest restart recovery are implemented in the isolated Warlock candidate `cf41c6690e089cab4a00950d6048ad6828eaeab7`.

Implementation and evidence closeout are complete through Step 45, with the frozen-corpus format qualification retained. Frozen-corpus limitations and final gate results below must be read separately from implementation completion. The candidate has not been deployed or merged into the original main worktrees.

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

Results: the current-format protocol smoke passes (1/1), including all functional assertions and twelve fidelity probes. Inventory, functional and fidelity phases each reached all three frozen corpora and returned `blocked_by_corpus_format`, with all twelve authoritative source hashes unchanged. Each REL contains one CVAFRS03 record plus 71/57/92 CVAFRS02 records respectively. This branch requires CVAFRS02; current-owner container migration returns AlreadyCurrent and is not a prototype converter. Consequently zero frozen-corpus retrieval calls/scenarios ran and recall, candidate counts and latency remain null. Useful frozen-corpus answer/evidence recall is unmeasured, not preserved by assertion. Compatible authoritative fixtures or separately scoped format migration/rebase are required before production qualification.

Reproduce with `python scripts/agentic_memory_fixture_acceptance.py --binary bin/agentic-core-final-cache/debug/deps/reliquary_memory-3e7a1095be73f667.exe --phase inventory` (then `functional` and `fidelity`). Retained results: [inventory](../benchmarks/agentic-memory-fixture-inventory-2026-10-16.json), [functional](../benchmarks/agentic-memory-fixture-functional-2026-10-16.json), [fidelity](../benchmarks/agentic-memory-fixture-fidelity-2026-10-16.json). Unsupported prototype format must be reported as a corpus limitation, not bypassed or counted as functional/fidelity success. No new prototype converter, synthetic replacement corpus, or runtime compatibility mode is authorized by this acceptance harness.

## Hard-cut and ownership audit

Primary review and a bounded Luna source audit found one canonical controller/candidate/decision/completion path. Controller state contains exactly CurrentQuery, SearchedIDs and SelectedIDs. Operation-local staging is discarded on error. Query pools are complete before the Freshness cut; refinement uses only the declined set; graph decisions gather before state commit and union independent approvals while retaining anchor reports. The shared vector decoder/cosine and existing Freshness writer remain authoritative. Existing passive search remains separate.

The final architecture gate initially identified five unclassified new public Gateway APIs. The coverage guard now classifies the three preparation APIs and conversation-scoped Echo read, and separately enforces exact-confirmed coordinated completion. It recognizes `record_accepted_memory_use` as a write and rejects it in read/preparation classifications. All 92 positive/negative guard fixtures pass; all 166 public APIs are classified.

Two audit observations were resolved without changing the frozen protocol:

- A missing active compatibility profile fails Gateway query atomically, as already frozen by the Step 18 tests. Lower-domain lexical-only populations remain supported; this does not imply the Gateway may invent an embedding compatibility profile.
- Anchor count and therefore concurrent fan-out are not globally capped by this design. Each anchor retains its ten-candidate cut, and there is no new turn budget. Large anchor lists remain an operational sizing concern for the embedding host.

The real production consumer lives in the separate Warlock candidate; the absence of a host caller inside the storage/runtime crate is expected call direction.

## Final repository gates

Locked CLI formatting/check/test passed: all 21 tests. Core formatting and cargo check passed. Full cargo test passed: 1,227 library tests, three ignored; 141 integration tests across 23 targets; zero doc tests and zero failures. Both documentation policy modes report only the four pre-existing missing sections in the Ego inventory, with no retrieval findings.

The final core gate uses `CARGO_TARGET_DIR=C:\!bin\workspace\Reliquary-agentic-retrieval\bin\agentic-core-final-cache` and `TEMP`/`TMP=C:\!bin\workspace\agentic-test-tmp`, then `cargo fmt --all -- --check`, `cargo check --quiet -j2` and `cargo test --quiet -j2` against this worktree's manifest. This fresh physical cache avoids the earlier missing generated dependency archives; no source workaround or machine cleanup-policy change was introduced.

The first whole-suite run finished with 1,224 passed, three failed and three ignored. The existing external-source plan installer uses raw MoveFileExW paths; the longer worktree-local TEMP made its generated destination 266 characters and caused Storage/Conflict. The tests, helper and installer are unchanged from baseline 596f5ce. All seven external tests pass with the shorter workspace TEMP and identical source/binary; the full suite then passed under that environment (job job_8LNsKwIMm5PHMvS6sHttTdRB, exit 0). This is an existing Windows path-length constraint, not an agentic runtime change.

`python scripts/check_architecture.py --refresh` passed: 17 areas, 23 rules, no violations; 92 coordination guard fixtures and 11 external-boundary guard fixtures passed. Refreshed snapshot: `sha256:268ff9b01f3749e5cc028848dee1046b0ae984b0930f502ed8613b7f3e20aae1`. Arcana records 77,755 nodes, 197,085 edges, 16,352 resolved calls and 17,711 possible calls. No Rust source changed after this refresh. Dynamic field calls to `runtime.cva.collect_memory_candidate_pool` and `runtime.cva.record_accepted_memory_use` remain unresolved in the generated call graph (builtin-target/dynamic-target); direct source, enforced guard classifications and runtime tests establish these edges. The graph is current but does not independently prove those calls. No unexpected ownership violation was found.

Full fixture fidelity and deployment are separate release qualifications even when code/guard gates pass.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, design/specification, host seam and shared documentation rules.
- Updated: current architecture/API/status/Freshness records, index, behavioral/coverage maps, linear ledger, execution record and this verification report.
- Not affected: unrelated Ego edits and storage-format semantics.
- Compliance check: both structural and change-impact modes isolate the same four pre-existing Ego inventory missing sections; retrieval documentation is clear.
- Known documentation gaps: Ego inventory lacks Purpose, Overview, Related docs and Notes.

## Architecture impact

- Standards added or changed: no shared standard or Pitlord policy change.
- Ownership or boundary impact: one host-owned turn controller; passive REL preparation; exact receiver-confirmed coordinated existing accepted-use writer; no second journal or Freshness implementation.
- Pitlord enforcement impact: refreshed enforcement passes; source coordination coverage now admits the new APIs explicitly and rejects preparation writes.
- Other verification impact: frozen fixture wrapper binds source/binary hashes and reports format blocks; focused real-host receiver/restart and full CLI gates pass.
- Known architectural gaps: dynamic graph call-resolution limits, uncapped anchor fan-out sizing, unmeasured frozen-corpus fidelity and deployment qualification remain explicit.

## Related docs

- [Linear implementation plan](agentic-memory-retrieval-linear-implementation-plan-2026-10-16.md)
- [Architecture specification](agentic-memory-retrieval-implementation-plan-2026-10-16.md)
- [Design](agentic-memory-retrieval-design-2026-10-15.md)
- [Execution record](agentic-memory-retrieval-baseline-2026-10-16.md)
- [Host handoff seam](agentic-memory-host-handoff-seam-2026-10-16.md)
- [External consumption contract](freshness-external-consumption-contract.md)

## Notes

No original main-worktree Ego edits, Warlock graph/UI work, or authoritative source fixtures are part of this change. Generated build and semantic-analysis caches are disposable. Documentation findings and corpus limitations must remain explicit in the final disposition.
