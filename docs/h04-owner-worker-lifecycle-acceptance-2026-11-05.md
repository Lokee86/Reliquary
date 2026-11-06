# H04 owner worker lifecycle acceptance (2026-11-05)

H04 is accepted after H01 and H03 on the rebuild branch. This checkpoint covers Gateway-owned worker scheduling and lifecycle for Insomnia, Dream, Entity, Vector, and Freshness. G3 remains a separate complete-Gateway gate.

## Accepted behavior

- Insomnia retains its dedicated durable Episode lease/checkpoint worker. Claims and bounded Archive evidence are hydrated under short owner access; model extraction runs without an owner lock; publication revalidates the lease, owner, route, and current grants. A synced extraction checkpoint is reused after interruption rather than invoking the provider again.
- Dream, Entity, Vector, and Freshness use the owner specialty coordinator. Each supplied driver is a typed semantic workflow route bound to durable owner evidence; Gateway owns only its bounded polling lane, authorization fences, quiesce, join, and restart.
- An owner may have at most one coordinator lane for each specialty. Duplicate specialties, empty/oversized sets, zero polling intervals, a foreign Gateway Instance, an unavailable writer, or a missing REL grant fail before threads start.
- Gateway checks the authenticated writable owner immediately before each unlocked specialty poll and again afterward. Drivers use authenticated Instance operations for their own prepare/revalidate/commit phases; the coordinator exposes no Container, store, writer, or owner mutation callback.
- Provider, embedding, and evaluator work occurs in `OwnerWorkerDriver::poll` with no Gateway owner lock held. Slow external work therefore cannot block unrelated owner reads or other specialty lanes.
- Stop fences new polls, wakes idle threads, waits for every in-flight poll, joins all lanes, and reports a worker panic. Drop performs the same quiescent join.
- Restart constructs fresh volatile lanes over the same durable owner projections. Durable Insomnia leases/checkpoints and the Dream, Entity, Vector, and Freshness workflow receipts remain their respective retry authority; an ephemeral wake/poll never authorizes publication by itself.

## Focused evidence

The runtime Gateway tests cover:

- bounded Insomnia worker counts, bounded admissions, no provider call under owner lock, routed REL/PHY retry, extraction checkpoint reuse, provider failure hold, stop/join, and credential/owner fences;
- concurrent Dream/Entity/Vector/Freshness specialty lanes with a measured bound;
- quiescence with no active poll remaining;
- restart against retained driver state;
- rejection of foreign Instances, duplicate specialty authority, and invalid bounds.

## Verification

Run from `C:\!bin\workspace\Reliquary-rebuild` with the repository-local `target-codex-check` build directory. The verified-copy tests require normal Windows hard-link permission.

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

All commands passed at acceptance. Existing non-fatal warnings remain outside H04.
