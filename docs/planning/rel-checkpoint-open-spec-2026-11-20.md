# Verified REL checkpoints and Hermes owner mounting

Parent index: [Planning](./INDEX.md)

## Purpose

Make a large REL usable promptly after restart. On the measured Windows host, the 79.8 MiB 28-day REL took 3.10 seconds and 250 MiB peak working set for one optimized Core verification. A local Hermes host took 11.54 seconds on first attach and 8.69 seconds on restart. Core replays the entire history to reconstruct its stores. Hermes repeats that replay during mount, with an additional replay when it builds the initial topology. [Measurements](../acceptance/g5-owner-open-and-recovery-2026-11-20.md)

## Overview

A trusted host mounts each selected file through one Core admission. A routine leased REL open restores a verified checkpoint, applies the committed suffix, and serves older records from the REL as operations request them. A checkpoint miss takes the full verified replay path and gives a writable session an opportunity to publish a new checkpoint.

The work starts from Core `7611f9a4` and Reliquary for Hermes `a93efd3`. The checkpoint fast path covers lease-owning REL writers, including General and project RELs, and scoped standalone REL reads. The active PHY and long-lived read-only Gateway mounts use one admission and return a detached snapshot. The public `Rel::open` and `Phylactery::open` constructors also return detached snapshots. Existing clients can hold a detached snapshot while mounting the same file for writing. Ordinary CLI REL metadata and bounded history commands use the scoped reader. Explicit verification and copy maintenance use full admission.

## Mount once

- Add a trusted Root Gateway path mount that takes the requested owner kind and access mode and an expected owner ID when one is already bound. It returns the actual admitted owner ID. Owner-session acquires the appropriate lease and checks the current-format header on that same handle before semantic admission. An ID or kind mismatch returns the existing admission error before the Gateway records a mount. The writer uses the admitted handle for its lifetime. The existing public handle-based mount methods use the same internal install path, and quiesced reopen supplies its registered owner ID.
- In Hermes `Registry::open`, build first-run or v1-upgrade topology from the IDs returned by staged mounts. Check canonical paths, unique role owners, and any persisted v1 bindings before publishing v2 or installing session admissions. A configured restart takes expected IDs from the accepted topology. The Registry publishes and authorizes the staged set only after all required owners have admitted. Preserve unavailable status for an optional retained REL that fails to mount. An already-mounted owner supplies its existing evidence.
- In live Apply, use the reviewed generation, path, and ID for the same mount operation. Track only the new mounts for rollback and use the existing publication and revocation flow. Explicit Setup validation and Review are separate operations with their existing checks. Apply performs one admission for each newly mounted owner. The standalone CLI local-host setup uses the same path mount with its existing grants.

## Checkpoint identity and integrity

**Authority.** The native 2.1 REL is the authoritative, portable file. Core owns a disposable, versioned checkpoint in its private local cache. The cache root is determined by the trusted local process, independent of selected owner paths and hosted requests. Require owner-private permissions and refuse unsafe redirections and untrusted cache entries. If that storage is unavailable, use ordinary full replay. A copied or downloaded REL brings only its ledger authority. The private cache stores typed state and indexes, with historical bodies and packed vector payloads referenced in the REL.

**Identity and format.** Use a slot keyed by canonical path and kind. Bind the manifest to the actual header, owner ID, physical file identity, schema and replay-validator revision, owner history generation, covered frame-end offset, and SHA-256 of the exact prefix through that offset. Record the clock frontier, domain cuts, and a bounded directory of typed state and index sections. Keep strictly bounded page lengths and counts and integrity checks for manifests and pages. Bind pages to their generation, section, and position. Bump the replay-validator revision when the admitted interpretation changes. Treat versions of the checkpoint as disposable derived schemas. The 2.1 header, frame encodings, owner IDs, and public cut tokens carry their existing meanings.

**Open on a hit.** Capture the leased file extent, check the header and manifest binding, and stream one SHA-256 pass over the covered prefix using the same handle. A native 2.1 owner proves the old bytes through this prefix digest comparison. The hash pass uses a bounded raw byte buffer. Semantic decoding starts at the checkpoint offset. Validate and restore the required state and index descriptors, then start the physical cursor at the recorded frame boundary. Validate the suffix through the captured extent before publishing the owner session. Verify the stable extent as part of admission.

**Failure and refresh.** A missing, wrong-version, foreign, corrupt, truncated, or prefix-mismatched checkpoint is a miss. Verify the authoritative file through the full path under the same lease. A bad authoritative prefix or tail fails admission according to the existing error and repair contract. A torn append requires explicit repair. A complete suffix that violates the existing replay rules fails verification. A clean authoritative append beyond the checkpoint is a valid suffix.

Build the first checkpoint after successful full writer admission. Refresh at clean quiesce and after a committed group takes the uncovered suffix past 16 MiB or 16,384 frames. These are initial refresh triggers. A crash or failed refresh can leave a larger valid suffix. Reuse the prefix hash state from admission and extend it over newly committed physical bytes. Merge the prior on-disk index and recent changes as a stream. Under the existing writer serialization, publish from an accepted cut after the REL has synced and its typed state has advanced. Stage and sync the new checkpoint before atomic replacement. On interruption, accept either complete generation whose binding verifies. Coalesce retries at the next refresh boundary. The REL commit determines the write receipt. Checkpoint publication is best effort. Explicit full verification reads the ledger independently and leaves the cache untouched.

## Replay and on-demand reads

- **Container** owns the file lease, header, stable extent, frame boundaries, hashing, and clock validation. Split leased header admission from full physical replay so a checkpoint hit can initialize the physical cursor and clock at the verified frontier. Include outstanding standalone reservations and access to recorded timestamps, including absent times. A suffix can bind a reservation from before the checkpoint. Keep the same accepted frame and explicit repair rules across both starts.
- **Owner-session and stores** own checkpoint capture, restore, and canonical replay. Start from an empty state for a full replay or from the validated seed for a suffix. Reuse each store's typed decoders, application rules, and cross-store proof checks. Process frames in physical order and advance the visible cut after their required effects are accepted. Keep sufficient continuation state for Archive and Memory versions and provenance, Graph and Community closure, Entity and relationship changes, Ego, vector mappings and generation receipts, Freshness, import and work journals, grouped Insomnia and Dream evidence, dependencies, user request receipts, and maintenance fences. Preserve cross-cut and in-progress work state. An unhandled continuation requires a full verified open. Checkpoint acceptance requires the pinned migrated 28-day REL to hit with its retained record families.
- **Current state** contains the heads, routing, eligibility, and minimal proof state required for the mounted owner. Store large values by validated reference. A warm open loads current state and index roots. Vector matrices are hydrated on demand. Vector lookup and scoring use the existing compatible profiles and load the rows they need on request. Suffix validation may fetch precise old facts needed for a new publication.
- **History** uses paged indexes ordered by owner-local key, version, and publication coordinate. Keep index pages and payloads on disk under a fixed buffer budget. Use offsets and lengths from validated frames and bind them to the expected record family, ID, cut, and payload digest. For records inside a grouped frame, retain the child coordinate and validate it through the owning group decoder. Use the existing positional reader through the live lease and the owning codec to resolve only the requested history or provenance. Preserve exact historical order, revision and availability, tombstones, pruned data, pagination, and stale-cut behavior. Propagate a damaged backing page or payload as a verification failure. Invalidate the checkpoint and fence writes for that session until normal reopen revalidates the owner.
- **Public boundary.** Root and Gateway expose typed reads and mount results. Hosted reads still pass the existing owner grants, revocation, and cut checks. The lease belongs to a mounted or scoped reader for the duration of its operations. The detached public snapshot constructor stays at the compatibility boundary and materializes through the same full replay. Domain ownership stays with the existing stores.

## Completion

- Cold full replay, checkpoint restore with a valid suffix, and explicit verification agree on owner metadata and current state. Native historic reads preserve values, order, provenance and availability. An unchanged checkpoint hit decodes zero old history payloads, and a historical read decodes the indexed records requested. Corruption and interrupted publication follow the existing failure contracts.
- Hermes admits each required owner once at bootstrap and restart and each newly selected owner once in Apply. Persisted routes, reviewed identity, writer exclusivity, and grants pass the existing restart and switch cases.
- Pass the startup budgets and focused verification in the [implementation plan](rel-checkpoint-open-plan-2026-11-20.md). Measure ordinary mounts separately from full audits. The first implementation still has one old-prefix byte read, so storage bandwidth and the current active set remain relevant at larger sizes.

## Related docs

- [Implementation plan](rel-checkpoint-open-plan-2026-11-20.md)
- [Storage architecture](../architecture/storage.md)
- [Owner sessions](../architecture/owner-session.md)
- [Public product API](../public-product-api.md)
- [Open and recovery measurements](../acceptance/g5-owner-open-and-recovery-2026-11-20.md)

## Notes

The integration boundary is current native files, the public Root API, and Reliquary's existing signed Hermes protocol. Ship the change through the normal Core source pin in the plugin. This work covers local startup and history access.
