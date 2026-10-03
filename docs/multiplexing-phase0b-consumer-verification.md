# Multiplexing Phase 0B G6 — source-backed consumer and verification baseline

Parent index: [Documentation index](INDEX.md)  
Owning design: [Lifecycle and consumers](multiplexing-phase0b-lifecycle-consumers.md)  
Readiness: [Multiplexing readiness review](multiplexing-readiness-review.md)

## Purpose

Freeze the *inspected* downstream consumers, stage capabilities, repository/dependency revision boundaries, reproducible current-code checks, and explicit later migration/release tests. Do not confuse source inventory or standalone test oracles with implementation of GatewayRuntime or verified production interoperability.

## Overview

The inspected primary consumer is the local Warlock v2 Tauri application. It embeds Reliquary in-process and owns provider routing, credentials and cooperative cancellation of active inference. Reliquary owns Archive and durable stream semantics; Warlock has **no demonstrated durable pending chat queue** or stable pending-cancel/dequeue/status service. Current Warlock thus does not qualify as the authority for native provisional work under G1. A future external host may qualify only after an adapter passes the full cancellation and reconnect contract.

**Status, 2026-10-03:** G6's **Phase 0B consumer/design and existing-pinned-version compile baseline are complete**. Existing Reliquary full-test exceptions remain recorded and the future combined upgraded Warlock/GatewayRuntime release gate is not claimed green.

G6 has **two separate completion criteria**. Phase 0B design completion requires a verified source and capability inventory, executable drift checks, recorded *actual* reproducible baseline results, an explicit list of known baseline failures, and target-interface migration tests. Phase 5/6 runtime/release completion additionally requires successfully compiling and testing migrated Warlock, CLI if affected, provider handoff and concurrent-instance behavior. The latter cannot be honestly certified by an unchanged, documentation-only Phase 0B branch.

## Current pinned dependency baseline

| Checkout / dependency | Verified fact | Evidence |
| --- | --- | --- |
| Warlock v2 local main, no source edits for this audit | Embeds `ReliquaryRuntimeHost` in `WorkspaceService`, mounts `InteractionRuntime`, and stores one active REL/PHY selection | `src-tauri/src/workspace.rs`, `workspace_open.rs`, `workspace_session.rs` |
| Warlock → Reliquary | Git revision `83b3ac87a80a4221039fa07d9722f5f8076464ba` | Warlock `src-tauri/Cargo.toml`; locally inspected committed Reliquary revision's manifest |
| Warlock → Arcana, directly and through pinned Reliquary | **Both** `66b4e96ecf18cc79dfd6b1ea30232bef7c86bba3` | Both pinned manifests and actual `cargo tree --locked --offline -i arcana`, which displays one Arcana with two inbound references |
| Phase 0B Reliquary baseline | `a6f580c` pins Arcana `9ebccd6e7d089b98a8992c5451ba57f903f295fb` | This worktree `Cargo.toml` |
| Future Warlock integration | Advance Reliquary and direct Arcana **together**, regenerate Warlock lockfile and verify only one intended Arcana package ID | An explicit Phase 5 migration gate, **not** an existing duplicate-crate defect |

An independent source drift check is `python scripts/verify_multiplexing_phase0b_consumers.py`, run from this Reliquary worktree with the sibling `Warlock-v2` checkout or an explicit `--warlock` path. In addition to its critical API landmarks, it freezes the **complete inspected direct-import footprint: 58 Warlock Rust files** using `reliquary_memory::` (81 import occurrences in the source audit). This gives Phase 5 a concrete, fail-on-drift inventory of *all observed direct import sites*, rather than only the central host files. It also verifies both pinned manifests and exact shared Arcana revision. It must fail if the source checkout is unavailable, a pin diverges, known integration methods disappear, or the direct-import footprint changes without deliberate manifest revision. `python -m unittest discover -s tests -p test_multiplexing_phase0b_consumer_audit.py -v` supplies a positive baseline and four negative drift tests. The script is **not** a substitute for compilation or semantics tests. When Phase 5 removes the ambient host interfaces, replace this frozen inventory with a revised final-interface inventory; do not preserve old APIs merely to keep the baseline script green.

## Verified caller and authority map

| Consumer paths | Inspected present-day behavior | Final migration contract |
| --- | --- | --- |
| `workspace.rs`, `workspace_open.rs`, `workspace_session.rs`, `workspace_set.rs` | One `ReliquaryRuntimeHost` in app workspace; lower-level `InteractionRuntime::new(cva)` mount; local session restore | One GatewayRuntime ownership boundary for each mounted owner, instance attachment for each independently operating context, reconnect/reauthorization and quiescence before mount replacement |
| `conversation.rs`, `conversation_stream.rs` | Host-global `start_conversation_session`, `reopen_conversation_session`, `require_active_session`, writable stream operations | Instance-selected conversation/branch and gateway-shared conversation coordinator, stable operation ID, many viewers but one writer; no legacy host-global selector facade |
| `chat_commands.rs`, `chat_stream.rs` | Provider inference lease then immediate `send_user_message_with_attachments`; Tauri `Channel` events follow Reliquary durable checkpoint | Gateway provisional admission/finalization **before** canonical Archive turn, subscriber-checked stream checkpoint/cursor delivery and cancel/status behavior |
| `inference_lifecycle.rs`, `provider*.rs`, `reliquary_routes.rs`, `reliquary_general.rs`, `reliquary_embedding.rs` | Warlock controls provider credentials/routes and cooperatively interrupts running calls, not serialized durable pending cancellation | Warlock keeps actual provider execution and running-call interruption; Reliquary coordinates durable operation and pending queue, using capability-checked adapter calls |
| `workspace_compaction.rs`, `workspace_echo.rs`, `chat_echo.rs`, `workspace_memory_provenance_view.rs`, project/reconcile helpers | Direct and ambient host/owner reads and maintenance calls | Explicit authorized instance/owner context, scoped private PHY reads, owner fencing and reauthorization rather than ambient global mutable selection |
| `tests/*` in Warlock Rust and Tauri command callers | Existing in-process single-workspace assumption | Migrate fixtures to final gateway and independent instances; retain frontend command shapes only where semantics permit |

A bounded source audit of the available `hermes-agent` checkout found no `reliquary_memory`, `reliquary-memory`, `ReliquaryRuntimeHost` or `InteractionRuntime` direct Rust/Cargo references in the inspected scope. The current Reliquary `cli/src` directly uses `ConfiguredRuntime` for Insomnia and vectors but contains no direct `ReliquaryRuntimeHost` or `InteractionRuntime` reference. No independently maintained ACP adapter source checkout was identified in the inspected local roots. These are scoped observations: **unknown external consumers remain an integration boundary**, not evidence that every framework has been audited.

## Authority and handoff matrix

| Stage | Current Warlock integration authority | Required invariant |
| --- | --- | --- |
| Native pending submission / cancel-before-dequeue | **Reliquary gateway** (target; current Warlock provides no qualifying host queue) | Durable ack only after journal sync; one REL-wide writer; pending cancellation wins before dispatch or returns the honest already-running result |
| Provider execution and in-flight interruption | Warlock | One stable operation handoff; cancellation is cooperative once provider execution starts and cannot promise reversal of external effects |
| Archive finalization, activity, Episodes and checkpoints | Reliquary | Exactly one durable turn/operation identity; checkpoint-before-stream-delta; idempotent post-commit schedule |
| REL updates, active conversation streams, grant/reconnect checks | Reliquary GatewayRuntime (target) | All authorized REL viewers receive relevant owner-status updates; only selected authorized conversation viewers receive text; atomic snapshot/watermark after reconnect |
| Warlock UI/lifecycle/credential presentation | Warlock | No independent semantic owner or ambient shared-session authority |
| Future host-provided upstream durable queue | Only a separately **proven** host adapter | Durable stable ID and status query, cancellation serialized with dequeue, idempotent handoff, startup reconnect and cancellation reconciliation before dispatch; otherwise use Reliquary native queue |

Warlock `InferenceLifecycle::quiesce()` can request cancellation and drain active provider leases; it is **not** a pending submission queue and does not satisfy the stronger G1 cancellation guarantee.

## Baseline results and scope

The preceding Phase 0B record documents a previous `cargo check --locked` success, one actual `multiplexing_baseline_contract` pass, six Rust-std-only Phase 0B reference fixtures with **62 passes**, and successful formatting/documentation checks. That baseline does *not* establish migrated runtime behavior.

A previous full `cargo test --locked` compiled existing examples and failed on the `archive_open_profile` allocator collision and `insomnia_stress` crate lookup; focused library testing previously reached **767/768**, with one Lore historical-read Windows file-sharing error 32. One `runtime_host_` filtered attempt reached 43/44 due to a timing-sensitive backpressure assertion also known to pass in the broader run. Preserve these as **known pre-migration baseline exceptions**; neither silence them nor attribute them to the documentation-only Phase 0B change. Report fresh results separately. Full integration cannot be marked green until those issues are resolved or a narrowly justified, independently repeatable test isolation/exclusion is accepted without masking new failures.

Verified during this G6 audit on 2026-10-03:

- Warlock `cargo tree --locked --offline -i arcana`: **passed**, exactly one Arcana revision reached through both Warlock and pinned Reliquary.
- Warlock `npm run build`: **passed**, TypeScript and Vite production build.
- Source-bound consumer audit: **passed**, current caller/pin manifest verified.
- Positive/negative drift-audit tests: **5 passed** (current source match, pin mismatch rejection, disappeared host API rejection, missing checkout rejection and a new untracked direct-import consumer rejection). Full inspected footprint: **58 files**.
- Reliquary `cargo test --locked --lib -- --test-threads=1` rerun in this isolated Phase 0B worktree: **timed out after 420 seconds during dependency compilation**, having reported `lore-revision`, `lore-notification`, `lore` and the Reliquary package; it produced no test results. The previous 767/768 library baseline remains historical evidence, not a new passing result.
- Warlock `cargo check --offline --locked -j 1` initially **failed in third-party `icu_provider v2.3.1`** on unavailable dependency `.rmeta` paths (`zerotrie`, `zerovec`, `writeable`, `yoke`, `icu_locale_core`, `zerofrom`), before any Warlock source error. Metadata files existed under other hashes in the old cache. An isolated targeted `cargo clean --offline -p` for those seven dependencies succeeded. The subsequent serialized check with `CARGO_INCREMENTAL=0` passed the original failure but exhausted its five-minute build window while compiling the remaining Tauri/Lore dependencies.
- **Successful Rust baseline:** the next warm-cache `cargo check --offline --locked -j 1`, still with `CARGO_INCREMENTAL=0`, **passed for the current pinned Warlock** after 3m49s. There were 15 non-blocking Warlock dead-code warnings. No Warlock application source edits, dependency pin changes, compatibility stubs or whole-target clean were required. This proves the current pinned *check* baseline only; migrated-version compilation and Rust unit/integration tests are independent future gates.

The source audit is intentionally separate from a new-version Warlock compile: Warlock currently pins a different Reliquary revision. Phase 5 must advance both pins before claiming compatibility with a new gateway API.

## Ordered implementation/release gates

1. **Phase 1A authority and 1B coordinator:** on the **final target API**, first add failing tests for two instances attaching to one REL/conversation and generation, independent selection, one writer, cancellation/finalization race, full-REL journal compaction, source attribution, and stable recovery. Keep existing old-host characterization independent until the hard cut; no production method is proven by a standalone model oracle.
2. **Phase 1C/2 routing:** migrate selected REL/PHY context and explicit principal routing; enforce negative grant/PHY tests against actual storage and mixed-principal Insomnia.
3. **Phase 3/4 event coverage:** exercise real mutations (inactive conversations, Dream/Insomnia/vector workers, queue state and checkpoint-only events), authorized REL broadcast, subscriber-only text, disconnect/remount/revocation, epoch and watermark recovery.
4. **Phase 5 downstream hard cut:** update Warlock pinned Reliquary **and** direct Arcana in one reviewable change, ensure `cargo tree -i arcana` resolves one intended package, replace direct host-global selector and raw-host routing with GatewayRuntime/InstanceRuntime, preserve Warlock provider cancellation and Tauri event boundaries, and update source drift manifest to target interfaces. Use a fake host adapter to prove unknown status, cancel/dequeue serialization, handoff-crash retries and capability downgrade; no real Warlock pending queue may be assumed.
5. **Phase 6 release:** clean build `cargo fmt/check/test --locked`, relevant detached CLI locked checks/tests, Warlock `cargo check/test --locked`, Warlock `npm run build`, documentation checks and architecture policy, actual multi-instance/host recovery/race tests. Explicitly account for each pre-existing red exception and any new regression; completion requires real integration evidence rather than oracle-only success.

## Related docs

- [G1 conversation journal and shared execution](multiplexing-phase0b-conversation.md)
- [G3/G4 events and authorization](multiplexing-phase0b-events-authorization.md)
- [G5 lifecycle and G6 consumer summary](multiplexing-phase0b-lifecycle-consumers.md)
- [Integrated Phase 0B handoff](multiplexing-phase0b-design.md)
- [Phase 0 execution baseline](multiplexing-phase0-baseline.md)

## Notes

This file records the complete G6 **design and current-pinned consumer compile baseline**, not a shipped GatewayRuntime, an external ACP implementation, migrated-revision compile conformance or a claim of green whole-stack release tests. The current Reliquary full-test exceptions and unavailable external source trees remain explicit limitations.
