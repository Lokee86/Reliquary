# Echo crate extraction — implementation specification

**Status:** Ready for implementation; extraction **not started**.  
**Branch/baseline:** `refactor/crate-first-restart` at `880d2ec`; Container, Archive and Memory Core are extracted.  
**Target:** `crates/reliquary-echo/` — a **real**, independently compiled source-scoped Echo owner.  
**Scope:** correction-only hard cut, no new provider behavior or on-disk format.

## 1. Problem and desired outcome

Echo is an independent durable owner, but the production implementation is still the root package's `src/echo/echo_{model,codec,store}.rs`, mounted through `src/lib.rs`. Its public values, store replay, REL open path, import/CLI consumers and Context Engine/Runtime callers all compile within the monolith. This prevents Cargo from enforcing the already accepted dependency boundary and makes Echo easy to mistake for Archive, transcript or Memory state.

**Done means** one actual Echo implementation in `reliquary-echo`; the root `Cva` opens, appends and reads that crate's owner; all actual consumers use its canonical `EchoEvent`, `EchoEventKind`, `EchoError` and store; the root's obsolete source modules and declarations are deleted. No separate physical container, runtime service, duplicate codec, adapter or hidden second owner.

## 2. Governing architecture and evidence

| Authority | Binding decision |
| --- | --- |
| [Crate-first target](crate-first-layout-spec-2026-10-25.md), [component DAG](architecture/component-dag.json), [crate map](architecture/crate-map.json) | Logical `echo` is a rank-2 store. Physical `reliquary-echo` may depend **only on `reliquary-container` among Reliquary crates**, not Archive, Memory Core, Retrieval, Context Engine, Runtime, History or the root product. Higher consumers depend downward. |
| [ADR 0014: Echo](decisions/0014-echo-historical-reasoning-traces.md) | Echo is historical computation/evidence attached to a visible source, **not** transcript/Archive truth, durable Memory truth or Ego summary. It is cold by default and must not silently become ordinary retrieval content. Source-local retrieval begins with **one** selected visible source. |
| [ADR 0001](decisions/0001-purpose-built-database-ownership.md), [storage format](storage-format.md), [behavioral contracts](behavioral-contracts.md) | Echo is REL-only and occupies `CVAECHO1` physical records. Preserve current eight-byte magic, typed binary fields, ID/sequence identity, UTF-8 lengths, present flags, error behavior, and byte-neutral opening. Echo consumes **no** global, Archive, Memory, Graph or Vector Generation version. It does not wrap records in `CVAPUB01`. |
| [Container extraction](container-crate-extraction-spec-2026-10-25.md), [Archive extraction](archive-crate-extraction-spec-2026-10-25.md), [Memory Core extraction](memory-core-crate-extraction-spec-2026-10-25.md) | Reuse current physical writer/scan and error discipline. Archive's transcripts and Memory's revisions remain independently owned; do not move their code to Echo or make peers depend on Echo. |
| [Runtime multiplexing](subsystems/runtime-multiplexing.md), [context engine](subsystems/context-engine.md) | Runtime/owner grants decide **who** may record or view evidence; Context Engine decides **how** chosen evidence is rendered. An Echo record by itself cannot grant authority or instantiate provider continuation. |
| Actual current source | `src/echo/echo_model.rs`, `echo_codec.rs`, `echo_store.rs`, `echo_tests.rs`; `src/facade/cva.rs`, `cva_lifecycle.rs`, `cva_error.rs`; `cva_reconcile_repack.rs`; `src/context_engine/evidence.rs`; `src/runtime_inference/runtime_host_gateway_{ambient,conversation_generation_events,conversation_metadata}.rs`; `cli/src/import_{cmd,input,tests}.rs`; `tests/live_runtime_adapter.rs`. |

**Selective salvage:** inspect actual Echo/reconcile/physical diffs in abandoned `d1c0d4b2`, `957733a4` and `86145476` against the verified current baseline; adopt a concrete correctness fix only when applicable. Do **not** cherry-pick phase scaffolds, broad architecture ratchets, or old wrappers. The original `C:\!bin\workspace\Reliquary` main, backup bundle and promoted corpus must remain untouched.

**Corpus constraint:** `CVAFRS07` is separately owned Freshness/Import settlement evidence, not part of Echo. No runtime fallback readers or migration machinery are authorized for this extraction.

## 3. Canonical source ownership and minimal interface

The placeholder package already exists. Move these **three** production modules directly, retaining their descriptive filenames if preferable:

```text
crates/reliquary-echo/
  Cargo.toml            # Reliquary dependency: reliquary-container only
  src/
    lib.rs              # EchoEvent, EchoEventKind, EchoError, EchoStore and needed methods
    echo_model.rs       # current stable source-keyed value and typed errors
    echo_codec.rs       # exact CVAECHO1 encoder, recognizer, decoder, validation
    echo_store.rs       # append/replay/ordered source reads and idempotency
    echo_store_tests.rs # focused owner-local tests; only if moved/split
```

`EchoStore` should expose only operations actually required by the root owner and higher callers:

- `default/new`, `ingest(payload)`: recognize only `CVAECHO1`; unrelated records do nothing, recognized malformed records fail closed without changing accepted state. Normal replay is read-only; absence of Echo records yields an empty store.
- `put(&mut Container, EchoEvent)`: validate the event and append the **existing raw** `CVAECHO1` bytes under caller-held writer authorization; return `true` on new record, `false` for identical same-key retry; conflicting same-key replay/write fails with `ConflictingSequence`. Failed append never mutates in-memory Echo.
- `for_turn(conversation_id, message_id)`: return source-keyed events in ascending **numeric sequence**, no neighboring turn expansion.
- `for_conversation` and `all_records`: remain **bounded by the higher caller's authorized REL scope** and available only as explicit enumeration/recovery/reconcile APIs. They do **not** constitute conversation/global Echo *search* or allow bypassing the single-visible-source retrieval rule.
- Keep `EchoEvent` fields and the existing `EchoEventKind` discriminants, including `ToolContextAccepted` as tag **8**. Use typed owner errors; `CvaError::Echo` conversion lives **above** Echo.

Keep codec validation as one implementation, not a second implementation at the root. Keep individual event records and existing `HashMap<(conversation, message), BTreeMap<sequence, event>>` or an equivalently simple index. **Do not build** a new index/search abstraction just for this extraction.

### Persistence invariants

Freeze current format in [storage format](storage-format.md#echo-sidecar-records): `CVAECHO1` magic; length-prefixed conversation/message strings; `u64` sequence; `i64` nanosecond timestamp; optional `u32` model round; 0–8 kind tag; optional correlation/name strings; content string. Preserve all current validation and errors: required source IDs, required tool-call name/correlation, required result/activity/ack correlation, invalid flags/tags, invalid UTF-8, truncation, oversized lengths and trailing bytes.

`ToolContextAccepted` is durable **private receiver acknowledgment**. It must survive reopen/import/reconcile but **never** be rendered as model/UI execution evidence. An ordinary tool result is **not** acknowledgment or accepted-use proof. The authenticated manifest/digest, owner/receiver checks, Freshness reinforcement and delivery journal remain higher-level authority.

Provider continuation tokens, opaque runtime `ProviderState`, transient `CurrentContext`, visible assistant output and tool-result transcript remain with their existing owners. Do not introduce provider-native reasoning representations, derived segmentation, new compression, Echo vectors or a generalized trace schema during this cut.

## 4. Concrete consumer crosswalk

| Existing code | Required migration |
| --- | --- |
| `src/lib.rs` path modules `echo_codec`, `echo_model`, `echo_store` | Remove displaced root modules and duplicate definitions; retain existing supported product `pub use reliquary_echo::{EchoEvent, EchoEventKind, EchoError}` as **one deliberate external API re-export**. Internal consumers import the canonical new crate, not a product forwarding implementation. |
| `src/facade/cva.rs` | Change held `EchoStore` to the compiled crate's type; `put_echo_event`, `echo_events`, `conversation_echo_events`, `echo_records` delegate directly to that one implementation while preserving return values and `CvaError`. Facade retains REL write/read admission and higher caller authority. |
| `src/facade/cva_lifecycle.rs` | Construct/replay `reliquary_echo::EchoStore` within the existing REL physical cursor's **original record order** and Fallback route. Do not add Echo to PHY. No implicit repairs, no extra global version reservation, no unintended envelope. |
| `src/facade/cva_error.rs` | Preserve `Echo(EchoError)` mapping with its existing typed public error behavior; no `CvaError` inside Echo crate. |
| `src/facade/cva_reconcile_repack.rs`, `cva_reconcile_tests.rs` | Preserve the existing ordered left/right Echo record replay and exact duplicate/conflict behavior across canonical REL copies; coordination remains Maintenance/root. Echo implements only local record identity and ordering. |
| `cli/src/import_input.rs`, `import_cmd.rs`, `import_tests.rs` | Continue to read/write through the documented `reliquary-memory` product API; imported reasoning and tool evidence remain source-keyed with the existing binary layout. CLI's separate lockfile must be checked. |
| `src/runtime_inference/runtime_host_gateway_ambient.rs`, generation/metadata code, `interaction_live.rs`, `runtime_shared.rs` | Retain attached ViewRel/writer grants, authenticated source selection, generation/correlation checks and partial-generation recovery **at Runtime**. Echo does not take an instance token or launch provider work. |
| `src/context_engine/evidence.rs` | Keep `render_assistant_content_from_echo` in Context Engine and retain exclusion of `ToolContextAccepted` and activity-only units. Source selection/authority occurs before these events are presented; Echo itself does not format prompts. |
| `src/echo/echo_tests.rs`, `src/context_engine/tests.rs`, `tests/live_runtime_adapter.rs` | Move only codec/store-local tests into Echo crate. Keep REL lifecycle, runtime permissions, import/reconciliation, acknowledgment rendering and public API tests at the owning higher integration seam. |

**Important distinction:** `for_conversation` is used for *authorized transcript/restart materialization* and `all_records` for *maintenance reconciliation*. They must not be replaced with new ordinary conversation-wide reasoning search. Echo's retrieval contract still requires a selected **one** visible parent source before querying or ranking material.

## 5. Staged hard-cut migration

1. **Freeze real behavior:** confirm clean HEAD, Cargo DAG, `CVAECHO1` fields, current tests/consumers and relevant archived corrections. Use direct `rg`/Cargo inspection; Arcana only if a current index exists. No generated graph census.
2. **Move owner once:** move the three root model/codec/store implementations directly into `crates/reliquary-echo/src/`; add only `reliquary-container` as an internal Cargo dependency. Make exactly the minimal `EchoStore` API public that actual root consumers need; keep private validation/indices private. Unit-test independent replay/codec/put using disposable Container files.
3. **Rewire product and consumers:** add root's actual Cargo path dependency, replace `#[path]` declarations and imports; keep root product public exports and existing Cva/replay/error/reconcile APIs. Move only owner-local tests; leave Runtime/Context Engine/CLI code in their assigned packages. No dual Echo store, copying wrapper, new interface crate, or background feature.
4. **Clean and integrate:** delete obsolete `src/echo/echo_{model,codec,store}.rs` and old owner-local test declarations, update [maintainer map](maintainer-map.md), [architecture](architecture.md), [storage format](storage-format.md), root API/code navigation and accepted crate-first layout after implementation. Create one reviewed extraction commit after checks.

A bounded locally red interval is acceptable. **Do not** pre-extract Retrieval, Context Engine, Runtime Gateway, History or Maintenance simply to make the Echo crate compile. The complete root build is required at the final Echo extraction integration gate.

## 6. Testing and completion gates

**Existing targeted tests first:**
- `src/echo/echo_tests.rs`: sorted source-local replay/reopen; duplicate retry versus conflicting sequence; typed tool/action binary fields; receiver-ack persistence and private rendering. Split into crate-owned codec/store tests and root `Cva`/render tests, not duplicate suites.
- `src/facade/cva_reconcile_tests.rs`: divergent REL reconciliation carries both Echo histories and rejects incompatible same-key payloads; repeated replay remains byte-neutral.
- `src/context_engine/tests.rs`: selected Echo evidence rendering excludes receiver acknowledgments and activity-only entries.
- `cli/src/import_tests.rs`, `tests/live_runtime_adapter.rs`: importer carries Echo reasoning/tool order and public runtime adapter remains compatible.
- Runtime-specific permission/sequence tests in `runtime_host_gateway_ambient_tests.rs`, `runtime_host_gateway_conversation_generation_events.rs`, and `interaction_live_acceptance_tests.rs`; preserve binding to exact granted REL and generation source.

**Minimal additional unit gates if current tests lack them:** existing `CVAECHO1` literal/round-trip, malformed known records and full-prefix/trailing-byte rejection, unrelated-owner payload ignored, failed physical append leaves `EchoStore` unchanged. Do not build a huge independent fixture matrix or new test architecture.

**Final measurable acceptance:**
- `cargo check -p reliquary-echo --locked --offline` and `cargo test -p reliquary-echo --locked --offline` pass with **actual migrated code**, not the placeholder.
- `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, focused root Echo/Context Engine/reconciliation/runtime tests, and independent `cargo test --manifest-path cli/Cargo.toml --locked --offline` pass (CLI has its own lockfile). Disclose any environmental blockers rather than counting unrun tests.
- Cargo manifest confirms **only `reliquary-container`** as a Reliquary dependency; no Echo → Archive/Memory/root/Runtime edge. The root uses only `reliquary-echo`; the old owner model/codec/store implementations and `src/lib.rs` path declarations are gone.
- Exact preexisting `CVAECHO1` bytes, deterministic sequence, accepted duplicate semantics, REL/PHY owner-kind admission, crash/reopen and reconciliation are preserved, with no automatic write during ordinary open.
- Runtime grants, single-visible-source retrieval policy, tool-context-ack privacy and independent Memory/Archive authority remain unchanged. Current-format corpus `CVAFRS07` acceptance remains a **separate** future gate, not a reason to add compatibility readers to Echo.

## 7. Out of scope

No provider-state continuation redesign, semantic role changes, Echo-driven Memory creation, bulk reasoning reconstruction, source-wide search, new lexical/vector retrieval infrastructure, Archive/Memory schema change, Context Engine redesign, Insomnia/Dream/Ego/Freshness extraction, global-history rewrite, or custom proof/ratchet framework.

**Acceptance is a real owned, compiled Echo crate and working concrete callers.** A green empty scaffold, unused crate, or speculative test count is not completion.
