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

## Recovery and focused tests

The owner-session checkpoint test crosses a signed standalone reservation and a grouped Memory correction. It compares the exact historical proof with full admission, exercises a vector row spanning two verified pages, checks an interrupted checkpoint staging operation and a damaged page, rejects a same-owner copy's checkpoint, and refuses a torn tail. Container tests also exercise equal-length prefix changes, timestamp continuity, and explicit tail repair. Archive and Memory tests cover digest-checked ranges, detached reads, and grouped payload retry.

The Root API and CLI tests passed. Two Root corpus-specific tests remain ignored in the default suite, and the existing cross-process Container test needs Windows symlink privileges. The native corpus runs in this record use copies directly.

## Related docs

- [Checkpoint and mount specification](../planning/rel-checkpoint-open-spec-2026-11-20.md)
- [Checkpoint execution plan](../planning/rel-checkpoint-open-plan-2026-11-20.md)
- [Storage architecture](../architecture/storage.md)
- [Earlier owner-open measurements](g5-owner-open-and-recovery-2026-11-20.md)

## Notes

The warmed measurements are local to the tested Windows machine. Cold checkpoints and full audits retain the complete verification cost. Warm admission restores compact history metadata and loads owner payloads on demand. A serialized projection above the 256 MiB cache limit, or a suffix outside the seeded replay path, uses full admission on the same lease.
