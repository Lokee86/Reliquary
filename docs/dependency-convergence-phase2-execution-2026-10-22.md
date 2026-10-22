# Phase 2 execution and acceptance ledger

Parent index: [Documentation index](INDEX.md)

## Purpose

Track execution of the [91-task Phase 2 specification](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md) against the accepted owner DAG. This record does not replace current subsystem contracts.

## Overview

Phase 2 is accepted locally after all twelve gates passed. The baseline source is `529f2a8`, with the same authored Rust as accepted Phase 1 `8614547`. The pre-existing `.gitignore` edit remains outside this campaign. No corpus or persistent format migration is authorized or performed.

## Blocking gates

| Gate | Status | Evidence |
| --- | --- | --- |
| B0 | Baseline frozen | [Source inventory](dependency-convergence-phase2-baseline-2026-10-22.md), [before-edge ledger](../benchmarks/dependency-convergence-phase2-baseline-2026-10-22.json), [baseline checks](dependency-convergence-phase2-baseline-verification-2026-10-22.md). Fresh hashes/sizes match all six accepted sources. The ordinary sandbox baseline suite exposed environment-sensitive failures. The elevated frozen baseline completed with 1,957 passed and five ignored library tests. |
| B1 | Contract frozen | [Concrete owner contracts](dependency-convergence-phase2-contracts-2026-10-22.md). Existing callers retain cross-owner admission; stores retain local schema/history/publication. Grouped Memory body spans preserve binary and JSON representations without payload copies or format changes. |
| AR2, M2, E2, G2, F2, V2, O2 | Passed | Concrete owner cuts, focused owner-interface tests, full regression suite, source guard and six-source oracle. See the verification record below. |
| L2, D2, A2 | Passed | REL/PHY replay and privacy regressions, negative source fixtures, reviewed convergence delta, full tests, corpus oracle and documentation gates. |

B0 freezes prior accepted behavioral evidence and the current before-image. The frozen elevated baseline library run completed; changed-tree acceptance is recorded below. B1 froze the owner interfaces; all subsequent blocking gates have now passed. Freshness's direct Archive decoder removal was already achieved in Phase 1 and is verified rather than rewritten.

## Task register

| Task | Current disposition |
| --- | --- |
| P2.0.01 | Frozen baseline |
| P2.0.02 | Frozen baseline |
| P2.0.03 | Frozen baseline |
| P2.0.04 | Frozen baseline |
| P2.0.05 | Frozen baseline |
| P2.0.06 | Frozen baseline |
| P2.0.07 | Frozen baseline |
| P2.1.01 | Contract frozen |
| P2.1.02 | Contract frozen |
| P2.1.03 | Contract frozen |
| P2.1.04 | Contract frozen |
| P2.1.05 | Contract frozen |
| P2.1.06 | Contract frozen |
| P2.1.07 | Contract frozen |
| P2.2.01 | Accepted |
| P2.2.02 | Accepted |
| P2.2.03 | Accepted |
| P2.2.04 | Accepted |
| P2.2.05 | Accepted |
| P2.2.06 | Accepted |
| P2.2.07 | Accepted |
| P2.2.08 | Accepted |
| P2.3.01 | Accepted |
| P2.3.02 | Accepted |
| P2.3.03 | Accepted |
| P2.3.04 | Accepted |
| P2.3.05 | Accepted |
| P2.3.06 | Accepted |
| P2.3.07 | Accepted |
| P2.4.01 | Accepted |
| P2.4.02 | Accepted |
| P2.4.03 | Accepted |
| P2.4.04 | Accepted |
| P2.4.05 | Accepted |
| P2.4.06 | Accepted |
| P2.4.07 | Accepted |
| P2.4.08 | Accepted |
| P2.4.09 | Accepted |
| P2.5.01 | Accepted |
| P2.5.02 | Accepted |
| P2.5.03 | Accepted |
| P2.5.04 | Accepted |
| P2.5.05 | Accepted |
| P2.5.06 | Accepted |
| P2.5.07 | Accepted |
| P2.6.01 | Accepted |
| P2.6.02 | Accepted |
| P2.6.03 | Accepted |
| P2.6.04 | Accepted |
| P2.6.05 | Accepted |
| P2.6.06 | Accepted |
| P2.6.07 | Accepted |
| P2.7.01 | Accepted |
| P2.7.02 | Accepted |
| P2.7.03 | Accepted |
| P2.7.04 | Accepted |
| P2.7.05 | Accepted |
| P2.7.06 | Accepted |
| P2.7.07 | Accepted |
| P2.8.01 | Accepted |
| P2.8.02 | Accepted |
| P2.8.03 | Accepted |
| P2.8.04 | Accepted |
| P2.8.05 | Accepted |
| P2.8.06 | Accepted |
| P2.8.07 | Accepted |
| P2.8.08 | Accepted |
| P2.8.09 | Accepted |
| P2.8.10 | Accepted |
| P2.9.01 | Accepted |
| P2.9.02 | Accepted |
| P2.9.03 | Accepted |
| P2.9.04 | Accepted |
| P2.9.05 | Accepted |
| P2.9.06 | Accepted |
| P2.9.07 | Accepted |
| P2.9.08 | Accepted |
| P2.10.01 | Accepted |
| P2.10.02 | Accepted |
| P2.10.03 | Accepted |
| P2.10.04 | Accepted |
| P2.10.05 | Accepted |
| P2.10.06 | Accepted |
| P2.10.07 | Accepted |
| P2.11.01 | Accepted |
| P2.11.02 | Accepted |
| P2.11.03 | Accepted |
| P2.11.04 | Accepted |
| P2.11.05 | Accepted |
| P2.11.06 | Accepted |
| P2.11.07 | Accepted |

## Verification discipline

Tests, caches, logs and temporary files stay inside the repository. The final gate includes local owner tests, full library/integration/CLI checks, the six-file opt-in oracle, physical and logical negative fixtures, documentation gates and a single stabilized source-graph refresh. The existing Phase 0 graph and debt before-image remain retained. Compilation alone is not a test pass.

## Related docs

- [Phase 2 specification](dependency-convergence-phase2-owner-local-boundaries-spec-2026-10-22.md)
- [Phase 1 accepted verification](dependency-convergence-phase1-verification-2026-10-22.md)
- [Target DAG](dependency-convergence-target-dag-2026-10-21.md)

## Notes

Phase 3 composition, CVAFRS06 settlement minimization, new semantic features, implicit repair and cross-owner privacy changes remain excluded. This ledger records verified gates, deleted paths and the Phase 3 contract register.

## Accepted verification

The final Rust tree passes formatting and locked compilation. The full
feature-enabled run passed **2,142 tests across 26 targets**, including **1,979
library tests**; seven tests were ignored by that command. The detachable CLI
passed formatting, locked compilation and **32 tests**. Ten focused compatibility
profile tests also passed. The separately invoked six-source canonical historical
oracle passed in **6,398.24 seconds**, checking exact current/history results and
unchanged canonical bytes and modification times. Ignored tests are not counted
as passes. The prepared graph smoke example also passed: 2,972 nodes, 75 branches, 498 fragments and 2,972 content objects round-tripped. The older graph export lacked branch paths; a local copy derived those paths from parent IDs without changing the source fixture.

The stable source refresh produced Arcana snapshot
sha256:f6ee9a86f878515af28ce0cb93d13a5454b2846e2b692697e5f6d4b3c19a8716.
The indexed authored Rust manifest matches the working tree. All 34 components
and 163 justified DAG edges remain unchanged. Physical guards, owner guards,
negative fixtures and unbaselined Pitlord rules pass. The convergence gate passes
with **4,630 classified findings and no new or stale debt**. Of the prior 4,625
findings, 4,507 fingerprints remain unchanged, 118 stale fingerprints have explicit
source dispositions, and 123 new or reidentified findings have individual review.
The total increase is not a claim of worse or better ownership: the finer owner
rule distinguishes immutable values and higher composition from durable storage.
The original Phase 0 graph and boundary before-image remain unchanged.

Documentation policy, changed-document policy, Pitlord documentation ownership
(1,252/1,252 mapped code files), and Archivist indexes/links pass. Luna audits
reviewed read/proof views, Graph/Perception admission and checker bypasses. The
identified Project-file map exposure and crate-root-alias enforcement gap were
fixed before final acceptance.

- [Source manifest, corpus identities and verification receipts](../benchmarks/dependency-convergence-phase2-verification-2026-10-22.json)
- [Individual convergence delta review](../benchmarks/dependency-convergence-phase2-debt-review-2026-10-22.json)
- [Concrete Phase 3 handoff](dependency-convergence-phase3-handoff-2026-10-22.md)

Local raw logs remain under .verification-tmp/phase2-evidence/; committed
verification receipts retain their SHA-256 digests. Tests required the local
Windows execution environment with filesystem/process access; initial restricted
sandbox failures are recorded in the baseline evidence. Final tests used
repository-local temporary files and disabled incremental compilation after
interrupted builds exposed stale linker objects. No source, format, semantic
policy or corpus change was made to accommodate an environment failure.

## Deletion and boundary register

Deleted facade/owner_maintenance.rs and facade/history_prune_manifest.rs; their
owner-local storage now lives in src/maintenance/, while source-history proof
composition lives in facade/owner_history_proof.rs. Removed foreign-store
arguments from Memory, Graph, Resolution/Relationship, vector generation,
DreamPair and Insomnia persistence. Removed Memory's Insomnia/External envelope
decoders, the import completion token and Dream replay callback. Their explicit
caller admission/transport replaces them, without dual store APIs. Pure profile
validation is separate from provider probes. Archive interaction storage uses
its own error contract; higher conversion preserves the public errors.

The owner manifest records exact production files, immutable producer contracts
and higher composition classifications. This bounded lexical enforcement is not
a whole-program compiler proof. Higher History/Maintenance/Import, Runtime and
facade convergence remains assigned to later phases; no Phase 3 semantics or
CVAFRS06 minimization was implemented.
