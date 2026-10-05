# Multiplexing Phase 4 — P4.4 non-conversation publication

Parent index: [Documentation index](INDEX.md)
Parent plan: [Phase 4 distribution plan](multiplexing-phase4-distribution-implementation-plan-2026-10-04.md)

## Purpose

Record the P4.4 gateway and concrete REL worker publication boundary on the independent P4 branch, following P4.3 commit `400fe7c`.

## Overview

One canonical gateway mutation adapter holds current attachment/owner admission and the mounted REL sequencer gate across mutation, required synchronization, source-clock capture and summary publication. Workers share that same sequencer. Semantic stores remain authoritative; notifications do not become another persistent log or semantic version.

## Coverage and ownership

| Family | Boundary and classification |
| --- | --- |
| Memory create/replace and Graph relations | Explicit gateway mutators; SemanticOwnerChanged and RetrievalInvalidated, plus ArchiveChanged if its clock actually advances |
| Community rename | Committed owner name change produces semantic/retrieval invalidations even when semantic clocks do not move; unchanged names produce nothing |
| REL metadata and dependencies | OwnerLifecycleChanged; dependency changes also invalidate retrieval |
| Principal projection | Source PHY remains independently authorized; only a changed existing REL principal projection emits its safe Entity ID |
| Project validation/checkpoint/upload | External Git/filesystem work precedes admission; synchronized correlation changes emit ProjectStateChanged, registered file changes emit ArchiveChanged |
| Vector profiles/backfill | Profile acceptance and actual committed vector rows invalidate retrieval; embedding and profile probing precede the gate |
| Insomnia | Claim, application preparation, durable plan freeze, verified REL completion and failure/retry status use the worker adapter; exact private writes and model calls remain outside the REL gate |
| Dream and Perception | Checked post-inference owner commits publish actual semantic/retrieval and safe processing changes; stale checked commits produce nothing |
| Processing switch | Explicit ephemeral boolean invalidation and snapshot field; no invented durable version |
| Host reconnect | Connection/reconciliation finishes before safe capability-ID invalidation of each live mounted REL; arbitrary adapter errors are excluded |
| Lifecycle and maintenance | Existing quiesce/close and reopen epoch reset invalidate subscriptions; incremental continuity is not claimed across maintenance |
| PHY-only writes | No REL event; private worker processing remains separately owned |
| P3 conversation transitions | Explicit P4.7 convergence classification; no guessed branch/generation semantics |

`runtime_host_gateway_mutation_events.rs` owns classification and post-sync publication. `runtime_host_rel_execution.rs` shares its mounted sequencer with concrete worker modules. Public ambient delegates call explicit mutators once. No facade-wide broadcasting or semantic-store subscriber handles are introduced.

Non-conversation publication preserves the journal's last observed durable watermark without taking its lock under the semantic owner; fresh P3 transition capture remains P4.7.

Each changed canonical class receives one summary per committed transition, sharing that transition's synchronized source watermarks. A transition may change more than one class. IDs are typed and body-free. Cursor allocation remains notification ordering, independent of the frozen source-clock contract.

A rejected mutation with no source change produces no summary. Synchronization failure or a detectable partial authoritative change invalidates distribution and clears recipients instead of publishing success. Normal quiesce/reopen supplies a fresh epoch and snapshot. A delivery error cannot negate an already synchronized owner result.

## Coverage enforcement

`scripts/check_multiplexing_phase4_coverage.py --self-test` classifies every public GatewayRuntime API, including reads, attachment controls, private operations, epoch invalidation and P3 convergence. New public APIs fail closed until classified. Published methods must retain canonical admission; classified reads reject recognized authoritative writes. Concrete REL worker owner-write calls must remain inside the shared mutation closure.

The standard architecture runner invokes this guard before Lexicon/Arcana refresh. Seven negative/positive fixtures exercise unclassified APIs, missing admission, read/write misclassification, string masking and worker bypass. This is conservative source enforcement, supplemented by behavioral tests, rather than a proof of arbitrary indirect Rust calls.

## Verification

Collected on 2026-10-05 against the final P4.4 source:

- Library formatting and locked offline check passed.
- Serial library tests: 944 passed, zero failures; this includes all 12 new P4.4 publication tests.
- Serial integration verification: all 11 integration targets passed, 93 tests total; the library suite also passed again as part of this invocation.
- P4.4 coverage guard: 104 public gateway APIs classified, concrete REL worker writes admitted, all seven negative/positive fixtures passed.
- Phase 2 source ownership enforcement passed with no gateway violations; legacy consumer inventory remains P5.
- Both structural and changed-from-origin/main documentation checks passed, as did Git whitespace validation.
- CLI formatting passed. Locked offline CLI check stopped in Lore's build script because its cached regex rlib was absent; CLI tests were not reached.
- Unrestricted cargo test was attempted and failed during example/integration compilation with missing library/rlib artifacts and Rust internal compiler errors. The serial library/integration gates above subsequently passed; unrestricted examples remain a separate build gap.
- Architecture refresh passed the direct coverage guard, then stalled during Lexicon Rust/Python analysis. The refresh was stopped to release the worktree; refreshed Arcana/Pitlord enforcement remains incomplete.

P4.4 implementation and targeted acceptance are complete. The CLI build, unrestricted examples and refreshed architecture evidence are explicit verification gaps, not passing gates. P4.5 is next.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, limitations, roadmap, behavioral contracts, P4 inventory/plan, G3/G4 and the frozen P34 seam.
- Updated: this record, plan/index, architecture, behavioral contracts, maintainer map, limitations and roadmap.
- Not affected: semantic storage format, P3 journal/coordinator policy, frozen observation types, public API exports, CLI and provider configuration.
- Compliance check: final structural and changed-from-origin/main documentation gates below.
- Known documentation gaps: public consumer exports, final conversation events and stream wiring remain later P4 steps.

## Architecture impact

- Standards added or changed: none.
- Ownership or boundary impact: gateway and concrete REL worker post-sync publication share one mounted sequencer; source stores, private workers and P3 keep their existing authority.
- Pitlord enforcement impact: direct coverage guard added to the architecture runner before evidence refresh; refreshed enforcement result recorded below.
- Other verification impact: publication/privacy/race tests and existing gateway, publication recovery, lifecycle and source ownership regressions.
- Known architectural gaps: P4.5 reconnect policy, P4.6 stream hub and P4.7 P3 integration remain separate.

## Related docs

- [P4 snapshot and resync record](multiplexing-phase4-snapshot-2026-10-04.md)
- [P4 baseline inventory](multiplexing-phase4-baseline-2026-10-04.md)
- [Shared observation contract](multiplexing-p34-observation-contract-2026-10-04.md)
- [Architecture](architecture.md)

## Notes

Host reconnect is an invalidation after reconciliation, including repeated reconnects; it is not a claim that every capability changed. Worker lease renewals with unchanged safe status do not create a separate status event. Owner-operation Held/Observed repair bookkeeping remains internal and does not expose private publication payloads in the broad feed.
