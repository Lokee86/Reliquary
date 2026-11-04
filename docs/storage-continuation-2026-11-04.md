# Storage continuation evidence — 2026-11-04

C02–C08 remain in progress; no product gate is accepted by this record.

- C02: Archive and Echo fresh projections replay before their first write, rejecting conflicting existing identities (`84aef57`, `06bb6af`).
- C03: Memory mutations check owner and file identity (`bdb0ff5`); initialization refuses existing records (`5ddee31`); metadata encoding precedes body append (`aacad40`).
- C04: Community validates against the same owner and container identity as its Graph (`e36ed3a`).
- C05: Entity writes check file kind and scope as well as owner ID (`a06acf4`).
- C06: fresh Ego projections replay before mutation and owner checks include container identity (`3e38c32`, `68406005`).
- C07: five vector format initializers reject duplicates before append; their stores reopen after rejected retries (`51c84ea`).
- C08: Freshness initialization rejects existing records before appending a policy (`70d552e`).

Focused package tests and the 23-package, 76-edge architecture checker passed for these changes. The complete workspace suite passed with loopback access through f0428f. Remaining capability acceptance includes source authorization, grouped publication and recovery, exact cross-owner closure, and the other requirements in the canonical tracker. The offline conversion corpus proof remains incomplete; normal runtime remains REL 2.1 only.

## Later storage work

- `4ee297a` accepts a grouped completion that names both existing and new owner Memories; replay retains both and rejects unknown IDs. `e95b3a2`, `daad009`, and `e92272a` fence uncertain owner writes and track the writer epoch.
- `12dca0e` creates initialized REL and PHY writers; `fe8a080` verifies caller Episode admissions, and `40d0637` reconstructs Episode work from Archive on reopen and supports live finalization and work admission.
- `2b2f92e`, `d2cd075`, `97ee32d`, and `06a4593` check owner Graph and Entity mention closure at replay. Principal-association history and authorized cross-owner Graph references remain unsupported without owner-authenticated proof; unavailable Memory payload is not synthesized.
- `52795d7`, `70d9a0f`, and `6fd0cc6` retain Entity/mention revisions, validate Entity identity and timestamps, and recover exact mutation retries.
- `80a204b` through `2f5ddad` validate Memory metadata and recheck temporal proofs when backing arrives, using a rebuilt body index.
- `d785e9c`, `eeb0e80`, and `4fba38f` preserve retired-profile and pruned-vector history while validating packed backing and rejecting new writes against unavailable payloads.
- `9205b07`, `1e00453`, `52bfcee`, and `cc96500` harden Freshness replay and retain accepted-use proof. `5e2e8e3` rejects partial Insomnia lease records; `c80804c` bounds malformed Archive attachment counts.

These are verified slices, not acceptance of C02–C12 or G1. Full-workspace tests passed with loopback access through `2b2f92e`; later changes require a fresh full-workspace run. PHY routed publication remains closed pending sealed candidate evidence; principal and cross-owner Graph proofs remain incomplete.

Continuation: `79a049e` implements deterministic Archive fragment identity and branch windows using the pinned reference hash and overlap algorithm. It does not persist fragments or assign Archive/global publication coordinates. Those require the owner publication history model; until then Archive-vector fragment provenance and generation cuts remain unverified. Dream compact Begin admissions likewise still require owner-derived physical-prefix proof. No acceptance gate is claimed.
The full locked offline workspace test suite passed with local loopback access at `79a049e`; workspace check, formatting, and the 23-package/76-edge architecture guard also passed. This is not product gate acceptance.

`4ee49d2` adds a validated, immutable Container publication-envelope decoder for future Archive and Dream replay. It exposes physical version, optional transaction time, reservation form, and opaque body; it does not establish Archive-local clocks or Dream prefix authority. Container tests, workspace compilation, formatting and the architecture guard passed after this change; the full suite passed immediately before it at `79a049e`.

`bdf90ec` exercises new REL creation through native turn, Episode, source-grounded Memory publication, reopen, and retry. `3463d1e` checks retries against the originally published Memory revision after later updates. `cac8a5f` and `1116b5f` reject Dream admissions naming absent owner records, future cuts, or identities created after a nonzero local cut; these are necessary checks, not physical-prefix, global-order, or birth-cohort authentication. Full locked offline workspace tests passed with loopback access at `1116b5f`; no capability or product gate is accepted by this evidence.

`0e69618` requires grouped completions to follow their source Episode physically; `86cb69c` also checks the Freshness birth turn against accepted Archive activity at the carrier's physical prefix. Tests rewrite valid files to place the Episode later or forge the activity cut and confirm owner open fails. Full locked offline workspace tests passed with loopback access at `86cb69c`; the production graph remains 23 packages and 76 edges. Dream physical birth/cohort prefix proof and durable Archive publication/fragment history remain incomplete.
