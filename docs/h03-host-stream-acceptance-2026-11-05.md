# H03 host delegation and committed stream acceptance (2026-11-05)

H03 is accepted after H02 on the rebuild branch. This checkpoint covers trusted host arbitration, durable operation reconciliation, and recipient delivery of committed generation checkpoints. It does not claim H04 worker lifecycle or G3 acceptance.

## Accepted behavior

- Trusted host adapters declare configured capabilities and negotiate an explicit per-capability contract. Conversation delegation becomes ready only when durable acceptance, stable IDs, linearizable cancel/dequeue, queryable status, and idempotent handoff are all declared; incomplete declarations retain native authority.
- A host handoff intent with stable operation ID and payload digest is appended and synced before the adapter is called. Once an intent exists, retry and restart paths query reconciliation/status and never invoke blind redispatch.
- Ambiguous handoff, status, cancellation, connection, or reconciliation outcomes retain durable ownership debt and enter `Held`/`UnknownStatusHeld`. Authority is released only after a terminal operation observation or clean reconciliation with no known operation debt.
- Capability routers are bound to the Gateway that created them. A router from another Gateway cannot arbitrate an Instance's work.
- Generation output is represented as bounded, full-prefix checkpoints. A delivery event becomes visible only after its journal frame has synced; a failed append exposes no text.
- Recipient subscriptions are opaque and bound to the authenticated Instance, REL owner, submission, operation, generation, and branch. Another Instance cannot reuse the handle, current REL authorization is checked on every delivery, and credential revocation fences an existing subscription.
- Snapshot watermarks are independent stream cursors. Reconnect produces a fresh authoritative snapshot; lag outside bounded replay yields `ResyncRequired` instead of a partial or guessed continuation.
- Queue reopen recovers the last synced checkpoint, stream watermark, Held state, and host operation identity.

## Focused evidence

The `reliquary-runtime-gateway` suite covers:

- complete and incomplete capability negotiation;
- connection failure and reconnect fencing;
- known-operation ownership debt and clean terminal release;
- intent-before-handoff, exact receipt validation, status-only recovery, and no redispatch after uncertain handoff;
- stable host identity across dispatch, reconcile, and cancellation;
- rejection of a router belonging to another Gateway;
- checkpoint append failure before visibility;
- synced checkpoint/status delivery, journal reopen, bounded replay, explicit resync, and fresh reconnect watermark;
- cross-Instance subscription rejection and grant revocation during delivery.

## Verification

Run from `C:\!bin\workspace\Reliquary-rebuild` with the repository-local `target-codex-check` build directory. The verified-copy tests require normal Windows hard-link permission.

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

All commands passed at acceptance. Existing non-fatal warnings remain outside H03.
