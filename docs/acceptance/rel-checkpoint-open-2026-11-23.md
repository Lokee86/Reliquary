# REL checkpoint startup verification

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the local startup, history, and write-recovery checks for verified REL checkpoints and the single-admission Hermes mount.

## Overview

The Windows run used Core `33f32b2d0d7d6e55a9ca817d4ff4ac735f2ff5df`. All startup and write checks used disposable owner files. The source files and the copied PHY retained their original SHA-256 digests.

| Workload | REL bytes | REL SHA-256 |
| --- | ---: | --- |
| Migrated 28-day REL | 83,691,741 | `a2d9a672b6a043e28b587d7daed5d4a191c41cba13d11d7dcfbd02048c93ee7f` |
| Matched 28-day control | 82,209,027 | `b77d49fa766cefa27c5ddef90b7fa594b0fa98885685b6510b55301b82125233` |
| Synthetic 365-day REL | 1,072,720,982 | `09695462383fe4f4b389c9acdf71a946a38b8456da557e36028f4f32a91a7244` |

The synthetic pair uses the same live Memory, conversation, and 1,536-dimensional vector. The year file adds retired history through native writes. The migrated 28-day PHY used with Hermes has SHA-256 `30ab8045169a47c1dd8b32d3de25cdc1ec257778de05a280336757fd9b9c0be0`.

Core measurements used the optimized owner-session probe with SHA-256 `0e75e29b99430ad4c7bd7a01f23d03d3984ddbc9e4003d8e3ea8151dd3be9ec6`. One cold writer open created the checkpoint and warmed the file cache. Three fresh processes then timed writer admission with builds idle. Each reported a verified prefix covering the entire file and an empty suffix. The prefix column times physical verification on separate fresh shared leases. Peak working set is the maximum of the three writer samples.

## Core results

| Workload | Cold open with checkpoint creation | Warm writer median | Prefix verification median | Warm peak | Detached full audit |
| --- | ---: | ---: | ---: | ---: | ---: |
| Migrated 28-day REL | 5.78 s | 0.891 s | 0.063 s | 100.83 MiB | 5.44 s |
| Matched 28-day control | 0.77 s | 0.069 s | 0.068 s | 13.97 MiB | 0.67 s |
| Synthetic 365-day REL | 9.35 s | 0.839 s | 0.835 s | 19.76 MiB | 9.24 s |

Both warm writer medians met the plan targets. Peak year working set was 1.41 times the matched control. The scoped read gave the same current and historical evidence as the leased writer and detached full audit. The migrated sample retrieved Memory revision 1 and its exact 78-byte Archive source turn. The year sample preserved revision 1 and the recorded terminal Archive state. The measured bounded history requests took less than 1 ms.

## Hermes results

The locally packaged host uses Hermes commit `a9cfd3a4ecb7f2ef95249352189434b0c8d56abd`, with native binary SHA-256 `ae725f31fc4b5fb0b137ee380972d11edb617488051ebab79f481c1e4cc255a4`. Every case started with a new profile and the separately copied 28-day PHY. Three fresh host processes then performed signed configured attach followed by clean shutdown. The checkpoint's digest and modification time stayed the same across the three timed runs, and the read-only samples preserved both owners byte for byte.

| Workload | Initial signed attach | Configured attach median | Warm host peak |
| --- | ---: | ---: | ---: |
| Migrated 28-day pair | 14.45 s | 1.014 s | 109.08 MiB |
| Matched 28-day control with the same PHY | 0.83 s | 0.228 s | 22.37 MiB |
| Synthetic 365-day REL with the same PHY | 9.36 s | 0.979 s | 27.34 MiB |

Both configured attach targets passed. A signed dashboard write on the copied 28-day REL returned a durable receipt. After a forced host restart, the dashboard read returned the same Memory ID, revision, body ID, and body. The pinned host passed all 11 native Registry tests and the signed role-switch test against Hermes `2bd378ed1b8d6bf2192cb26e0a55ff3091e6bd19`. The switch kept the same host process and preserved the pinned session and source owners.

### First-open trace

The original 14.45 s was a single observation. The receipt recorded elapsed time and peak memory. It has no timing for process launch, REL admission or the signed exchange, so the source of the excess delay in that run remains unknown.

A repeat used the same packaged executable and the same PHY, with a fresh profile, copied owners and an empty checkpoint slot for every sample. The order was migrated, year, year, migrated. Checking each copy's digest before launch warmed the operating system's file cache. The observer polled checkpoint and discovery files every 10 ms and read the native process's Windows I/O counters.

| First signed attach | Migrated 28-day pair | Synthetic year pair |
| --- | ---: | ---: |
| Observed totals | 4.54 s, 3.59 s | 9.53 s, 10.12 s |
| From host launch return to checkpoint file creation | 2.79–3.02 s | 9.35–9.93 s |
| From checkpoint file creation to publication | 0.65–0.86 s | about 0.04 s |
| From checkpoint publication through signed attach | about 0.14 s | 0.13–0.14 s |
| Logical bytes read by the host through attach | 2.32 GiB | 30.61 GiB |
| Checkpoint size | 75.31 MiB | 5.70 MiB |

A third run using the original benchmark without the observer took 3.34 s for the migrated pair and 8.65 s for the year pair. The original checkpoint timestamps also survive. Creation to last write took 0.57 s for the migrated checkpoint and 0.03 s for the year checkpoint.

The first interval includes physical validation, replay and the hash of the admitted REL prefix. The totals also include process launch, which took 0.52 s for the first sample and at most 0.02 s for the other three. Logical reads count repeated requests even when Windows serves the bytes from its file cache.

A separate optimized Core probe timed the same writer admission with counters around each phase. On the year REL, physical admission took 0.40 s, store replay and validation took 8.11 s, and creating the checkpoint took 0.84 s. Replay alone requested 28.04 GiB. For example, Graph replay and its history validation each read the full REL even though its Graph version was zero. The shared frame scanner reads each payload before the store checks its type. The year fixture has 8,761 turns, two retained Memory identities and one active Memory.

The migrated REL holds 16,465 turns, 2,847 Memory identities and a Graph version of 25,337. In the same probe, physical admission took 0.05 s, store replay and validation took 5.33 s, and checkpoint creation took 0.72 s. Memory replay alone took 3.57 s. These are standalone Core timings measured in a separate process.

A second Core probe timed `MemoryStore::apply_grouped_completion_inner` on a fresh migrated copy. Each grouped Insomnia completion clones the accumulated Memory store and replaces the old copy after validation. Across 1,344 completions, copying took 2.525 s and discarding the old states took 1.118 s. Together those operations accounted for 3.643 s of the 3.749 s spent in Memory replay (97.2%). They copied 2,169,241 Memory identities cumulatively. Of the 1,344 completions, 456 supplied neither a Memory body nor a creation record. The synthetic year REL contains zero grouped Insomnia completions.

### Grouped Memory replay after removing the per-group copy

Cold replay now moves the Memory store it is building through each grouped completion. Only a successful open exposes that store. The live writer still validates a group on a private copy before installing it. The focused regression check rejects a group after it has supplied a body and a creation, and verifies the live version, body availability and Memory identities. All 12 Memory Core tests and both owner-session tests for grouped replay and pruning passed.

On separate copies of the migrated REL with empty checkpoint slots, matched optimized Core probes timed one cold writer open each. Memory replay fell from 6.052 s to 0.085 s. The complete writer open, including checkpoint creation, fell from 9.905 s to 2.792 s. Both checkpoints were 78,965,245 bytes. An independent reader compared all 2,847 retained Memories and 5,727 revisions, including their metadata, body availability and contents. Both projections had SHA-256 `0562fd52818b03ff9321d9b55444941c96939ff6d57fa1e7d34ddf2105cd26dc`. The original revision-one Memory and its Archive source also matched their migration witnesses.

The native Hermes comparison used two local release builds from plugin `a9cfd3a4` and the same dependency lock. Of 788 host, vendor and lock files compared, only Memory Core's `store.rs` differed. Each build performed three interleaved first signed attaches on fresh 28-day REL and PHY copies with empty checkpoint slots. Another build was active on the Windows machine during these samples. System CPU utilization was 33–39%.

| Signed Hermes attach on the 28-day pair | Before | After |
| --- | ---: | ---: |
| Median first attach | 3.987 s | 2.602 s |
| Range of first attaches | 3.913–4.192 s | 2.538–2.652 s |
| Median host CPU time | 3.766 s | 2.422 s |

First attach took 1.385 s less at the median (34.7%). All six returned the same owner cuts and exact revision-one Memory body. On a separate reopen, the updated host used a checkpoint from the earlier build and returned the same signed result in 1.067 s. The checkpoint digest and modification time matched its original values.

### First attach with store-specific replay

The comparison starts from Core `2e140ad4`, which already includes the grouped Memory change above. The Windows host opens fresh copies with an empty checkpoint slot and the same migrated PHY for both RELs. Hashing the copies before launch warms the filesystem cache. Logical bytes read are requests counted by Windows, including bytes served from cache. Peak memory is the process's peak working set at the first successful signed attach. Historical queries run after that measurement.

The migrated REL contains 135,279 physical frames, including graph history, grouped completions and packed vectors. The synthetic year REL is dominated by 8,761 Archive turns. Its Graph version is zero. Filtered replay lets Graph and other stores pass over the bodies of those turns. Physical admission, Archive's own replay and hashing the prefix for the first checkpoint still read the owner.

A separate release-mode Core writer comparison measured the open before querying history. Logical reads fell from 2.294 to 1.528 GiB on the migrated REL and from 30.576 to 4.179 GiB on the synthetic year. Peak working set went from 154.4 to 138.4 MiB and from 20.8 to 20.4 MiB, respectively.

The Core runs compared all retained Memory metadata and bodies, tombstones, active relations and the Community snapshot. All projections and owner cuts matched. The migrated REL returned 2,847 Memory identities, 5,727 revisions and 36,405 active relations. The year REL returned two identities and 366 revisions, including its tombstoned Memory. Both returned the recorded revision-one body. The migrated Memory's Archive source matched its recorded digest, and the year Memory's original source turn remained terminal. Each candidate emitted the same checkpoint length as its baseline, 78,965,245 bytes and 5,975,076 bytes respectively.

The local Hermes builds used the same plugin source and dependency lock. The executable SHA-256 values were `23d2b0bebc346a26321e10edd5a340ba197aaf9853e402eeccd0df16fcbddc8a` before and `2aaa846d3d1c7de0d4ab33618fe653c6625f7d4f3068eab0fb3a98341cc62bf0` after. Each measured host returned the expected signed REL and PHY cuts. A separate signed request on the same host then checked the migrated revision-one body or the synthetic year's current Memory body against the fixture.

| First signed Hermes attach | 28-day before | 28-day after | Year before | Year after |
| --- | ---: | ---: | ---: | ---: |
| Samples | 5 | 5 | 2 | 2 |
| Median elapsed | 2.63 s | 2.82 s | 16.48 s | 5.61 s |
| Elapsed range | 2.24–3.82 s | 1.91–4.37 s | 9.48–23.48 s | 5.32–5.89 s |
| Median host CPU time | 2.48 s | 2.19 s | 14.87 s | 4.67 s |
| Median logical reads | 2.323 GiB | 1.553 GiB | 30.605 GiB | 4.204 GiB |
| Median peak working set | 192.4 MiB | 211.6 MiB | 29.4 MiB | 29.5 MiB |

Each pair of runs used fresh profiles and alternated which build ran first on successive pairs. After the first two 28-day pairs showed substantial variation, three more pairs were measured. The table includes all samples. Other workloads shared the machine, and overall CPU utilization reached 100% during the measurements. Both year attaches completed faster. On the migrated pair the host requested 33% fewer bytes and used less CPU at the median. Its median elapsed time increased by 0.19 s and its median peak working set rose by 19.1 MiB.

### Import receipts at checkpoint scale

An optimized Windows Core probe compared leased receipt references with the previous resident receipt projection. The checkpoint retains each receipt's admitted identity, its conversation coverage and titles, and its verified physical reference. Source message mappings are read from that frame when needed. The first open in each pair uses an empty checkpoint slot. A fresh process then opens the same disposable REL at the exact verified extent. The tests use the operating system's normal filesystem cache. They time checkpoint publication separately from admission.

Each real fixture used one matched pair against `e13607fa`, with matching REL hashes, verified extents, and restored owner cuts.

| Fixture | Full open, previous → leased | Exact reopen, previous → leased | Checkpoint bytes, previous → leased |
| --- | ---: | ---: | ---: |
| Reliquary import, 426,251,206 bytes | 5.228 → 5.249 s | 0.812 → 0.793 s | 60,326,683 → 55,352,179 |
| Space Rocks import, 164,542,974 bytes | 2.199 → 2.294 s | 0.461 → 0.404 s | 47,842,469 → 37,203,387 |

The synthetic six-import REL holds 216,000 mappings in 263,908,031 bytes. Its checkpoint fell from 224,563,900 bytes to 9,330 bytes. Sampled private memory for exact reopen fell from 274,743,296 to 2,625,536 bytes. The eight-import REL holds 288,000 mappings in 351,877,600 bytes. Its previous projection exceeded the 256 MiB limit on source ID, revision, and parent strings alone, which account for at least 281,232,000 bytes. Full replay on that build reached publication and left no completed checkpoint after the write failed. The candidate published a 12,105-byte checkpoint and selected its exact extent on the next open. Both candidate checkpoints returned all receipt mappings through verified reads, including the retained turn and branch in the last import. REL hashes and owner cuts matched. The synthetic runs overlapped downstream compile checks, so their timings are diagnostic.

The limit still applies to the typed projection. Exact reopen also reads the complete covered REL prefix for integrity, so the cost on a file with a small projection still depends on storage throughput. A prior phase probe on the 426 MB import copy attributed 0.129 s of a 0.756 s exact reopen to checking the prefix and 0.625 s to restoring the projection.

Full replay on the same copy spent 1.51–1.53 s of 4.98 s verifying the Graph prefix, which includes 2,256 historical Dream Frontiers. Their carriers account for 300,246,766 bytes of the REL. In a separate diagnostic sample, decoding those Frontier JSON records took 1.112 s of the 1.480 s Graph prefix phase. Embedded Begin decoding and population construction took 0.042 s. The diagnostic ran alongside compile checks, completed in 5.018 s, and reproduced the owner's hash and restored cuts.

## Recovery and focused tests

The owner-session checkpoint test crosses a signed standalone reservation and a grouped Memory correction. It compares the exact historical proof with full admission, exercises a vector row spanning two verified pages, checks an interrupted checkpoint staging operation and a damaged page, rejects a same-owner copy's checkpoint, and refuses a torn tail. Container tests also exercise equal-length prefix changes, timestamp continuity, and explicit tail repair. Archive and Memory tests cover digest-checked ranges, detached reads, and grouped payload retry.

The selected-frame replay change passed 309 tests across Container, the eight stores and owner-session. The focused regressions cover physical clock effects in skipped records, exact payloads and positions, skipping indexed bodies without reading them, scanning after an append, nested import receipts, and rejection of malformed source records inside a generic publication envelope.

The Root API and CLI tests passed. Two Root corpus-specific tests remain ignored in the default suite. The cross-process Container test exercised direct paths and hard links on this machine. Windows requires an additional privilege for its symlink case. The native corpus runs in this record use copies directly.

## Related docs

- [Checkpoint and mount specification](../planning/rel-checkpoint-open-spec-2026-11-20.md)
- [Checkpoint execution plan](../planning/rel-checkpoint-open-plan-2026-11-20.md)
- [Storage architecture](../architecture/storage.md)
- [Earlier owner-open measurements](g5-owner-open-and-recovery-2026-11-20.md)

## Notes

The warmed measurements are local to the tested Windows machine. Cold checkpoints and full audits retain the complete verification cost. Warm admission restores compact history metadata and loads owner payloads on demand. A serialized projection above the 256 MiB cache limit, or a suffix outside the seeded replay path, uses full admission on the same lease.
