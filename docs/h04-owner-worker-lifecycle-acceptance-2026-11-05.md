# H04 owner worker lifecycle audit (2026-11-05)

H04 is **not accepted**. Commit `e4ac3b18` added a bounded polling coordinator, but its callback-only API does not implement the H04 deliverable.

## Verified implementation

- Insomnia is a functional worker. Its durable Episode claim/checkpoint is reconstructed from the REL owner, external inference occurs outside the owner lock, and publication revalidates owner authority and the current lease.
- Gateway now owns an atomic `(owner, specialty)` lease registry shared by every clone and coordinator. A second Dream, Entity, Vector, or Freshness lane for the same owner is rejected until the first lane has quiesced and released its lease.
- Stop and drop fence new polls, wake idle threads, join in-flight work, and release leases only after joining.

## Missing behavior

- `OwnerWorkerDriver` remains an injected polling callback. No production Dream, Entity, Vector, or Freshness driver exists and no mounted-owner lifecycle starts one.
- Dream inference is not split into owner snapshot, unlocked provider evaluation, and revalidated publication. The current `draft_dream_organization` API receives an owner session while calling classifier/verifier endpoints, so using it from a worker would violate the no-provider-under-owner-lock requirement.
- Entity and Vector have real prepare/evaluate/commit workflows and durable owner-session receipts, but no durable work-discovery source or worker integration consumes them.
- Freshness has a rebuildable `FreshnessDueDispatcher`, but Gateway does not rebuild it from durable owner records, drain it in bounded batches, or publish lifecycle notifications/receipts.
- Restart tests for the specialty coordinator retain an injected in-memory counter; they do not recover eligible work from durable owner projections or prove idempotent publication.
- The coordinator currently admits only writable REL owners, while the runtime design calls for eligible REL/PHY owner-local workers.

H04 may be accepted only after these gaps have production paths and restart, authorization, quiescence, and duplicate-publication tests over reopened synthetic owners.
