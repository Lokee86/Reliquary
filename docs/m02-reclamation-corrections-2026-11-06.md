# M02 reclamation corrections — verified implementation slice (2026-11-06)

**Status:** M02 reclamation defects from the independent audit are repaired and tested in the native 2.1 rebuild. This is an implementation verification record, **not** GM acceptance or a claim that all M02 domains have been independently recertified.

Baseline: `2592b7c7` plus M02 working changes; reference behavior oracle `Reliquary-reference` `f62b1fb4`. Requirements: canonical M-phase plan §4 and independent audit `docs/m02-independent-reverification-2026-11-06.md`.

## Corrections

1. **Unactivated Archive mapping:** reclaim `RELAVEC1` records not referenced by any durable generation, and their exclusively owned packed vectors; retain every mapping belonging to an accepted or historical generation, even a retired profile. An orphan mapping that shares packed backing with a retained mapping does not remove the backing.
2. **Native external-index snapshots:** interpret `RELXIDX2` using the existing owner-index decoder. Retain the latest namespace heads and last discard/damage barrier; discard superseded snapshots. Retain all packed matrices named by surviving indexed chunks, including across multiple namespaces, independent of Memory and Archive mapping references. Current index source maps and sticky damaged state are compared against an independently reopened copy; any mismatch rejects the candidate.
3. **Unreceipted physical generations:** validate `RELVGEN1` record structure and deterministic generation ID, match every receipt to an exact earlier physical target, and classify only wholly unreceipted valid generation frames as removable. A temporary, private, filtered owner copy is reopened through full native owner admission before computing other reachability. Final candidate uses generation-receipt relocation and independent reopen. Malformed, missing, ambiguous or incorrect physical generation evidence fails closed. Source and destination are not mutated/published on failure.
4. **Safety:** unknown frame families still fail closed; verified source identity/cut/digest, staged sync, no-replace publication and existing interruption/collision guards remain intact. No legacy 2.0 reader or unapproved domain dependency was introduced.

## Executed checks

- `cargo test -p reliquary-owner-session storage_reclamation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **10 passed**. Includes live/shared Archive mappings, orphan matrices, two-namespace external snapshots and packed backing, discard barrier, malformed snapshot/damage marker, valid unreceipted generation, PHY no-op, and failure injection.
- `cargo test -p reliquary-owner-session gm_audit_reclamation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **2 passed**, including the previously failing `RELXIDX2` and unactivated Archive mapping independent regressions.
- `cargo test -p reliquary-application storage_reclamation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **2 passed**, covering accepted generation receipt relocation and malformed unreceipted generation fail-closed behavior.
- `cargo fmt --all --check`: **passed**.
- `python scripts/check_architecture.py`: **passed** (23 packages, 76 permitted production edges).
- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1 --skip gm_audit_historical_proof_distinguishes_retained_intermediate_entity_revisions`: **passed**. The single known M04 historical-fingerprint regression is deliberately excluded, not reported as an M02 regression or a full GM pass.

## Remaining acceptance boundaries

This does not close the known M03 or M04 contracts; the M04 adversarial test remains outside this M02 proof. GM still requires an independent reference-to-rebuild matrix and end-to-end transformed-history parity. Future native frame families must be explicitly classified by the reclamation reachability policy before accepting them. This record supersedes the *defect status* in `docs/m02-independent-reverification-2026-11-06.md`, but retains that document as the red-test audit trail.
