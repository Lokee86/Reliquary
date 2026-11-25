# Copied-source deletion cascade

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record focused verification of committed delete and rewind on disposable Reliquary owners.

## Overview

Archiving and restoring retain the accepted source, its knowledge, and the capture binding. A committed delete retires the whole copied source. A rewind retires the removed suffix and permits new writes at the surviving head. Core applies the removal cut to source reads and derived content. The host retains each pending change until Core acknowledges it.

## Core verification

All 38 Archive tests passed, including an independent copied branch whose shared evidence survived a rewind. The owner-session tests passed verified rewind checkpoint recovery and the direct, grounding and episode provenance cases across reopen. The 16 mounted Gateway retrieval tests passed, including stale prepared and retained Observation disclosures after a committed tombstone. They used synthetic data and disposable RELs.

The product source revision pinned in both hosts is `91b6ea7081d4b5c2e7bf1eaab1145da345235abd`. The later checkpoint test correction changes fixture code only. A Community snapshot that contains an ineligible Memory is hidden until the existing Community recomputation publishes a current snapshot.

## Hermes verification

Both signed native variants passed on Windows. The committed variant used Hermes transcript authority `4f1bf621049ebbf811ed4a3b1cc444c910db5d0f` and the inference checkout `168a923023a7018be3dbf66530531dd20b1834b9`. It verified recall while archived, a committed rewind with a surviving prefix, exact replay after the first acknowledgment was withheld across restart, a new copied Memory after rewind, and whole deletion after archive. Exact Core-backed Memory inspection denied the removed body, and a prepared retrieval could no longer disclose it. The disposable PHY was byte-identical.

## Mercury verification

The native Windows Gateway WebSocket fixture passed at Mercury `8363be6` with the pinned Core. On a disposable profile it committed a rewind with capture disabled, restarted and applied the retained notice, removed the old Archive hit, and accepted a new independently recalled Memory. Archiving preserved Observation recall. The fixture then deleted the archived chat while capture was paused, restarted, and verified that its supporting Memory and Observation were withheld while an unrelated Memory was returned.

Mercury `6308668` adds recovery of legacy archive-only adapter fences. It verifies an exact live receipt before releasing the fence and records a different durable fence for current delete notices. The exact final revision passed the single native WebSocket fixture in 72.24 seconds on a GitHub-hosted Windows runner. Local Windows Application Control had blocked launch of the updated executable. [Mercury archive-fence CI job](https://github.com/Lokee86/hermes-rust-rewrite/actions/runs/45554156993/job/139002083458)

## Core GitHub CI follow-up

The scripted Application fixture verified a same-wording successor using independent evidence after the original source was removed. The GitHub-hosted Windows job executed exactly one test. [Core Observation re-review CI job](https://github.com/Lokee86/Reliquary/actions/runs/45554127709/job/139001989705)

The external-consumer fixture exposed a reclamation scanner gap on its first hosted run. Core could reopen native-import historical Freshness snapshots, but reclamation refused their `RELHFRS1` frame. The fix retains the replay-validated snapshot unchanged. The focused rerun executed exactly one test and verified copied-source rewind and terminal scope across reclamation, including receipt replay, continued capture and a surviving independent copy. [Core external-consumer CI job](https://github.com/Lokee86/Reliquary/actions/runs/45556874577/job/139010936193)

## Related docs

- [Copied-source deletion cascade specification](../planning/copied-source-deletion-cascade-spec-2026-11-24.md)
- [Public copied-source API](../public-product-api.md#native-and-copied-source-import-contracts)
- [Owner storage and replay](../architecture/storage.md)

## Notes

These tests verify logical removal from Reliquary's read paths. The live owner files, existing profiles and separately running processing were outside this verification.
