# Original 28-day REL and PHY history migration

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the first offline migration of the original `chatgpt-first28d` REL and PHY to native 2.1.

## Overview

The original files were pinned and read through disposable copies. Both converted owners passed the ordinary 2.1 CLI verifier and the independent retained-history comparisons. The CLI rejected the original container signature for each owner.

## Results

| Owner | Original frames | Global cuts including genesis | Memory revisions | Native output bytes |
| --- | ---: | ---: | ---: | ---: |
| REL | 151,185 | 52,805 | 5,727 | 83,691,741 |
| PHY | 7,192 | 2,216 | 490 | 1,706,572 |

| File | SHA-256 |
| --- | --- |
| Original `project.prj.rel` | `76c15780bd7cfe7e3083dbff5684e236a8fd2090fb0a82ec3d66529d14307d87` |
| Original `user.phy` | `32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d` |
| Native `project.rel` | `a2d9a672b6a043e28b587d7daed5d4a191c41cba13d11d7dcfbd02048c93ee7f` |
| Native `user.phy` | `30ab8045169a47c1dd8b32d3de25cdc1ec257778de05a280336757fd9b9c0be0` |

The REL matched 21,068 Archive publications, including 16,465 turns. All 99 project attachment bindings kept their original backing cutoffs, covering 49,677 global cuts where at least one reference lacked backing. The pinned repository objects were independently checked. Both owners matched their Graph, Entity, Resolution, Community and vector histories. The REL's vector generation receipt resolved to the relocated native object, and the Insomnia, Dream and retained Freshness journals and settlement matched.

The source REL stored local Episode and node coordinates on its Memories. The converter used those retained fields to form native provenance for 5,727 revisions. The source PHY used a prefixed representation of the paired REL UUID on 276 revisions. The source REL used the same form for 246 grouped references to private Memories. Each reference kept its underlying owner and object identity.

The REL retained its project scope. The PHY source had no scope value, which maps to the neutral 2.1 header. Both files retain the original owner UUIDs.

## Verification

- The independent pair verdict was `PASS`. The before and after SHA-256 hashes matched for both originals, both disposable inputs, and both converted owners.
- The CLI returned the earlier Memory at local cut 1 for both owners. The PHY read also matched the original revision-one body. The CLI resolved provenance to exact original transcript bytes for both owners. PHY provenance was restricted without the matching REL grant. The actual-pair Root test allowed the private Memory read with the paired grants and denied it with the REL-only grant.
- The locked offline workspace suite passed 701 tests. The ignored actual-pair case passed separately. The existing Container symlink test was skipped after Windows returned error 1314. The detached writer, detached reader and comparison tests passed 11, 22 and 14 cases respectively.
- Formatting, the locked offline workspace check, the CLI build, documentation policy and the 23-package, 76-edge production architecture check passed. The separate architecture negative-control runner expects a different diagnostic for its workspace-inheritance case. It reproduced the same failure at the starting revision.

## Revisions

- Core starting revision `184857a58bb283fc3bbfca4f6073a1e7af701acb`.
- Recovered D01 and D02 code from `5f2b1a7c`. Native port exercised at `c88ff160998641cb02177799722f471d741689d6`.
- Isolated F03 reader `774af82ade4a5de1aaeece2a2e382f36e125f6eb`.
- Frozen source coverage SHA-256 `1fa3c96cdfb96a80dbac94af764c980fffd58e5286e0d56728162d442c349053`.
- Source witness manifest SHA-256 `3957af793a5cb8b1ef1ef879f478bb70d91e5bba99e00a161e2bd0b262d14217`.
- Frozen expectation SHA-256 `936125efe2093e9faff10edf562e9a1895649f69f5d8dec140bdd4fd3ba93cce`.
- Paired receipt SHA-256 `12d57562317fe333d8779bf01d22a451f438df92d0a72c9ff5fa9b6a24cd5b64`. The receipt records the writer, reader, CLI and grant-test executable hashes.

## Related docs

- [Migration specification](../planning/owner-history-migration-spec-2026-11-18.md)
- [Migration execution plan](../planning/owner-history-migration-plan-2026-11-18.md)
- [Native storage and durable state](../architecture/storage.md)

## Notes

The private run is under `.test-tmp/owner-history-first28d-20261118`. The source witness and family receipts are in `source/` and the native files and tool receipts are in `pair-attempt-1/`.
