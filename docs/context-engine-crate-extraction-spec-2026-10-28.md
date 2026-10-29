# Context Engine crate extraction — implementation specification

**Status:** Ready for implementation as an existing-code, correction-only extraction (2026-10-28). **Not implemented by this specification.**
**Repository / branch:** `C:\!bin\workspace\Reliquary-crate-first` / `refactor/crate-first-restart`.
**Baseline:** `2b5bd0ea` (Ego deferred); verify HEAD/status immediately before implementation.
**Target:** `crates/reliquary-context-engine/`, package `reliquary-context-engine` (currently comment-only placeholder).
**Unrelated worktree change:** `docs/graph-crate-extraction-evidence-2026-10-26.md` — never modify, stage or discard it.
**Scope:** Move existing deterministic current-session context / compaction policy from `src/context_engine.rs` and `src/context_engine/{model,compaction,compaction_budget,evidence}.rs` into the compiled crate. **No new service, no new context capabilities, no new Ego functionality, no changed file format.**

## Purpose and acceptance

The Root product currently exports a functioning, self-contained Context Engine, while the reserved dedicated crate exports nothing. This is a genuine migration of existing code, unlike the deferred Ego semantic service. Make the new crate the **single authoritative owner of the already-implemented algorithms and types**. Root should continue to export the same public functions/types/constants, but as direct re-exports, not copied implementations.

**Passing an empty crate, adding an unused facade, or moving only the data structures is not completion.** Completion means all current compaction/rendering policy runs in the new crate, the consumer-facing Root API remains unchanged, source tests move with the implementation, product integrations pass, and the extraction is committed.

**Existing baseline witness:** `cargo test --lib context_engine --locked --offline -j 1 -- --test-threads=1` passed **9/9** tests at drafting. The narrower `echo::` test filter matched zero cases and is **not** Echo coverage; use the actual `echo_tests` names when verifying. The previously verified Dream HEAD `4936804b` preceded the docs-only Ego decision `2b5bd0ea`; no baseline inference about all Context Engine integration paths is made here.

## 1. Actual source inventory and ownership crosswalk

| Source now in Root | Destination and obligation |
| --- | --- |
| `src/context_engine.rs` | Replace only its public Root re-export role. Do **not** preserve the original Root implementation modules as an alternate live owner. |
| `src/context_engine/model.rs` | **Move** canonical `ContextRole`, `ContextTurn`, `ContextView`, `ContextMessage`, `ContextRequest`, `EvidenceKind`, `ContextEvidence`, `CompactionCheckpoint`, `CompactionBudget` and `CompactionPlan`. Preserve fields, derives and return/borrow semantics. |
| `src/context_engine/compaction_budget.rs` | **Move** `ContextTokenCounter`, `CompactionError`, budget constants, `compaction_budget`, `compaction_needed`, `budget_satisfied`, `normalize_summary`; no tokenization/provider assumption added. |
| `src/context_engine/compaction.rs` | **Move** `request_context`, `request_messages`, `checkpoint_applies`, `plan_compaction`, `plan_session_compaction`, message/tail selection and exact current compaction instructions; all pure. |
| `src/context_engine/evidence.rs` | **Move** `render_assistant_content`, `render_assistant_content_from_echo`, pure `EchoEvent` → `ContextEvidence` mapping. This requires the existing lower `reliquary-echo` value dependency, not Archive/Container access. |
| `src/context_engine/tests.rs` | **Move** existing 9 pure tests to the crate, adapting imports only. Preserve test cases rather than duplicating the test suite at Root. |
| `src/lib.rs` | Keep **all** existing user-facing `pub use` names and their exact types, constants, signatures and behavior; replace the private Root `context_engine` module with re-exports from `reliquary_context_engine`. |
| `src/echo/echo_tests.rs` | **Remain Root integration:** currently calls public `render_assistant_content_from_echo`, and must still compile against the unchanged Root export. |
| `crates/reliquary-archive/src/conversation_compaction_*.rs` | **Remain Archive:** canonical conversation/branch history, compaction checkpoints, physical records, replay and append. Not an input data cache owned by Context Engine. |
| `src/runtime_inference/runtime_host_*.rs`, other Gateway/owner code | **Remain host:** token-counter choice, model/provider requests, authorized source-branch selection, scheduling, persistence and accepted summaries. Do not invent new internal callers to make extraction seem larger. |

A scoped source search at drafting found `plan_session_compaction`, `plan_compaction`, `request_context` and rendering policy **implemented in Root and exposed publicly**, but no additional direct Root production call sites beyond the exports; one Root Echo regression uses the public rendering function. The specification does **not** claim a deployed/current host invokes the planner. The move must not opportunistically wire up a new runtime path.

## 2. Dependencies and authority

- `reliquary-context-engine` may depend directly on **`reliquary-echo`** for immutable `EchoEvent`/`EchoEventKind` values, as allowed by `docs/architecture/crate-map.json`. Add `reliquary-archive` only if a real existing pure immutable value needs it; currently none does. Do not add optional `reliquary-ego`, `reliquary-freshness`, `reliquary-retrieval` or `reliquary-switchboard` just because the map permits them.
- The Root `reliquary-memory` product depends **downward** on `reliquary-context-engine`; the latter must never import Root, Owner Lifecycle, Runtime/Gateway, History, Import, Maintenance, Container, mutable Archive/Echo stores or provider transports. No back edge or dedicated manager framework.
- Context Engine accepts a **caller-supplied** ordered `ContextView`, optional `CompactionCheckpoint`, explicit `CompactionBudget` and `ContextTokenCounter`. It returns a derived context view/request or plan. It never reads a REL/PHY file, writes a checkpoint, advances an Archive/Ego/Freshness clock, performs model inference, or publishes a summary.
- **Host/Archive** remain authoritative for conversation and branch lineage, message IDs, relevant checkpoint identification, source authorization, token-counter implementation and durability. Present `checkpoint_applies` only tests whether `through_message_id` occurs in the supplied view: do not mistake that local check for independently authenticated branch lineage. Higher host provides a view bound to the authorized lineage.
- **Echo** owns canonical retained assistant evidence and its event kind/sequence. Context Engine only renders event values supplied by the authorized host; it is not a second Echo cache or replay engine.
- **Ego** remains a placeholder semantic crate; no Ego cross-chat, Memory-Web, Personality or new injection policy is implemented. Context Engine's current-session compaction is explicitly *not* Ego's planned cross-chat summary selection.

**No duplication:** keep the canonical `Context*` value types once in the new library. Root aliases/re-exports them; do not create parallel `ContextRole`, `CompactionBudget`, or `ContextTokenCounter` definitions.

## 3. Behavior and outputs that must remain identical

1. **Context message order and visibility:** preserve `ContextView.turns` order, user/assistant roles, message IDs and contents. Without a matching checkpoint, `request_messages` passes the whole view. With one, emit one assistant `[Compacted earlier conversation]` message followed by the exact tail **after** its `through_message_id`. Preserve `COMPACTED_CONTEXT_INSTRUCTIONS` byte-for-byte, including its `live_transcript_read` and `live_transcript_search` guidance.
2. **Compaction trigger and budgets:** default trigger **50%**; target **80% of trigger**; protected raw tail **50% of target**; summary **25% of target**, clamped to 1 and **10,000**; input budget **80%** of total context. Current allowed trigger range **1–95**, zero-context and impossible budgets rejected; percentage calculations, saturation and `>=` trigger / `<=` target hysteresis stay exact. No provider output reservation or Ego 20% budget is introduced.
3. **Tail preservation:** `MIN_RAW_TAIL_TURNS = 1`; preserve latest turn even if individually larger than the raw-tail budget. Select compactable prefix by **measured token counts**, not by fixed turn count. `ContextTokenCounter` is the only counter authority; no fallback tokenizer or guessed cost.
4. **Compaction input:** preserve `[message_id:<id>]` message tags, exact instruction prompt, previous checkpoint prefix and input-token accounting. Advance `through_message_id` only through fully included compactable turns. On insufficient first-message budget, return `InputBudgetUnsatisfiable { message_id }`. On no trigger/insufficient compactable material, return `None` with existing precedence. Keep current failure and error strings.
5. **Checkpoint validity:** stale/unmatched `through_message_id` is ignored, not force-applied. Do not add retrospective search, branch guessing, rewriting, or a new summary generation stage. The caller remains responsible for authenticating branch membership.
6. **Summary postprocessing:** trim whitespace, reject empty, enforce supplied `max_tokens` using `ContextTokenCounter`, and truncate on valid UTF-8 character boundaries by current algorithm. No model call or persistence; preserve precise error messages.
7. **Evidence rendering:** map supplied `EchoEvent` values to the same `EvidenceKind` fields. Drop `EchoEventKind::ToolContextAccepted` from the rendered evidence; ignore `ActivityStarted/Completed/Failed` in displayed content. Preserve user-visible labels, event ordering, optional round/name/correlation fields, exact `[Warlock execution context for this prior assistant turn]` and `[Final assistant response]` framing; no evidence means final text unchanged. Do not add or remove any evidence class in this refactor.
8. **Public API compatibility:** Root exports must continue to accept the same source values and return the same typed errors/results. All existing public constants remain unchanged. Do not rename crate/product packages, introduce a new `ContextEngine` manager type, or change caller-visible source ownership.
9. **Persistence:** Archive compaction formats, Echo records, logical event order and all REL/PHY bytes are untouched; ordinary opens remain read-only. No new compatibility reader or format migration.

## 4. Staged implementation (short, independently checkable slices)

| Gate | Action | Witness |
| --- | --- | --- |
| **C0 — freeze baseline** | Verify HEAD/status, existing Root exports and `EchoEvent` callers; run focused 9-case `context_engine` baseline and the actual Echo test filter. Check exact public signatures and instruction/prompt strings. | Baseline counts and existing unrelated Graph edit recorded; old policy bytes noted. |
| **C1 — move pure model + budget** | Move `model.rs` and `compaction_budget.rs` into crate; export only canonical values/functions. Introduce Root dependency on the new crate, preserve import identities. | `cargo check -p reliquary-context-engine --locked --offline`; compile Root, direct external-consumer call with real values; no duplicate Root model. |
| **C2 — move context and compaction** | Move `compaction.rs` in one bounded source/call-site slice. Preserve exact prompts, IDs, checkpoint semantics, tail and token behavior. | Migrate corresponding tests to new crate; checkpoint+oversized-tail and exhausted-input regression green. |
| **C3 — move Echo rendering** | Move `evidence.rs` with a direct lower `reliquary-echo` dependency; Root re-exports actual implementation. | Echo event ordering/filtering and framing cases pass under crate and Root public regression; Echo Store untouched. |
| **C4 — eliminate Root implementation** | Delete displaced `src/context_engine.rs` and `src/context_engine/**` production copies (including moved tests). Replace `src/lib.rs` internal module declaration/public exports with direct compiled crate exports. | `rg` shows **one** implementation for each public function and type. No Root alias cache; no service back edge. |
| **C5 — acceptance and commit** | Run matrix below on final source; update source maps and implementation evidence. Commit only source, tests and relevant docs/locks, excluding unrelated Graph edit. | Full production move proven by independent tests and Cargo; acceptance evidence records exact outcomes/limitations. |

Avoid a new >300-line module or a giant coordinator. The current source files are already compact; keep them scoped. No large autogenerated interface, generalized context engine framework, architecture ratchet or make-work test expansion.

## 5. Required verification on final code

- **Crate:** `cargo check -p reliquary-context-engine --locked --offline -j 1`; `cargo test -p reliquary-context-engine --locked --offline -j 1`. Include at least one genuinely behavioral external-consumer test of compaction, budget, checkpoint and Echo projection through the compiled crate, not a test of constants/re-exports. Reuse the 9 existing tests; add only boundary-specific coverage.
- **Root/CLI:** Root `cargo test --lib context_engine --locked --offline -j 1 -- --test-threads=1` no longer matches moved unit tests, so **do not mistake 0 tests for a pass**. Run the new crate tests plus the real Root `echo_tests` and any Root public-context/API contract cases, as well as `cargo check --workspace --all-targets --locked --offline -j 1`, `cargo test --lib --locked --offline -j 1 -- --test-threads=8`, `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`, and `cargo fmt --all -- --check`. The filter must match actual tests, not old module names.
- **Fidelity matrix:** compare exact pre/post messages, instructions, role, text, checkpoint cut and rendered Echo output on deterministic fixtures; percentages/limits, saturation and error shape; ensure tool context accepted and activity records remain absent. Preserve public compile signatures and Echo integration.
- **Dependency review:** `cargo tree -p reliquary-context-engine --depth 1 --offline`; inspect crate + Root imports. Expected internal direct dependency **Echo only**, unless a separately justified immutable value import genuinely appears. Root depends on Context Engine, never the reverse; no Context Engine→Ego requirement.
- **Documentation:** update `docs/subsystems/context-engine.md`, `docs/api.md`, `docs/architecture.md`, `docs/maintainer-map.md`, `docs/crate-first-layout-spec-2026-10-25.md`, relevant crate-map and `docs/INDEX.md` **only for code actually moved**. Run `ddocs check --indexes --links`, existing architecture check, available refreshed Arcana/Lexicon where feasible, and `git diff --check`. Record stale graph witness and tooling limitations separately.
- **History/product:** no changed durable format; verify current REL/PHY conversation compaction checkpoint read/reopen and existing Echo evidence round trip where already covered. No new model endpoint, worker, lifecycle, fake source or cross-chat behavior. Promoted `CVAFRS07` corpus is an independent program gate, not a prerequisite to moving these pure files; do not claim it passed.
- **Commit discipline:** after final-source verification record file ownership, exact test counts, commands, docs/architecture status, migration commit and remaining independent gates in `docs/context-engine-crate-extraction-evidence-2026-10-28.md`. Commit only complete integration. Preserve `docs/graph-crate-extraction-evidence-2026-10-26.md` untouched.

**Definition of done:** existing real Context Engine behavior is compiled and consumed from `reliquary-context-engine`; Root has no parallel implementation; the public Root surface is retained by direct re-exports; tests, Cargo boundaries, docs and commit are complete. Future Ego synthesis, cross-chat selection, deeper context assembly, actual deployment or service creation **are not part of this refactor**.

## 6. Reference decisions

[Context Engine subsystem](subsystems/context-engine.md), [crate-first layout](crate-first-layout-spec-2026-10-25.md), [crate-first architecture](crate-first-architecture.md), [crate map](architecture/crate-map.json), [Echo architecture](decisions/0014-echo-historical-reasoning-traces.md), [behavioral contracts](behavioral-contracts.md), [Ego deferral](ego-service-crate-extraction-spec-2026-10-28.md).

**Specification only:** this commit adds no Rust implementation, feature, provider call, source migration or persistent record.
