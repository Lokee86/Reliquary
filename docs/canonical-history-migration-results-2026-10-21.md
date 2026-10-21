# Canonical history migration results — 2026-10-21

## Purpose

Record six-source conversion, historical equivalence, raw-byte costs and
measurements before authoritative promotion and runtime retirement.

## Overview

All six pinned development REL/PHY files have disposable canonical V2 copies.
Every copy reopens through its concrete owner, retains owner identity and all
owner heads, and passes historical import settlement/completion validation.
Every authoritative source retains its original SHA-256 and modification time.
The six-source historical equivalence matrix has also passed with zero owner
domain mismatches. Authoritative promotion and legacy retirement remain separate gates.

## Raw logical history bytes

| Fixture | Frozen V1 bytes | Canonical V2 bytes | Saved bytes | Saved |
| --- | ---: | ---: | ---: | ---: |
| 14-day REL | 31,853,182 | 30,742,210 | 1,110,972 | 3.49% |
| 14-day PHY | 814,287 | 675,300 | 138,987 | 17.07% |
| 28-day REL | 80,056,100 | 76,542,131 | 3,513,969 | 4.39% |
| 28-day PHY | 1,840,755 | 1,507,220 | 333,535 | 18.12% |
| Ellis REL | 19,561,460 | 18,315,280 | 1,246,180 | 6.37% |
| Ellis PHY | 4,070,532 | 3,180,063 | 890,469 | 21.88% |
| **Total** | **138,196,316** | **130,962,204** | **7,234,112** | **5.235%** |

Compression is excluded. Every file byte is accounted for: the container header
is separate and each physical frame is charged exactly once. For native
publication envelopes, framing is charged to the concrete body family. Earlier
standalone publication links remain explicit separate rows. The
[concrete-family artifact](../benchmarks/canonical-history-family-bytes-2026-10-21.json)
records both overhead and body bytes, including all growth. In particular,
standalone semantic import evidence is larger than the old physical-prefix
checkpoint; this cost is included in the totals.

The [conversion artifact](../benchmarks/canonical-history-conversion-2026-10-21.json)
records source/output hashes, disposable output paths, owner heads, staging
times and complete physical-family counts. Its historical-equivalence flag now links the subsequent
[complete historical oracle](../benchmarks/canonical-history-equivalence-2026-10-21.json)
records all six successful historical comparisons; authoritative promotion remains pending.

## Resolution history acceptance

An independent raw-frame verifier reconstructs each current CVAERS03 postimage
and compares it byte-for-byte with the frozen CVAERS01 full comparison carrier.
All 8,134 active history postimages match in physical order, including revisions,
timestamps, decisions, candidate order, evidence fingerprints and every optional
unresolved field. All 169 previously inert CVAERS02 records are retained exactly
inside checksummed CVAERL01 carriers with zero projection effect. The
[resolution-history artifact](../benchmarks/canonical-resolution-history-equivalence-2026-10-21.json)
records per-fixture counts and independently equal ordered history digests.

## Verification

The corrected production core passes 2,110 library cases with five existing
ignored tests and no filtered cases, including all exhaustive recovery cases.
All 157 integration cases across 24 suites pass. The three focused conversion
regressions cover literal Echo/project evidence, standalone initial-history proof
with damaged-source refusal, and completed native proof with normalization
deduplication and exact completion retry. Structural and change-impact
documentation checks pass on a byte-verified 1,810-file local snapshot.

The sandboxed broad run could not reach a localhost mock and could not start one
Lore child process. The already-built core was rerun locally outside that
sandbox; all cases pass without changing production behavior or test assertions.
Architecture snapshot `dc54dee47978dbf0eaf046cf6fe06a364a9106e5e7b789e6caee103924ece48e` passes all23 rules with109,407 source nodes,4,184 relationships and zero compatibility warnings. The [source binding](../benchmarks/canonical-history-architecture-binding-2026-10-21.json) verifies15 production paths against the captured mirror and names subsequent test-only changes. A final post-retirement refresh is still required.

## Related docs

- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md)
- [Converter coverage](latest-offline-converter-coverage-2026-10-21.md)
- [Retained resolution contract](retained-entity-resolution-contract-2026-10-21.md)
- [Storage format](storage-format.md)

## Notes

Historical Memory, Archive, Entity, Relationship, Graph, Ego, Freshness, vector,
Dream and global projection acceptance now passes and is recorded by the plan.
These measurements do not authorize deleting legacy readers or promoting an
unverified authoritative fixture. The temporary converters remain until those
gates pass.


The [original-source preservation manifest](../benchmarks/canonical-history-original-source-preservation-2026-10-21.json) pins exact copies of all six original files and their six authority/checksum metadata files before promotion. It records the fixture repository's preexisting uncommitted state. This preservation does not modify that repository or promote any file.

## Historical equivalence

All six source/output pairs pass in2,338.55seconds. Every Memory, Entity,
Relationship, Graph, MemoryGraph and Ego cut agrees. Freshness agrees at every
activity cut and accepted Event coordinate; vector generations and exact matrix
identities, Dream pairs/processing/proofs and every global ordering point agree.
Archive publications, origins, dependencies, time/global coordinates and orphan
behavior agree in order, with exhaustive publication-prefix hashes.

Whole Archive snapshots and whole-owner global snapshots use representative
recorded cuts when exhaustive materialization would take many hours. This
sampling applies to combined snapshots only: concrete histories, Archive
publication prefixes and global ordering remain exhaustive. The oracle records
the selected cuts. Test-only scan caches are checked against normal queries at
initial/middle/head cuts; no runtime replay semantics change.

## Reopen and migration measurements

The [performance artifact](../benchmarks/canonical-history-performance-2026-10-21.json)
contains eighteen isolated optimized processes. First open means a fresh process;
the operating-system file cache is uncontrolled, so these are not cold-disk
measurements. Warm open is the mean of three subsequent close/reopens.

| Fixture | First open V1/V2 ms | Warm open V1/V2 ms | Migration ms | Input MiB/s | Migration peak resident MiB |
| --- | ---: | ---: | ---: | ---: | ---: |
| 14-day REL | 4,274 / 3,197 | 3,947 / 2,885 | 17,426 | 1.74 | 68.80 |
| 14-day PHY | 22 / 17 | 25 / 18 | 209 | 3.71 | 12.64 |
| 28-day REL | 8,522 / 7,067 | 8,114 / 7,895 | 49,583 | 1.54 | 158.03 |
| 28-day PHY | 62 / 50 | 58 / 49 | 502 | 3.49 | 14.79 |
| Ellis REL | 2,178 / 1,989 | 2,389 / 2,062 | 13,164 | 1.42 | 47.12 |
| Ellis PHY | 152 / 136 | 162 / 142 | 1,219 | 3.18 | 20.34 |

Oldest, middle and recent preloaded Memory snapshot fingerprints are identical
for each pair. Three lookup repetitions and preparation time are recorded
separately. Peak working set and committed bytes come from Windows lifetime
process counters sampled every5ms; they include fixed harness/startup overhead
and are not an allocation trace. Neither lookup nor peak memory shows a
universal improvement.

Derived checkpoints are deferred: the largest warm owner reopen remains about
7.9seconds, while the measured preloaded Memory lookups remain below a second.
This work has no defined interactive latency budget requiring a new persisted
acceleration layer. Revisit with a named workload and latency target rather than
adding another format during the hard cut. No checkpoint generation/load or
corruption behavior is introduced; the conditional implementation steps do not
apply under this measured deferral.

## Maintenance checks collected before promotion

The [repair/rewrite artifact](../benchmarks/canonical-history-repair-rewrite-2026-10-21.json)
records all six current files. Both an incomplete frame header and an incomplete
frame body recover to the exact original accepted bytes with identical public
owner snapshots. Default fresh-history rewrite reopens successfully, emits
exactly the planner-selected Memory IDs and retains their complete current
Memory facts after normalizing only revision/global/local coordinates, which
are intentionally new in a rewrite. Source hashes remain unchanged.

The [frozen reconciliation-policy artifact](../benchmarks/canonical-history-frozen-reconciliation-policy-2026-10-21.json)
compares the concrete Freshness rebasing decision for each frozen V1 REL with
its current V2 counterpart. All three agree exactly, including refusal to rebase
historical checkpoint authority without a verified reconstruction manifest.
This refusal is preserved behavior. Public identical/extension/divergence and
reclaim checks are tracked separately until all fixtures finish.

The current-only exhaustive corpus test is independent of the upgrader module.
It reads the permanent frozen oracle and current fixture hashes, then uses only
normal current owner opens. It survives removal of conversion tests and V1
decoders; the full source-versus-output comparison has already passed.
