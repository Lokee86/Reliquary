# H02 durable native conversation queue acceptance (2026-11-05)

H02 is accepted after H01 on the rebuild branch. This checkpoint covers the owner-scoped native submission queue and its operational journal only; it does not claim H03 host delegation/streaming, H04 worker lifecycle, or G3 acceptance.

## Accepted behavior

- Each mounted REL has one owner-qualified queue journal and one exclusive queue writer. The journal header binds the durable owner identity, and a journal copied under another owner name fails closed.
- Submission admission is principal-scoped and idempotent only for the exact same owner, conversation, submission identity, content hash, and expected conversation revision.
- Conversation revisions serialize submissions and terminal transitions. A stale parent cannot advance the canonical branch; an explicitly named, valid fork can commit against its authorized parent.
- Cancellation and finalization have one durable winner. Only the principal that submitted the pending operation can cancel or finalize it, and terminal results remain idempotent after reopen.
- Finalization first records durable intent, then commits and syncs the native Archive turn and branch through Application, then records the exact Archive receipt. Queue code cannot fabricate that receipt because its proof constructor is private to the trusted publication path.
- If the process stops after Archive commit but before the queue receipt is appended, reopening and retrying finds the exact Archive node and records the receipt without publishing a duplicate turn.
- Journal recovery truncates only an incomplete final frame, rejects a corrupt complete frame, and does not expose a transition whose append/sync failed.
- Host-operation and generation records share the same owner journal, but their arbitration and recipient streaming remain H03 scope.

## Focused evidence

The `reliquary-runtime-gateway` suite covers owner binding, exclusive queue access, ordered pending replay, stale revisions, duplicate payload conflicts, durable cancel/finalize races, principal-scoped cancellation, canonical-branch rejection and explicit fork acceptance, Archive receipt validation, incomplete/corrupt frame recovery, and the Archive-commit-before-queue-receipt crash window.

## Verification

Run from `C:\!bin\workspace\Reliquary-rebuild` with the repository-local `target-codex-check` build directory. The verified-copy tests require normal Windows hard-link permission.

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

All commands passed at acceptance. Existing non-fatal warnings remain outside H02.
