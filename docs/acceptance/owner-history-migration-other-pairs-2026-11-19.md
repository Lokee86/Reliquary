# Original 14-day and Ellis REL and PHY history migrations

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the separate offline migrations of the original 14-day and Ellis REL and PHY pairs to native 2.1.

## Overview

Each pair was converted from copies checked against the original file hashes and verified against its own frozen source witness. All four converted files passed native admission and retained their owner IDs. Both RELs retained their original scopes. The legacy reader ran offline at `de7cec88fd632c339d644ec594e90fd1c6648f4e`. The writer, independent native reader, CLI and grant test matched the executable hashes from the 28-day proof.

## Results

| Pair | Owner | Source frames | Global cuts including genesis | Memory revisions | Archive publications |
| --- | --- | ---: | ---: | ---: | ---: |
| 14-day | REL | 46,989 | 18,939 | 3,348 | 7,526 |
| 14-day | PHY | 2,409 | 1,089 | 421 | |
| Ellis | REL | 39,144 | 11,726 | 1,452 | 3,763 |
| Ellis | PHY | 19,588 | 5,222 | 976 | |

The 14-day REL retained the backing cutoffs for 95 project attachments. The verifier matched all 18,202 global cuts where at least one reference still lacked backing and checked the pinned repository objects. It also matched 169 inert Resolution frames and 1,062 Insomnia work entries at their recorded cuts. The Ellis REL retained its neutral scope, its clocked owner metadata and its Archive history with zero project attachment bindings. Both pairs preserved cross-owner provenance and grouped private Memory links.

| Pair and file | SHA-256 |
| --- | --- |
| 14-day original `project.prj.rel` | `a02c8696db754c53a73c896133b06df9828a32923aee610c9f918a16698f76d0` |
| 14-day original `user.phy` | `7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63` |
| 14-day native `project.rel` | `dd12ac1e5854abbee36723932a2fb740e73cdf41719445e570e1e2d82afe5105` |
| 14-day native `user.phy` | `17323c938985b699e5939b0d0c9276cb9c0b87d669077c30ff3c3a2129219a9e` |
| Ellis original `project.prj.rel` | `cbc38d055936fc5702a17c4d569557d8c35e8887d1946c5562ba419038de6ee3` |
| Ellis original `user.phy` | `58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0` |
| Ellis native `project.rel` | `0f9dd01de0aa8416411538b953151590e84d01bcd0dd1004aedfeb405cab1b51` |
| Ellis native `user.phy` | `4c89004dc4c9125dbad71c36ffb03dfc807e38ce00b74a35c7f929ec139eb721` |

## Verification

- The 14-day pair verdict was `PASS`. Its `pair-verification.json` SHA-256 is `2b3678a453acc6a38c33eb9bab3703a144d6007472a6c81bd03da2680be7a795`.
- The Ellis pair verdict was `PASS`. Its `pair-verification.json` SHA-256 is `bed17504ab78dca1118c32a13bc89566da2ec0346a8bf3afcc5b94e4c4300af3`.
- Each receipt binds one pair's input, source witness, frozen expectation, output and executable hashes. All original, copied and converted file hashes matched before and after verification.
- The current CLI rejected all four original signatures and verified all four native files. For each owner it returned an earlier Memory and resolved provenance to the exact original transcript bytes. Each pair's native grant test allowed the private PHY read with the paired grants and denied it with the REL grant alone.
- The 14 offline comparison tests passed. The 28-day frozen expectation reproduced exactly, the verifier rejected a witness from the wrong pair, and the package graph check passed.

## Related docs

- [Original 28-day migration evidence](owner-history-migration-first28d-2026-11-18.md)
- [Migration specification](../planning/owner-history-migration-spec-2026-11-18.md)
- [Migration execution plan](../planning/owner-history-migration-plan-2026-11-18.md)

## Notes

The private run is under `.test-tmp/owner-history-other-pairs-20261118`. The `chatgpt-first14d` and `rhelm-david-r-ellis-pre-relationship` folders each have their source witness in `source/` and their native files and verification receipt in `pair-attempt-1/`.
