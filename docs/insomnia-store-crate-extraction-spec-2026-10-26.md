# Insomnia Store crate extraction — implementation specification

**Status:** Specified; implementation not started (2026-10-26).
**Branch:** `refactor/crate-first-restart`. **Baseline:** `5a8b445f` (Vector Store extraction).
**Target:** `crates/reliquary-insomnia-store/` — next lower owner-store extraction.
**Scope:** One correction-only hard cut; unchanged durable format, public API and product behavior. No Insomnia service extraction.

## 1. Intended boundary

The compiled **Insomnia Store** owns durable Episode-keyed work records, attempt/lease state, accepted completion evidence needed for store replay, transitions and deterministic ready/retry/lease-expiry scheduling. It reads and appends only through **Container**, its **only direct Reliquary crate dependency**. It does not own Archive Episodes, Memory records, extraction inference, prompt/ledger logic, candidate ranking, authorizing source references, or processing execution.

Owner Lifecycle and History must be able to reconstruct the work/attempt/completion state without depending on an Insomnia model processor, Archive, Memory Core, Freshness, Runtime, or the root product. Caller-side Archive/source and cross-owner admission proofs remain in the higher product. This boundary is governed by `docs/crate-first-layout-spec-2026-10-25.md`, `docs/architecture/crate-map.json`, ADR 0012, ADR 0023, `docs/subsystems/insomnia.md`, ADR 0041/0042 and `docs/storage-format.md`.

The target crate currently contains only a placeholder manifest and `src/lib.rs`; its existence is not implementation evidence.

## 2. Concrete source crosswalk

**Move after dependencies are cut:**

- `src/runtime_inference/insomnia/store.rs`, `store/{rebuild,scheduler,transitions}.rs`: accepted work/attempt state, deterministic priority keys, lease/retry indexes, transition and replay rules.
- `src/runtime_inference/insomnia/codec.rs`: current `CVAINSF1`, `CVAINSW1`, `CVAINSA1` record bytes, strict decode/encode, format pin and exact retry identity.
- `src/runtime_inference/insomnia/rebuild.rs`: passive accepted-prefix scan, projection and completion recognition.
- Durable **value** portions of `model.rs` and `error.rs`: canonical `InsomniaWork`, `InsomniaAttempt`, `InsomniaPriority`, `InsomniaWorkState`, lease token, stats and precise store error codes; keep provider/worker/service errors higher.
- Store-specific portions of queue, recovery, scheduler and completion tests, retaining exact golden bytes and replay fixtures.

**Split rather than blindly move:** `completion.rs`, `completion_compact_retained.rs`, `completion_compact_tests.rs` and completion tests. The existing `CVAINSC7` completion contains grouped Memory bodies, Memory records, routing metadata, global versions and explicit Freshness birth proof. Its current codec imports `reliquary_memory_core`, root Memory and Freshness policy. Moving that entire codec into the lower store would violate the approved Cargo boundary. The higher product retains **full completion authoring/decoding and cross-owner validation**; the lower store consumes only an exact, already-authenticated *completion projection* (episode, attempt, status, accepted times and necessary durable work/receipt identifiers) and optionally recognizes the fixed completion envelope minimally on replay. Do not introduce a second completion format or rebuild a separate Memory codec. Define how replay hands off decoded completion facts across the boundary **before** moving `rebuild.rs`: either the higher owner decodes then passes the narrow projection during its existing frame traversal, or the store performs bounded envelope-only extraction with byte-compatible tests. Favor the first when exact validation would otherwise be weakened. Ensure retained compact completions and fresh/reopened prefixes are both covered.

**Remain higher:** `candidate*`, `candidate_policy*`, `ledger.rs`, `extraction*`, `enrichment*`, `evidence*`, `processor*`, `worker*`, `runtime_step.rs`, `synthesis.rs`, `temporal.rs`, `input.rs`, `cva.rs`, `ownership.rs`, `backpressure.rs`, provider/model calls, and episode-to-Memory authorization. Their tests remain root service/integration tests.

## 3. Mandatory dependency corrections

| Observed coupling | Required hard-cut interface |
| --- | --- |
| `InsomniaStore::queue` and `Scheduler::new/register/rebuild` import `Archive` and resolve `archive.episode()` | Caller authenticates actual Episode existence, owner, and accepted Archive source position; passes immutable `AcceptedEpisodeOrder` containing exact 32-byte Episode ID, source-through timestamp, conversation and start-node ordering fields. Store uses it for deterministic priority key; never fabricates Episode evidence or imports Archive. On passive reopen, caller supplies accepted Episode metadata for every active work item; missing/inconsistent metadata retains existing error/skip behavior. |
| Work/attempt structs, codec and scheduler reference root `EpisodeId` and `MemoryId` | Lower defines fixed-byte Episode/Memory identities with direct explicit root conversion and identical hash/order/wire bytes. No generic identity framework and no all-store wrapper. |
| `InsomniaOpenState::ingest` imports high-level `completion::decode_completion` | Split full completion parsing/Memory/Freshness validation higher; transfer only accepted completion facts to store's replay/projection. Preserve frame order and fail-closed invalid completion semantics. No accepting malformed completed claims because a shallow header matches. |
| `InsomniaStore::apply_completion` reads `InsomniaCompletion` | Replace with a narrow store-owned authenticated `AcceptedCompletionState` / value record; higher authorizes exact full completion against Memory grouped bodies, routing, global versions, Freshness birth and ownership before applying it. Store checks attempt progression, old/current transition and idempotence. |
| Store transition methods depend on root error, lease and worker types | Move only exact durable errors/token/transition values, re-export or convert at root. Lease selection and scheduling are deterministic state operations; processor/worker dispatch remains higher. |
| Queue/claim methods carry `&mut Container` without writes | Preserve actual accepted durability contract: inspect where transitions are recorded, and do not falsely add journaling in the store or silently treat ephemeral transitions as durable. Accepted append points, atomicity and error precedence must remain identical. |

**Critical preimplementation decision:** document the *actual* persistence lifecycle of queue, claim, renew, retry, completion and attempt. Several `store.rs` methods presently take `_container`, meaning their transitions do not append there. Identify which caller writes durable work/attempt/completion frames; preserve ordering and crash recovery exactly. Do not change accepted semantics to make the ownership label appear simpler.

## 4. Nonnegotiable invariants

1. **Container is physical owner:** no new queue database, second journal, ad hoc frame file, new clock or automatic repair. Reads/reopen remain passive; no model execution on replay.
2. **Exact current encoding:** preserve `CVAINSF1`, `CVAINSW1`, `CVAINSA1` and full `CVAINSC7` bytes, including retained compact completion and its explicit Freshness birth cohort. Reject malformed/truncated/foreign versions and unsupported legacy. No new format migration.
3. **Scheduling determinism:** priority, source-through, conversation ID, start-node ID and Episode ID tie-break order unchanged; ready/retry/lease-expiry promotion, worker claim and renewal behavior unchanged.
4. **Identity and idempotence:** exact 32-byte Episode and Memory IDs, stable lease token computation, bounded attempt counts, explicit transition validation, duplicate retries and conflicting receipts fail closed. Reopen reconstructs same terminal/pending/failed states.
5. **Cross-owner authority:** only higher caller validates Archive Episode, Memory creation/provenance, external-memory owner references, Freshness birth proof, grouped body encoding, producer model and source routing; lower independently checks intrinsic record consistency. No implicit Memory publication on passive read.
6. **Recovery/performance:** replay valid accepted prefixes including completion-before/after-work cases according to existing rules; no skipped completion due to frame-scan separation, no repeated full Archive scan per work, no copied full queue or new unbounded receipt mirror.
7. **Public API:** root/REL/PHY/CLI queue, lease, attempts, completion and processing tests still work with unchanged signatures and error classes; store does not own higher scheduling policies or change model inference.

`CVAFRS07` recovery and the authoritative six-file current-format corpus remain separate product-level gates. This extraction must not claim that gate without actually running it.

## 5. Bounded hard-cut execution

1. **Freeze:** confirm commit/branch/status and inspect root callers with `rg`. The unrelated modification `docs/graph-crate-extraction-evidence-2026-10-26.md` must never be staged, reset, edited or committed. Capture exact format vectors, queue/crash/replay tests and current write path.
2. **Separate durable primitives:** relocate canonical lower work/attempt/lease priority/value/error types and fixed-byte identities; explicit root conversion. Independently compile and test format+work/attempt codecs.
3. **Correct Archive scheduler seam:** root constructs verified Episode ordering facts; move scheduler/transition state. Ensure absent Episode cases preserve present behavior; test stable mixed-priority queues, tied source order, expiration and retry.
4. **Correct completion seam first:** keep full `CVAINSC7` authoring and Memory/Freshness validation above; implement and test the single narrow authenticated completion projection at replay/apply boundary; preserve compact retained replay and exact no-append behavior.
5. **Move real lower store and rebuild:** use Container frames, strict prefix/format, accepted attempt/work/completion ordering. Compile lower crate with only Container among Reliquary deps; delete displaced root owner code when callers have migrated.
6. **Rewire product clients:** Cva/REL open, worker, import/reconciliation, historical reads, retention/prune/compact, detached CLI and tests to the one compiled store. No permanent forwarding `mod store` facade or duplicate queue cache.
7. **Verify/integrate/commit:** update Cargo/locks, `docs/api.md`, `docs/architecture.md`, `docs/maintainer-map.md`, `docs/storage-format.md`, `docs/subsystems/insomnia.md`, and this spec. Record actual outcomes in `docs/insomnia-store-crate-extraction-evidence-2026-10-26.md`. Commit only the bounded hard cut after green product and CLI acceptance.

During cut, track precise remaining callers and first compiler failure. Never call the extraction complete merely because the lower crate compiles.

## 6. Acceptance matrix

- **Lower:** `cargo check -p reliquary-insomnia-store --locked --offline`; `cargo test -p reliquary-insomnia-store --locked --offline`. Source/manifest inspection confirms only Container is a direct Reliquary dependency, with no root, Archive, Memory, Freshness, Insomnia service or Runtime import.
- **Wire/replay:** golden work/attempt/completion bytes and current-only rejection; malformed/foreign format, conflicting duplicate, accepted-prefix and compact-completion replay; zero writes on reopen; exact raw vs projected completion and receipt parity. Test the high-level producer to lower accepted-completion seam.
- **Scheduling:** queue uniqueness, tie ordering, lease claim/renew/expiry, retries, failed/terminal transitions and mixed worker/owner scenarios, including real Archive episode authorization failures.
- **Product:** focused root `insomnia`, `queue`, `completion`, `recovery`, `import`, `freshness`, `historical`, `owner` cases, emphasizing `insomnia::completion_tests`, `queue_recovery_tests`, `scheduler_tests`, `worker_backpressure_tests`, and retained compact completion tests.
- **Integration:** `cargo check --workspace --locked --offline`; `cargo fmt --all -- --check`; `cargo test --manifest-path cli/Cargo.toml --locked --offline`. Run at least one real owner reopen and current native completion replay; distinguish independently run evidence from recorded prior results.
- **Completion:** real production store, codecs and accepted replay in rank-2 crate; authenticated Archive/completion seams; no duplicate store or upward Cargo imports; root and CLI green; evidence and commit. No broad architecture ratchets, extra databases or new semantics.

## 7. Out of scope

No `reliquary-insomnia` semantic service extraction, new Memory authority rules, provider/LLM prompting changes, new scheduling policy, multi-owner worker redesign, new completion format, Dream or Import extraction, native corpus recovery, or generalized cross-store interface framework.
