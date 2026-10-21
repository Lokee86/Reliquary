# Canonical history migration results — 2026-10-21

## Purpose

Record verified disposable conversion results and complete raw-byte costs before
the separate historical-equivalence and authoritative-promotion gates.

## Overview

All six pinned development REL/PHY files have disposable canonical V2 copies.
Every copy reopens through its concrete owner, retains owner identity and all
owner heads, and passes historical import settlement/completion validation.
Every authoritative source retains its original SHA-256 and modification time.
This completes conversion/admission and raw accounting; the remaining owner
historical equivalence matrix and authoritative promotion are separate gates.

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
times and complete physical-family counts. Its explicit flags say that full
historical equivalence and authoritative promotion are unfinished.

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
Architecture refresh for the standalone import converter remains in progress.

## Related docs

- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md)
- [Converter coverage](latest-offline-converter-coverage-2026-10-21.md)
- [Retained resolution contract](retained-entity-resolution-contract-2026-10-21.md)
- [Storage format](storage-format.md)

## Notes

Historical Memory, Archive, Entity, Relationship, Graph, Ego, Freshness, vector,
Dream and global projection acceptance remains separately recorded by the plan.
These measurements do not authorize deleting legacy readers or promoting an
unverified authoritative fixture. The temporary converters remain until those
gates pass.
