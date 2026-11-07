# D02 — independent converted-corpus parity verification

Parent index: [Architecture](INDEX.md). Parent plan: [post-G4 execution](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation-ready specification; D02 not started or accepted.** Tool implementation may proceed after PG-C01, independently of D01. Final corpus execution requires D01-T05 and F03-V04. D02 must be implemented/reviewed independently of conversion logic.

## Purpose

Detect semantic loss even when the converted files open successfully and retain identical current counts. Required failures include changed intermediate history, swapped private owners, missing grouped authority, rekeyed vector IDs, altered unavailable states, dangling physical references and false Freshness credit.

## Overview

Read candidates through the normal current-only 2.1 runtime in a fresh process. Export the [common evidence schema](post-g4-corpus-evidence-contract-2026-11-06.md), compare with the immutable independent F03 bundle, then exercise authorized product behavior across the three REL/PHY pairs. Treat D01 receipts/relocation as assertions to check, not the source of expected truth.

## Architecture and ownership

The verifier is offline assurance tooling outside the shipping package graph. Its target-reader binary depends exclusively on the current rebuilt owner/product packages; it never imports the old reader or converter. Its comparator accepts read-only witness bundles. Share only the frozen schema/canonical encoding test fixtures, not conversion traversal, field mappings or expected values derived from output. The historical source exporter remains a separate process with separately recorded code provenance.

Use `RelSession::open_path_current`/`PhySession::open_path_current`, typed historical reads/proofs and immutable read seams for complete state inspection. Use the public Root/Gateway/Instance for privacy, management, retrieval and operational consumer checks. Missing inspection fields require a narrow immutable owner observation or test-only verifier module, not a public mutable store. Do not infer whole-history parity from current `OwnerInfo` or from one native proof digest.

## Verification contract

The comparison report binds full F03 source-admission and D01 receipt hashes, schema version, target reader source/binary digest, six candidate file hashes, all cuts/family counts and every result. Exact semantic equality is required except for individually reviewed physical-encoding differences and source-proven unavailable states. A broad exclusion such as “Freshness changed” is invalid.

| Case | Positive requirement | Required adversarial counterpart |
| --- | --- | --- |
| D02-01 identity/custody | Exact owner UUID/kind/scope/generation and stable IDs for all six files | Swap two PHYs, one unverified source hash or one mixed-generation target; reject cohort |
| D02-02 Archive/source | Exact retained turns, parentage/branches/Episodes, Unicode bytes, attachments and source timestamps | Change one parent or one byte without changing record count; mismatch |
| D02-03 Memory/history | Complete retained revisions/bodies/lifecycle/provenance, not merely current content | Remove or alter an intermediate revision while preserving current head; mismatch |
| D02-04 Graph/Entity/Ego | Ordered revisions, canonical identities, relationships, communities and private Ego evidence | Different intermediate graph, wrong mention body/span, dropped Anchor; mismatch |
| D02-05 Freshness | Original FRS07 authority/events/causal basis/scores/cuts, no new credit | Duplicate use/birth, changed event order, fabricated intra-settlement state; mismatch |
| D02-06 work/import | Exact accepted source populations, claim/retry/held/terminal/completion receipts | Omit work, mark Held complete or change import scope; mismatch |
| D02-07 vectors | Six identity goldens and corpus IDs, exact packed/probe bits, row/profile/generation links | One float bit, row reorder, inactive profile falsely activated; mismatch |
| D02-08 physical references | Independently resolve every retained reference and verify bounds/identity | Forged relocation or offset points to another valid payload; reject |
| D02-09 operational consumers | Supported queue/stream/pending/finalized state and exact retry on copies | Blind redispatch of unknown host state, stale cursor or duplicate receipt; reject |
| D02-10 scope/privacy | Same-principal allowed access, unauthorized PHY and cross-owner references denied through Gateway | Revocation between prepare/commit and forged principal/owner grants; no leak/credit |
| D02-11 deterministic retrieval | Fixed queries/vectors/models, exact candidate IDs/order and authorized content under retained policy | Different canonical/active filtering, missing graph route, hidden rank drift; mismatch |
| D02-12 immutability/repeat | Fresh-process reread yields identical semantic bundle and no source/target mutation | Incomplete reader output, changed target during verification, sampled cuts labeled exhaustive; fail |

A physical full-file SHA normally differs old versus new; this is expected. Stable semantic identities and the common witness must match. Validate native historical proofs using their own proof version; do not compare different native hash namespaces as though identical. Sidecar absence, unsupported historical state and unavailable payloads must be explicitly proven, not mapped to null/zero to make equality pass.

Read-only checks run on verified candidates; mutating retry/recovery/consumer probes run on separately hash-bound disposable copies. Provider decisions and embedding query vectors are frozen; no live inference or silent retrieval tuning enters parity. Internal reads produce no accepted-use receipt. Reconcile actual host work only under the real host contract; unknown state stays Held.

## Independent expected data and diagnostics

F03 freezes expected rows before authentic conversion; D02 cannot regenerate expectations from candidate output. Negative tests are authored against a small explicit semantic oracle before running the converter. Inject mismatches through controlled fixture mutations, not by teaching the comparator to accept a special failure case. A converter/reader pair that shares the same omitted field must still fail these independent fixtures.

Every mismatch names owner, family, stable key, revision/cut, expected/observed availability and hashes. Print a bounded redacted diagnostic sample plus full discrepancy count. Missing/extra rows, families and cut ranges are failures. Tests cover integer precision, empty populations, duplicate keys, unknown versions and interrupted reports. Resume only at validated complete family/cut boundaries bound to both input bundles; mismatches cannot be hidden by resuming after the failure.

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance and evidence |
| --- | --- | --- | --- |
| D02-T01 — independent reader/comparator | PG-C01 | Current-only exporter and strict schema/diff implementation | Hand-authored equivalent old/new semantic fixtures pass; omitted/reordered/extra rows and unknown schemas fail; no converter dependency |
| D02-T02 — adversarial history/reference suite | D02-T01 | D02-01..08 and D02-12 synthetic negatives, complete cut/relocation coverage | Every injected fault is detected, including converged current heads; no empty-oracle success; read-only hash checks |
| D02-T03 — consumer and privacy parity | D02-T02 | D02-09..11 workload with exact identity/retrieval/operation expectations | Authorized/denied REL/PHY probes, retry/revocation/held state, deterministic rank comparison, zero read reinforcement |
| D02-T04 — verify six converted owners | D02-T03, D01-T05, F03-V04 | Exhaustive authentic cohort comparison and independent review | All 12 rows PASS for applicable owners, 6/6 independent current opens, no unreviewed differences, complete source/candidate custody; committed PASS report for D03 |

## Executable command contract

Create a small standalone interface during T01; these commands are specified targets, not existing product commands:

```text
parity-reader export --candidate-root <dir> --generation <candidate-manifest> --schema <v1> --output <target-witness>
parity-check compare --source <frozen-source-witness> --target <target-witness> --receipt <conversion-receipt> --output <report>
parity-check exercise --cohort <verified-copies> --cases <frozen-consumer-cases> --output <report>
```

All return structured results and nonzero for any mismatch or incomplete coverage. Exact output hashes and source revisions are required. Run locked/offline Rust tests/checks in the target workspace, schema/golden tests for the comparator, and the accepted architecture guard. If product observation APIs change, run the full workspace/CLI/G4 regression gates rather than validating only the verifier.

## Completion gate

D02 accepts only when exhaustive authentic semantic/history parity, physical-reference integrity and the privacy/consumer matrix pass independently with no source mutation. Review must inspect actual values/differences and read coverage, not just green counts. D01 is not allowed to edit the frozen source witness to repair a mismatch; fix conversion or an explicitly proven target conformance defect, then regenerate candidates and rerun D02 against the same expected truth.

## Out of scope

Repairing source data, choosing expected values after seeing output, re-inference, performance acceptance, multi-file promotion or a compatibility reader inside the production runtime.

## Related docs

- [Shared evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md)
- [F03 verification](f03-historical-verification-implementation-spec-2026-11-06.md)
- [D01 converter](d01-offline-converter-implementation-spec-2026-11-06.md)
- [D03 activation preparation](d03-safe-activation-implementation-spec-2026-11-06.md)

## Notes

Final D02 execution is serial after authentic conversion, but reader/comparator/adversarial implementation is parallel-ready. A common serialization utility is acceptable; a common source traversal or converter-supplied expected state defeats independence.
