# GM independent post-correction reacceptance — 2026-11-06

**Verdict: ACCEPTED** for the native REL/PHY 2.1 maintenance, history and
supported same-owner REL reconciliation boundary. This review supersedes the
rejection in `gm-independent-implementation-audit-2026-11-06.md` while
retaining that document as the defect and red-test record.

Baseline: `rebuild/clean-architecture` through M03 correction commit
`beb88ae0`; reference oracle `Reliquary-reference` at `f62b1fb4`; governing
contract `maintenance-history-parity-phase-2026-11-05.md`.

## Independent defect closure

| Gate | Reopened defect | Corrected proof |
| --- | --- | --- |
| M01 | Selected Memory could lose its evidence-linked Relationship and Entity closure; publication had a no-replace race. | Relationship evidence expands the owner-local Memory/Entity closure; complete retained Entity/Relationship histories and tombstones replay and compare after independent reopen. Publication uses same-volume atomic no-replace linking. The former red GM regression passes. |
| M02 | Unactivated Archive mappings/backing were retained; `RELXIDX2` reachability and valid unreceipted generations were not handled. | Reclamation now distinguishes accepted mappings, current external-index heads/barriers and all referenced packed matrices; superseded/unactivated exclusive backing and valid wholly unreceipted generations are removed. Malformed or ambiguous evidence fails closed. Both former red reclamation regressions and the ten-case focused suite pass. |
| M03 | Completed divergent import epochs, initial Dream state, retained Insomnia terminal/retry journals and rebased Freshness authority were incomplete. | Destination-native import replay preserves Archive/Memory/Freshness receipts, authenticated Dream population and the supported exact work-journal chain. Replayed settlements bind original source Freshness authority separately from the destination's actual merged snapshot; repeat reconciliation is byte-identical with zero Memory replay. Conflicting journals/authority and unsupported transitions publish nothing. |
| M04 | Whole-owner proof collapsed converged histories with different intermediate revisions and used unstable Debug encoding. | Versioned canonical semantic encoding binds complete retained Entity, Relationship, Graph, Ego and Archive Branch revision streams. Adversarial converged-history fixtures now produce distinct stable proofs and pass after reopen. |

Two independent post-M03 reviews were performed. The first semantic review
rejected a regression that accepted an explicit conflict on repeated rebased
Freshness replay. M03 was corrected so authenticated source authority is
durable and repeat succeeds byte-stably; the second review accepted the result
and confirmed that the Insomnia claim is narrowed to the tested terminal/retry
transition subset. A separate cross-gate review found the four original
implementation defects corrected and identified only stale acceptance/tracker
authority, which this record and the companion status updates resolve.

## Mandatory verification

Executed from `Reliquary-rebuild` with repository-local
`target-codex-check` and serialized locked/offline Cargo:

```text
python scripts/check_architecture.py
Production DAG: 23 packages, 76 edges; expected 76.

cargo fmt --all --check
passed

cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check
passed

cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
passed, including all crate, integration and documentation suites
```

Focused independent checks also passed: four `gm_audit_` regressions, 15
Application rewrite cases, ten storage-reclamation cases, nine historical-proof
cases and 16 semantic-reconciliation cases. The owner-session portion of the
final unrestricted run passed 154 unit and six integration tests.

Existing unused-code, incremental-cache, local hard-link fallback and Windows
`LNK4098` warnings remain outside GM. No package, manifest or approved
production dependency edge changed.

## Combined gate decision

The reference-to-rebuild behavior matrix and REL/PHY end-to-end scenarios in
`gm-maintenance-history-parity-gate-2026-11-06.md` remain the combined gate
record, now amended by the corrections above. Source immutability, independent
reopen, no-replace publication, owner/privacy boundaries, stable retained
identities, closure validation, non-reinforcing maintenance and no silent
reconciliation are covered by deterministic positive and negative fixtures.

Typed limits remain explicit: old-format runtime reading and corpus conversion;
arbitrary PHY/cross-owner reconciliation; open imports; unsupported Dream or
Insomnia transitions; divergent Community, Ego, external-source, maintenance
or pruned-payload state; and unknown physical carriers. These limits are not
presented as parity.

M01, M02, M03 and M04 are reaccepted. GM passes; P01 may use GM as a satisfied
dependency. D01–D03 conversion/cutover and G4 product-corpus acceptance remain
separate gates.
