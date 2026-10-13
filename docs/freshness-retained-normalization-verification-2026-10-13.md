# Retained Freshness normalization verification — 2026-10-13

Parent index: [Documentation index](INDEX.md)

## Purpose

Retain the completed authoritative-fixture normalization and C10 evidence after deleting all disposable conversion tooling.

## Overview

All three authoritative fixtures were normalized at their original REL/PHY paths through exact owner/hash-fenced candidates. The dated [full before/after evidence](../benchmarks/freshness-retained-normalization-2026-10-13.json) and [C10 measurements](../benchmarks/freshness-historical-corpus-2026-10-13.json) retain immutable proof hashes, derivation claims, actual/approximation coverage and native acceptance results.

The JSON evidence provides a reviewable summary and cryptographic hashes for the losslessly compressed [complete native proof archive](../benchmarks/freshness-retained-normalization-full-proof-2026-10-13.json.gz). That archive retains every original before/after identity/history fingerprint, publication proposal/claim and labelled approximation root; compression removes no evidence.

## Owner preservation and generation

| Fixture | Derived legacy births | Genuine actual Dream proofs | Labelled legacy approximations | Final REL SHA-256 |
| --- | ---: | ---: | ---: | --- |
| chatgpt-first14d | 1087 | 36 | 1087 | `800d98cc09081901ffdc8e37b6c4dab60579e6356ac710a810bd37508343c851` |
| chatgpt-first28d | 2816 | 31 | 2816 | `3816e5a83c73a0c30465536402bab07a938c97cfaabf2b0be1a672ba2cc02c2b` |
| rhelm-david-r-ellis-pre-relationship | 678 | 47 | 678 | `4ab338734939e6bece41622b927648366ed7db33f91b0ec2eb49f390948d49f8` |

Accepted Archive identities/positions, branch identities/flags, all prior Memory revisions/body/provenance, existing Graph authority, actual Dream proof and accepted-use receipts passed native preservation comparisons. Original owner UUIDs remain unchanged. Missing all-branch Import coverage was completed through current Insomnia; compatible fragment/current-target vectors were genuinely embedded. Current publications received actual native Dream passes. Legacy births were reencoded only where original completion/first-revision/Episode/activity evidence proved them, and existing physical references were relocated through the standard storage helper. Legacy Dream approximation derives from proved direct accepted Graph transactions/evaluated pairs and is explicitly labelled. It never creates an actual first-cycle pass.

## C10 measurements

| Fixture | Operation seconds | Audit CPU seconds | Peak working set MiB | Recipient contributions | Checkpoint bytes | Reopen seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| chatgpt-first14d | 38.437 | 88.766 | 149.93 | 13377 | 97076 | 1.368 |
| chatgpt-first28d | 176.824 | 407.984 | 355.25 | 21346 | 232418 | 3.448 |
| rhelm-david-r-ellis-pre-relationship | 26.225 | 57.859 | 105.03 | 29463 | 66039 | 1.525 |

Each run accepted a new production-native checkpoint with nonempty contributions on a disposable, hash-verified accepted-input prefix. The authoritative accepted checkpoint was untouched. Prefix remeasurement uses native actual Dream evidence and semantic fragment events; it does not recreate the disposable legacy approximation. Both resulting distributions are retained separately in the report. Graph traversal, semantic coverage/match histograms, checkpoint costs, retries and original source hashes are recorded. These runs perform zero inference. Whole-process CPU/peak working set/pagefile include open, inventory, proof hashing and diagnostics. Allocator-only temporary peak and spill high-water were not instrumented; no such measurement is claimed.

## Lasting authority correction

The retained corpora exposed valid `DuplicateOf` aliases whose canonical representatives were later superseded. Historical resolution now traverses duplicate components and supersession chains together. Supersession pointers must agree with a unique accepted edge and proved owner-local birth. Cycles/conflicting pointers remain rejected. Existing ranking selects eligible current authority without modifying Memory/Graph history. Thirteen native matrix tests passed, including combined chains across reopen, immutable-history preservation, malformed authority rejection and actual no-link precedence. This is current product behavior, not a legacy migration requirement.

## Cleanup and release verification

N10 cleanup is complete. No fixture normalizer implementation/private hook, temporary credential copy, candidate/rollback sibling or recorded C10 verification copy remains. C11's converged source checks passed:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo check --locked -j2` | Passed |
| `cargo test --locked -j2 -- --test-threads=4` | 1,266 library tests passed; four ordinary measurement/environment tests ignored; 138 integration tests and doctests passed |
| `cargo fmt --manifest-path cli/Cargo.toml -- --check` | Passed |
| `cargo check --manifest-path cli/Cargo.toml --locked -j2` | Passed |
| `cargo test --manifest-path cli/Cargo.toml --locked -j2` | All 19 CLI tests passed |
| `python -m unittest discover -s scripts -p test_freshness_normalization_contract.py` | All 10 design oracles passed |
| `python ../engineering-standards/tools/docs_policy/check.py --repo .` | Passed |
| `python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main` | Passed |
| `python scripts/check_architecture.py --refresh` | Passed: 75 P4 guard fixtures, 161 classified APIs, 11 external-ownership fixtures and 23 Pitlord rules |
| `git diff --check` / `git diff --cached --check` | Passed |

The refreshed static snapshot contains 70,932 source nodes and 2,379 relationships, SHA-256 `dd2d545418a3103ed844209427941ba294ff4fde8eee39c6a29267ff01993b99`. It is scoped source-ownership evidence, not proof of arbitrary dynamic calls. Both exhaustive native checkpoint/live-use byte-prefix recovery tests passed in the full root suite. Standard Archive roundtrip passed on a prepared mixed-turn fixture with descending timestamps: two Nodes, one branch, one fragment. Verification uses repository-local scratch and process-local debug-disabled build profiles; scratch is removed afterward. An earlier debug 28-day run was superseded before checkpoint publication and is not acceptance/performance evidence. C12 final production disposition remains open.

Documentation impact:
- Inspected: current architecture, invariants, storage/API, development, routing/coverage/behavior maps, limitations, roadmap and active plans.
- Updated: implemented fragment/canonical authority, deleted-tool guidance, current hashes, retained verification record and N6–N10/C10/C11 dispositions.
- Not affected: physical storage framing, public options, default activation and dated prior release evidence.
- Compliance check: both structural/change-impact modes and patch hygiene passed on the final source/doc tree.
- Known documentation gaps: C12 disposition remains open; allocator-only peak/spill measurement scope is explicit.

Architecture impact:
- Standards added or changed: none; no new format or migration registry.
- Ownership or boundary impact: Dream owns canonical/proof authority, Archive owns positions/fragments, Freshness owns checkpoint/scoring; conversion code is deleted.
- Pitlord enforcement impact: all 23 rules passed on the freshly captured source snapshot, with existing P4/external-ownership guards.
- Other verification impact: full root/CLI, both exhaustive native recovery gates, all three native C10 runs, 13 composed-authority matrix tests and ordinary Archive smoke passed.
- Known architectural gaps: mounted historical management/default activation remain out of scope; C12 production disposition remains open.

## Notes

C12 is now complete in the [production disposition](freshness-historical-production-disposition-2026-10-13.md). The verification above records the C11 source cut `cff0abd`; its earlier C12-open statements describe that pre-disposition stage rather than a current release block.

The authoritative stored legacy-approximation checkpoint and native actual-proof prefix remeasurement are separate evidence scopes. Earlier rejected/superseded debug runs are not acceptance/performance evidence. Current ordinary imports retain actual proof and have no automatic legacy conversion fallback.

## Related docs

- [Normalization plan and disposition](freshness-retained-fixture-normalization-plan-2026-10-12.md)
- [Historical completion plan](freshness-historical-completion-execution-plan-2026-10-12.md)
- [Current API](api.md)
- [Current limitations](current-limitations.md)
