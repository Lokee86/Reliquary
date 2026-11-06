# G3 complete Gateway acceptance on synthetic owners (2026-11-05)

G3 is accepted on the rebuild branch after H01–H04. This gate accepts the complete internal Gateway/runtime behavior on generated REL 2.1 owners. It does not claim the final public `Rel`/Phylactery API, consumer CLI, G4 product acceptance, old-data conversion, or release cutover.

## Prerequisite closure

- [H01 owner mount and Instance isolation](h01-owner-mount-acceptance-2026-11-05.md): one canonical owner registry/writer, authenticated immutable grants, duplicate/foreign/stale authority fences.
- [H02 durable native conversation queue](h02-native-conversation-queue-acceptance-2026-11-05.md): owner-qualified journal, ordered submit/finalize/cancel, branch/fork validation, and Archive-commit crash reconciliation.
- [H03 host delegation and committed streams](h03-host-stream-acceptance-2026-11-05.md): negotiated host authority, durable Held outcomes, no blind redispatch, checkpoint-before-visibility, private reconnect/resync.
- [H04 owner worker lifecycle](h04-owner-worker-lifecycle-acceptance-2026-11-05.md): bounded Insomnia/Dream/Entity/Vector/Freshness scheduling, unlocked external work, restart, and quiescent join.

G2 is the accepted semantic/Application prerequisite. All Gateway publication paths use those named workflows and owner-session operations; Gateway does not encode Archive or Memory records directly and does not expose Container or mutable store handles.

## Integrated acceptance

The synthetic REL 2.1 Gateway suite proves:

- read-only and writable REL/PHY mount identity, wrong-kind rejection, physical writer exclusion, and duplicate-owner rejection;
- trusted credential installation, immutable principal/REL/PHY grants, malformed/duplicate credential rejection, live revocation, cross-Gateway rejection, and authorization checks before lookup and again before commit/delivery;
- durable owner-bound queue recovery, exclusive writer, ordered conversation revision, idempotent enqueue, principal-scoped cancellation/finalization, canonical branch versus explicit fork, exact Archive receipt, and retry after interruption between Archive sync and queue receipt;
- corrupt complete journal frame rejection, incomplete tail truncation, failed append invisibility, and durable cancel/finalize winner across reopen;
- complete host queue-contract negotiation, native fallback for incomplete declarations, stable operation identity, intent-before-handoff, exact receipt validation, Held unknown outcomes, status-only reconciliation, cancellation, reconnect fences, and no redispatch under ambiguity;
- generation checkpoint durability, full-prefix validation, independent delivery cursors, snapshot/watermark reconnect, bounded replay/resync, cross-Instance subscription rejection, and revocation during streaming;
- bounded worker admission/concurrency, provider/embedding work outside owner locks, durable Insomnia checkpoint reuse, routed REL/PHY interruption recovery, specialty scheduling uniqueness, restart, and quiescent shutdown;
- end-to-end Gateway paths for Entity resolution, vector publication, project correlation, external reference hydration, context composition, retrieval delivery, confirmed use, and owner reopen with source/grant revalidation.

Fault and negative tests use generated current-format owner files. No REL 2.0/FRS07 runtime reader or fallback path is present in Gateway.

## Architecture and verification

The executable architecture guard reports exactly 23 packages and 76 approved production dependency edges. Gateway depends on Application and owner-session through the approved graph; there is no reverse dependency into Gateway/Root and no second owner authority.

Run from `C:\!bin\workspace\Reliquary-rebuild` with the repository-local `target-codex-check` build directory. The verified-copy tests require normal Windows hard-link permission.

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
```

All commands passed at acceptance, including the complete `reliquary-runtime-gateway` suite and workspace doctests. Existing non-fatal warnings are unchanged and do not weaken G3 behavior evidence.
