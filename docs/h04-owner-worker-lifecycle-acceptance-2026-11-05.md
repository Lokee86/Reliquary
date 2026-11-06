# H04 owner worker lifecycle acceptance (2026-11-05)

H04 is accepted after correction of the callback-only coordinator introduced by `e4ac3b18`.

## Accepted behavior

- Insomnia retains its durable Episode claim/checkpoint worker, with provider work outside the owner lock and lease/authority revalidation before publication.
- Dream, Entity, Vector, and Freshness now have concrete Gateway worker types and are started through `OwnerWorkflowSet`; the generic driver trait and raw scheduler are no longer public integration APIs.
- Dream publishes an evaluated Application batch through an authenticated REL/PHY route. Owner-session rechecks the owner, graph and Memory cut and uses the durable publication identity for idempotent retry.
- Entity prepares owner-bound facts, releases owner access for model work, and publishes through the durable mention receipt. A reopened retry detects the accepted receipt before invoking the endpoint.
- Vector prepares owner-bound sources, performs bounded embedding without an owner lock, and publishes through durable profile/body rows. A reopened retry reuses accepted rows without inference.
- Freshness rebuilds its volatile bounded due scan from the current durable REL retrieval cut and owner policy. Notifications remain derived at-least-once hints; owner Freshness records, not the queue, are authority.
- Writable REL and PHY owners are supported where the specialty workflow is eligible. Missing grants, read-only mounts, foreign Instances, and revoked credentials fail at authenticated owner operations.
- Gateway owns one atomic `(owner, specialty)` lease registry shared across all Gateway clones and coordinators. Duplicate active lanes are rejected and leases are released only after quiescent join.
- Stop/drop fence new polls, wake idle workers, wait for in-flight workflow operations, join all threads, and then release specialty leases.

## Recovery and negative evidence

Focused tests reopen synthetic owners and prove:

- Dream batch retry is idempotent after Gateway teardown/reopen.
- Entity and Vector retries make zero provider calls after their durable receipts/rows reopen.
- Freshness rescans the same durable owner clock after reopen rather than trusting an ephemeral wake.
- duplicate specialties are rejected across independent coordinators and cloned Gateway handles;
- quiescence leaves no active poll and allows a replacement coordinator;
- foreign Instances, invalid bounds, unavailable writers and permission fences fail closed.

## Verification

From `C:\!bin\workspace\Reliquary-rebuild`:

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

All commands passed. The architecture guard reports 23 packages and 76 approved production dependency edges; the complete Gateway suite reports 51 passing tests.
