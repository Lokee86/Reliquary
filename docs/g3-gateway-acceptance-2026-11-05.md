# G3 complete Gateway acceptance on synthetic owners (2026-11-05)

G3 is accepted after corrected H04 closure. This gate covers the complete internal Gateway/runtime on generated REL 2.1 and PHY owners; it does not claim the later public API, CLI, parity, or release-cutover phases.

## Integrated acceptance

- H01: canonical owner mounting, writer exclusion, immutable Instance grants, revocation and cross-Gateway fences.
- H02: durable owner-qualified conversation queue, ordered submit/finalize/cancel, branch/fork validation and crash reconciliation.
- H03: negotiated host authority, durable Held outcomes, checkpoint-before-visibility and private reconnect/resync.
- H04: functional Insomnia/Dream/Entity/Vector/Freshness workers, unlocked provider work, Gateway-wide specialty uniqueness, reopened-owner recovery and quiescent shutdown.

The synthetic Gateway suite covers read-only/writable REL and PHY mounts; negative grants; credential revocation; journal corruption and incomplete tails; ambiguous host outcomes; stream cursors and reconnect; Entity and Vector source binding; Dream batch receipt retry; Freshness reconstruction; provider-call suppression after reopen; and worker authorization, uniqueness and shutdown.

Gateway publication continues through named Application and owner-session workflows. It exposes no Container or mutable store handle and introduces no second owner authority or legacy REL reader.

## Verification

The final acceptance run passed:

- architecture guard: 23 packages, 76 approved production edges;
- formatting and locked offline workspace check;
- full locked offline workspace test suite and doctests;
- `reliquary-runtime-gateway`: 51 passed, 0 failed.

Existing compiler warnings are unchanged and outside H04/G3 behavior.
