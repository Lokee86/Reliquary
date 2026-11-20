# Owner open and interrupted append comparison

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Compare 2.0 and 2.1 owner open on the retained REL and PHY pairs and check recovery after an incomplete append.

## Overview

On November 20, 2026, the release CLIs at 2.0 reference `c6010fd30dd52acb81c667f7aed99b831307e49f` and 2.1 candidate `430757096311d4ef9cf762718a45bdf528ba55ee` verified copies of all six owners. The migration receipts established equivalent retained history and pinned each input hash. Both CLIs ran on the same Windows 11 and Intel i9-11900H machine with Rust 1.100.0. Each file had one warmup and three measured fresh-process opens, with alternating version order and a warmed filesystem cache. Every open verified the original owner ID. The copies and original fixtures retained their checksums through the measurements.

## Owner open

| Pair | Owner | 2.0 median seconds | 2.1 median seconds | 2.0 peak MiB | 2.1 peak MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| 28-day | REL | 9.278 | 49.684 | 124.9 | 338.1 |
| 28-day | PHY | 0.084 | 1.008 | 10.8 | 19.8 |
| 14-day | REL | 2.473 | 14.802 | 54.4 | 149.7 |
| 14-day | PHY | 0.042 | 0.357 | 8.7 | 15.4 |
| Ellis | REL | 1.960 | 11.098 | 47.8 | 84.7 |
| Ellis | PHY | 0.202 | 2.852 | 15.4 | 35.6 |

Peak working set is the maximum across the three measured processes. All six owners exceeded the original G5 limits of 1.10× for median open latency and 1.15× for peak memory. On this host the 2.1 RELs took 5.4–6.0× as long and used 1.8–2.7× the peak working set. The owner-open performance gate remains open.

## Recovery

The recovery probe appended two bytes of an incomplete frame to a fresh copy of each version of the 28-day REL and PHY. Ordinary verification refused all four incomplete tails and left their bytes intact. The 2.0 owner repair and 2.1 Container repair each restored the exact pre-append SHA-256. All four restored files passed fresh-process verification with their original owner IDs. The recovery copies and original fixture files matched their expected checksums.

## Buffered replay follow-up

The 28-day REL contains 135,279 frames. The original release took 563 ms for one header-only scan and 1,117 ms for one scan that collected every payload. During owner open, separate domain passes multiplied the cost of checking the file length and seeking on each frame. A release build with just buffered, positioned reads brought the 28-day REL median to 3.60 seconds. Its peak working set was 349 MiB. This isolates the file reads as the main cause of the latency regression.

The final build also reuses one payload allocation per replay scan. The same Windows host ran three measured fresh processes for each owner and version, after one warmup and with alternating version order. A workspace build was active during part of the comparison. All six native owner reports matched the previous 2.1 reports exactly, and all source and copy checksums matched their opening values.

| Pair | Owner | 2.0 median seconds | Optimized 2.1 median seconds | 2.0 peak MiB | Optimized 2.1 peak MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| 28-day | REL | 9.936 | 3.099 | 124.9 | 249.7 |
| 28-day | PHY | 0.100 | 0.042 | 10.8 | 16.4 |
| 14-day | REL | 2.546 | 0.729 | 54.4 | 110.6 |
| 14-day | PHY | 0.033 | 0.023 | 8.7 | 13.2 |
| Ellis | REL | 2.057 | 0.372 | 47.8 | 72.3 |
| Ellis | PHY | 0.201 | 0.080 | 15.3 | 27.9 |

The seven affected package suites passed 261 tests. The cross-process lease case was filtered after Windows refused creation of its symlink fixture with error 1314. Formatting, documentation and the architecture guard passed.

All six measured opens beat the 2.0 wall-time reference. All six remain above the original peak-memory limit of 1.15× the corresponding 2.0 working set. The four interrupted-append cases passed again with optimized 2.1 verification and repair, preserving the original owner IDs and exact committed bytes.

### Hermes native host

A local Windows host built from the plugin sources against the optimized Core used copies of the same 28-day pair. Read-only Setup took 4.120 seconds, first signed attach 11.541 seconds, and attach after restart 8.686 seconds. First startup replays the owners during bootstrap, identity validation and mounting. A configured restart still replays each owner during both validation and mounting.

The host committed a synthetic REL Memory and reopened its exact ID, revision and body after restart. The source files and the read-only PHY copy retained their input checksums. These timings are from the local test host.

## Related docs

- [Initial native product acceptance and G5 scope](../p01-public-product-acceptance-2026-11-06.md#explicit-limits-and-next-gate)
- [28-day migration evidence](owner-history-migration-first28d-2026-11-18.md)
- [14-day and Ellis migration evidence](owner-history-migration-other-pairs-2026-11-19.md)

## Notes

Local evidence is under `C:/!bin/workspace-data/reliquary-verification-2026-11-20/g5`. The open and recovery JSON receipts contain the individual samples, tool hashes and per-owner results. This run covers fresh-process owner verification and physical tail recovery. The follow-up measurements and native smoke receipt are in the `optimized-owner-open` subdirectory.
