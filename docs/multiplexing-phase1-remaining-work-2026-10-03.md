# Multiplexing Phase 1 — remaining implementation and acceptance

Parent: [Multiplexing implementation plan](multiplexing-implementation-plan.md)
Authoritative design: [Phase 0B integration](multiplexing-phase0b-design.md) and [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
Status: **historical completion plan; remaining execution superseded by the [Phase 1 finish plan](multiplexing-phase1-finish-plan-2026-10-04.md)**
Audit snapshot: 2026-10-03; branch `feat/multiplexing-phase1`, starting at `2b7dc5c` plus uncommitted implementation in the isolated worktree

## Purpose

Finish the **production Phase 1 gateway/instance authority foundation** already under development without restarting Phase 0B, redesigning the journal, absorbing parallel Freshness/staleness work, or prematurely attempting Phases 2–5. This is a sequenced implementation handoff: each work item has an owner boundary, concrete changes, focused proof, and a stop condition.

## Overview

Phase 0B G1–G6 is complete **as a design and executable-reference package** and is already on `main`. The current isolated Phase 1 worktree contains real `GatewayRuntime`, `InstanceRuntime`, REL-scoped `RelJournal`, shared conversation coordination, capability negotiation, gateway-authorized ambient/project APIs, instance management, and lifecycle/fencing methods. These are source implementations, not evidence of a completed migration.

Verification evidence must remain chronological:
- Previously, 27 integrated targeted tests passed: 9 gateway, 11 journal, and 7 instance; five host tests also passed in an independent warnings-denied `rustc` run. This covers the **earlier source snapshot**, not every subsequently added test.
- On the latest 2026-10-03 source snapshot, `cargo check --locked -j 2` **passed with six warnings**. `cargo fmt --all -- --check` **failed** on newly added gateway ambient/project code. The current source contains additional gateway, ambient, conversation, and journal tests that have **not** all been rerun as one verified set.
- The original worktree has uncommitted work. Do not equate a clean compiler result with Phase 1 acceptance, and do not commit or reset parallel work from `C:\!bin\workspace\Reliquary`.
- There is no current `.arcana/CURRENT` snapshot in this Phase 1 worktree; architecture-policy validation against a fresh structural index is outstanding.

**Important scope boundary:** Phase 1 must establish a usable, single-authority **new gateway path** with independent instance selection and shared conversation execution. The older `ReliquaryRuntimeHost` still has global active-REL/managed-session state for unmigrated callers. It must not operate as a synchronized mirror or second authority over gateway-mounted owners. Its remaining downstream/public consumers and final deletion are **Phase 5**. Phase 1 may not claim its stronger "one selection source" acceptance gate by merely introducing an additional parallel gateway API. Finish and test the handoff/isolation boundary now; record the final old-host deletion gate under Phase 5.

## Continuation checkpoint (2026-10-03, resumed 2026-10-04 UTC)

P1.0 formatting/check passes and initial dependency-cache/test-build blockers are recorded in the master plan. Earlier asynchronous retry job records were unavailable after interruption; no test result is inferred from them. P1.1 now has source implementations for read-only path/header admission, OS physical-file identity, same-file reuse, conflicting UUID-copy rejection before mutable recovery, process-local exclusive gateway execution claims, configured credential verification, exact owner-kind grants and authenticated grant refresh. The raw value-based attachment/regrant and preopened gateway mount APIs are no longer public request seams. All five focused admission tests passed in the resumed gateway run (25 passed, two unrelated-to-admission gateway failures). The two gateway failures exposed Windows long-path journal staging and double operation-gate advancement on grant revocation; repairs and broader final-source P1.0 rebaseline are pending.

The current host integration uses the existing REL journal for stable handoff identity, payload digest, remote stage status and sticky cancellation intent. Host payloads use a bounded binary frame rather than JSON attachment byte arrays. ConversationExecution completion gates Archive finalization of the original admitted user turn; no host assistant-result contract is claimed. Controlled adapter tests and final-source verification are pending, and other host worker capabilities remain open under P1.2.

Mixed legacy-host/gateway exclusion now has source implementations and tests through one shared process claim. Legacy startup is fallible, dynamic PHY attachment participates, and claims retain canonical path plus physical identity. Final-source execution is pending; the remaining P1.4 routing/selection acceptance is still open. The claim must not be described as direct-client or cross-process locking. P1.1/P1.7 acceptance is not yet claimed; P1.2–P1.6 remain as specified below.

## Source-backed implementation inventory

| Responsibility | Present code / evidence | Completion gap |
| --- | --- | --- |
| Gateway and instance authority | `runtime_host_gateway.rs`, `runtime_host_instances.rs`; mounted REL/PHY registries, grants, epochs, short leases, attach/detach and per-instance managed state | Read-only typed path admission, OS file identity, alias reuse, UUID-copy rejection and configured credential admission have five passing focused tests; mixed legacy exclusion and broader lifecycle races remain unverified |
| Host capability boundary | `runtime_host_gateway_host.rs`; connect/reconcile, configured capability states and declared `handoff`/`cancel`/`status` adapter methods | ConversationExecution handoff/cancel/status and durable operation records are being integrated with controlled adapter tests; final-source verification is pending. Other worker capability integrations and real Warlock conformance remain open |
| Shared conversation | `runtime_host_gateway_conversation.rs` and `multiplexing_journal.rs`; REL-wide durable journal, attachment staging, cancel/finalize, recovery, viewer tracking, compaction | Verify newer failure/GC tests; prove detach-independent accepted-work continuation, all crash windows, and single writer/lock ordering in real gateway paths |
| Instance ambient operations | `runtime_host_gateway_ambient*.rs` plus new extended/project modules | Format and exercise newly added operations; close owner-grant, source-provenance, dependency, revoke-during-I/O and disk-sync gaps |
| Legacy authority | `runtime_host.rs`, `runtime_host_session.rs`, `runtime_host_topology.rs` and legacy semantic modules still retain active selection and managed sessions | Define/test isolation while Phase 1 is implemented; migrate Phase 1's in-process gateway entry routes without simultaneously synchronizing legacy state; final caller migration/deletion is Phase 5 |

## Ordered remaining work

### P1.0 — Normalize and freeze the current implementation baseline

**Prerequisite for every other item.** Stay in `C:\!bin\workspace\reliquary-multiplexing-phase1`. First inspect `git status` and any concurrent jobs; do not overwrite existing uncommitted implementation. Run `cargo fmt --all` on this worktree only and confirm `cargo fmt --all -- --check`. Run `git diff --check` and `cargo check --locked -j 2`. Keep Cargo jobs serial and bounded; if the cold build is costly, reuse the same resolved target output rather than triggering parallel dependency rebuilds. Do not manufacture a green full-test baseline by excluding unrelated failures silently.

Rerun **all currently defined Phase 1 tests**, not just the prior 27: `cargo test --locked --lib gateway:: -j 2 -- --test-threads=2`, then the journal and instance test filters with the same build artifacts. Account for the newer gateway lifecycle, revocation, ambient authorization, conversation recovery, bounded attachment and cross-REL spool-GC tests. If compilation or tests fail, repair at the responsible production seam or correct an invalid fixture, rerun, and append exact command/result to the master plan.

**Exit:** formatting, check, all focused Phase 1 tests and a written baseline matrix pass or have individually recorded blockers; no undocumented source truncation, test-only assertions of implementation behavior, or untracked failing tests.

### P1.1 — Close gateway identity and attachment admission (1.1 / 1A)

Keep **one** canonical gateway-owned REL/PHY registry. Review the current `mount_rel(InteractionRuntime)` and `mount_phy(Phylactery)` APIs against Phase 0B G5: owner UUIDs alone reject duplicate semantic owners but cannot prove that a second path is an alias versus a conflicting copy. Select the narrowest real gateway entry seam that can retain the validated on-disk kind, canonical file key and available OS physical identity **before** constructing a second mutable owner. Same-file aliases must reuse one execution; same UUID with different physical identity must fail closed. Filesystems without stable physical identity must be explicitly path-scoped, not falsely described as hard-link safe. Do **not** claim cross-process exclusion or add an unplanned generalized storage layer.

Define a trust boundary for `attach_instance`: an authenticated, gateway-verified principal and its permitted PHY/grants must be supplied through a trusted constructor/adapter; untrusted clients may not pass arbitrary principal IDs or choose another PHY. Validate exact owner-kind grants before constructing the instance, and reject an unmounted/unauthorized bound PHY. Existing `InstanceRuntime` grant-generation and incarnation guards remain the canonical mutable context.

**Proof:** same alias mounted twice yields one execution, conflicting owner copy fails, wrong kind/invalid owner ID fails, two instances attach to the same mounted owner without a clone, forged principal/PHY/grants fail at the trust boundary, independent RELs continue operating. If the public API must change to accomplish this, update its immediate compilation consumers and mark unrelated consumers for Phase 5.

**Exit:** a real gateway mount and attachment cannot create two mutable authorities or accept caller-invented identity/grants in its supported in-process deployment.

### P1.2 — Complete the per-capability host operation boundary (1.1)

Retain the existing `CapabilityRouter`; **do not create a second queue or alternate capability registry**. Wire the adapter's declared `handoff`, `cancel` and `status` into the gateway's actual operation path with the Phase 0B G1/G5 host state machine:
1. Startup attempts connection immediately. Recover local native pending work independently, but fence each configured host-dependent stage until its contract and prior status/cancellation have been reconciled.
2. For a host-executed stage, only transfer **pending queue ownership** when durable acceptance, stable IDs, idempotent handoff, linearizable pending cancel/dequeue and authoritative status/reconciliation are demonstrated. An incomplete or nonconforming host leaves the native queue authoritative where safe.
3. Persist a native handoff **intent** with stable ID/payload digest and cancellation state before the external call. Exact matching durable host receipt determines transfer. Unknown outcome is **held**; no blind native replay, duplicate host dispatch, or duplicate Archive finalization.
4. Route a cancellation to the actual current stage owner, including cancellation before sender acknowledgment and disconnect during handoff. Report pending-cancel versus already-running cooperative interruption truthfully. A contract downgrade cannot strand or silently replay outstanding host-owned work.
5. Never hold REL/PHY locks or a long commit lease during provider/network calls. Validate authority immediately before cross-boundary dispatch and before terminal publication. Unrelated native capabilities remain available when a configured host capability fails.

For **Phase 1**, prove this with a controlled adapter implementing the declared contract. **Warlock's real host conformance and migration remain Phase 5**; its currently audited cancellation handle is not a proven durable upstream queue. Do not advertise host queue ownership from adapter declarations alone.

**Proof:** mock adapter tests for startup failure/reconnect, capability downgrade, handoff crash before/after durable receipt, identical retry versus digest conflict, unknown remote status, cancellation racing remote dequeue, cancellation during disconnect, and native-only independent progress.

**Exit:** the gateway actually calls and reconciles the declared host operations, with one durable stage authority at every tested transition; a handshake-only test is insufficient.

### P1.3 — Finish the single shared conversation/journal integration (1.2 / 1B)

Keep the existing REL journal as the **only** writer, sequence allocator, snapshot owner and compactor across all conversations in that REL. Do not instantiate a per-conversation journal, acquire a semantic owner lock while holding the journal writer, or retain duplicate mutable writable sessions. A viewer can attach/detach independently; only authorized writer admission can own the shared `InteractionRuntime` continuation.

Finish and exercise: admission validation **before creating spool artifacts**, 16 MiB journal frames and the specified 64 MiB / 64-item / 256 MiB attachment limits, one bounded REL-wide compaction and spool-GC policy, composite conversation/submission IDs, persisted equal-timestamp tie keys, deterministic replay, cancellation/commit winner, stable Archive identity, and exact post-commit scheduling receipts. Attachment GC must not delete content still used by another submission **or another REL**. If an accepted native item must outlive its submitting instance, give internal processing its own persisted work identity/capability so detach does not abandon admitted work; no unrelated viewer gains cancel authority. Full user-facing sequential/visible-branch policy remains Phase 3.

**Proof:** independently mounted two-conversation and two-REL tests; multiple writers/viewers; detach/regrant mid-operation; restart from every journal/Archive transition, including already committed Archive plus incomplete journal receipt; incomplete final frame versus complete corruption; failed sync/no acknowledged enqueue; spool loss/hash mismatch; shared-blob compaction and GC; cancel/finalize racing across separate conversations; no orphaned Archive activity from queued/cancelled work.

**Exit:** real gateway state machine and tests enforce the 0B journal/coordination invariants without a duplicate continuation or a second writer; pending accepted work and terminal tombstones survive a fresh gateway.

### P1.4 — Complete instance-local selection and the Phase 1 routing boundary (1.2a / 1C)

Only after P1.3's shared continuation acceptance, finish moving **new-gateway** active REL, per-REL managed conversation selection and reopen-pending state exclusively into `InstanceRuntime`. A view-only selection must not create a writable session or alter another instance's selection. Changing a selected REL must not close or alter another instance's shared continuation; one instance detaching must not mark an otherwise-viewed conversation globally inactive.

Inventory **new gateway/public entry paths** against their legacy host equivalents: conversation read/search, Echo and external provenance, source files, project and Archive operations, knowledge/memory/relation/Graph access, memory retrieval/embedding, dependency lookup, status, management, worker toggles, quiescence and reconciled reopen. Each user-initiated gateway path must take the initiating instance and validate explicit owner grants, including dependency/source owners. Keep maintenance's explicit owner APIs internal; never infer an initiating user from a host-global active execution.

**Legacy boundary:** existing `ReliquaryRuntimeHost` callers may be preserved only as **isolated, old deployment paths** until Phase 5. No mixed old-host/new-gateway authority over one live owner; no state synchronization, façade or fallback to legacy active selection from a gateway method. If a required Phase 1 entry point can still reach `active_execution()`/`active_rel_id()` on that gateway's behalf, its migration is unfinished.

**Proof:** two instances select different RELs and conversations; same REL with different selected branches; reader/writer split; switch away and reopen preserving instance selection; only authorized REL dependencies/source provenance exposed; old-host/new-gateway mixed ownership is rejected.

**Exit:** the canonical Phase 1 in-process gateway path has one instance-local selection source. Global deletion of the remaining legacy host call surface is explicitly deferred to Phase 5 and must not be misreported as complete now.

### P1.5 — Prove authorization for expanded ambient/project APIs (1.3)

Format and test the **new** `runtime_host_gateway_ambient_extended.rs` and `runtime_host_gateway_ambient_project.rs` alongside the existing ambient module. Add focused tests for every newly introduced class of operation, prioritizing grants for exact owners, ungranted dependency closure, foreign-memory provenance, Echo source identity, PHY/private boundary, project filesystem side effects, and revoke/detach during long external embedding or repository work. A grant check before a network/filesystem step is not enough: acquire and recheck the **short counted mutation/delivery lease** at the actual resulting store commit or delivery; persist and sync committed mutations through the existing canonical owner API.

Audit newly added paging semantics: paging over an intentionally bounded search result must be described as that bounded window, not all Archive results. Fail closed on invalid offsets/limits and prevent enumeration of ungranted REL identity. No REL lock may span embedding, Git/filesystem work, callbacks or host I/O.

**Proof:** same-owner allow and cross-owner deny for each operation family, grant replacement during external I/O, read-only versus manage grants, exact source-REL authorization and atomic canonical mutation publication; no leaked private data after revocation.

**Exit:** each Phase 1 instance-facing semantic/project API has an actual grant/revocation test through the new gateway boundary; unsupported PHY-private APIs stay explicitly limited until Phase 2.

### P1.6 — Close lifecycle, quiescence and fence correctness (1.4)

Use the gateway's existing owner/attachment `LeaseGate`, generation and mount-epoch machinery as **one** authority gate. Verify that `replace_instance_grants` and detach close admission, drain already-issued short leases, increment authorization generations and clear only the affected owner's outbound/viewer registrations. Adding an unrelated grant must not needlessly invalidate an in-flight operation's owner-specific generation. At an owner boundary, reject any operation carrying a stale instance incarnation, owner epoch or fence generation.

Implement/prove the 0B quiescence sequence: stop new admission and pending dispatch, preserve durable accepted pending submissions, drain bounded leases, stop/fence workers, reconcile durable journal and Archive state, reopen, advance mount epoch and restart the correct workers. A failure leaves the affected owner closed/held, **not** silently reopened. No long inference under a REL/PHY lock; no nested journal/semantic owner or REL/PHY locks. A separate REL must continue progressing during another REL's quiescence.

**Proof:** revocation during a bounded commit and during provider I/O; stale token after quiesce/reopen and unmount/remount; shutdown repeatability; owner maintenance failure; restart with pending/finalizing journal entries; two instances sharing a REL while only one detaches; lock-order stress without fixture-only simulation. Test path aliases from P1.1 as part of owner replacement.

**Exit:** neither an old token nor a detached instance can commit/deliver after its drain completes, and no quiescence/restart path loses accepted work or stops an unrelated owner.

### P1.7 — Finalize Phase 1 acceptance, document and hand off

Run these checks against the **final** source, recording exact commands/results in the master plan:
- `cargo fmt --all -- --check` and `git diff --check`; `cargo check --locked -j 2`.
- All gateway, instance and journal tests plus the existing `multiplexing_baseline_contract`; add the new P1.1–P1.6 adversarial tests. Run only one heavyweight Cargo build per checkout at a time and reuse the same warmed outputs.
- Existing bounded runtime-host behavioral regression cases and the broad library/integration baseline. Record inherited unrelated Lore file-share/flaky-provider and existing example failures separately from new regressions; **do not claim a green full suite** if those remain red.
- Public API compatibility check for any library signature changed in Phase 1; build the detachable CLI when these boundaries change. The existing pinned Warlock source baseline is a **Phase 0B datum**, not a successful build against the new Phase 1 library or proof of Warlock migration.
- Structural docs check and changed-from check: `python ../engineering-standards/tools/docs_policy/check.py --repo .` and `--changed-from origin/main`. Refresh Lexicon/Arcana evidence on the isolated checkout only when available and bounded, then run `scripts/check_architecture.py`. If the structural scan cannot complete, report that exact independent gate rather than implying success.
- Review current `docs/architecture.md`, `api.md`, `invariants.md`, `storage-format.md`, `current-limitations.md`, `behavioral-contracts.md` and this plan: distinguish **implemented gateway API** from **supported whole-stack deployment**. Record remaining Phase 2/3/4/5 work under its actual owner.

Before packaging, audit all old/global authority usages reachable from the new gateway (source search plus structural evidence if current) and document any remaining external legacy callers for Phase 5. Package Phase 1 code and test changes in coherent reviewable commits **only after** checking the worktree for concurrent modifications; do not sweep parallel Freshness or staleness work into the branch. Keep `main` untouched unless separately requested.

**Phase 1 exit gate:** one authoritative new gateway execution per mounted owner; trustworthy instance grants and independent selection; one shared continuation and REL-wide durable provisional journal; actual tested per-capability host stage transitions; bounded owner-specific fencing/quiescence; no gateway-facing legacy ambient fallback; focused tests pass and limitations are explicit. This is **not** certification of multi-principal PHY worker publication (Phase 2), full branching (Phase 3), REL feeds/generation streams (Phase 4), Warlock/CLI migration and legacy host deletion (Phase 5), or the complete multi-instance release (Phase 6).

## Work sequencing and handoff

**Serial critical path:** P1.0 → P1.1 → P1.2 → P1.3 → P1.4 → P1.5 → P1.6 → P1.7. P1.2 host adapter and P1.3 journal failure fixtures can be developed in independent file scopes once P1.1's interfaces stabilize, but their cross-boundary acceptance is serial. P1.5 independent tests may be prepared while P1.4 routing finishes; **do not** edit the same gateway files from parallel agents. P1.7 is the only final acceptance decision.

A fresh implementer should first read this file, the master plan's Phase 1 record, the 0B integration document and G1/G4/G5 contract details, then review `git status` before editing. Existing source is the starting point: **repair and finish**, do not replace the working journal/coordinator or redesign G1–G6.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Phase 0B integrated design](multiplexing-phase0b-design.md)
- [Phase 0B conversation/journal contract](multiplexing-phase0b-conversation.md)
- [Phase 0B events and authorization](multiplexing-phase0b-events-authorization.md)
- [Phase 0B lifecycle/consumer contract](multiplexing-phase0b-lifecycle-consumers.md)
- [Phase 0B consumer verification](multiplexing-phase0b-consumer-verification.md)
- [Phase 0 audited baseline](multiplexing-phase0-baseline.md)

## Notes

This is a completion plan, not a claim of tested deployment. It supersedes **only** the master plan's previously underspecified Phase 1 remaining-work detail; it does not expand Phase 1 to absorb later-phase event feeds, personal publication, user-visible branching, actual Warlock migration or entire runtime-host removal. New test results must replace the old verification snapshot only after execution on the corresponding source revision.
