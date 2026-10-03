# ADR 0023: Insomnia durable owner routing

## Status

Accepted and implemented — 2026-08-27.

Extends ADR 0012's Insomnia authority boundary, ADR 0018's Reliquary/Phylactery separation, and ADR 0022's durable typed owner identity.

## Purpose

Route Insomnia-derived durable state to the owner that should retain it without making ownership part of semantic extraction identity or introducing another probabilistic orchestration loop.

## Context

Insomnia originally published every retained proposition into the active Project Reliquary. Phylactery now exists as a distinct user-global owner with a durable `phy-<uuid>` identity, so genuinely user-global state such as stable preferences must be able to leave the Project REL without copying REL-local Episode/node source records into PHY.

The first ownership experiment stored separate `memory_ids` and `user_memory_ids` in Insomnia receipts and included ownership in the candidate semantic key. That was rejected. A bare `MemoryId` is not sufficient for a cross-file reference, and changing candidate identity merely because routing is enabled would unnecessarily change otherwise-identical Project Memory IDs.

## Decision

Insomnia uses a dedicated bounded ownership-classification pass after deterministic synthesis groups exist and after the optional metadata classifier, but before wording. The first classifier returns only:

```text
user
project
```

`project` is the conservative default and the current non-user sink. Ambiguous propositions stay with the active REL. Organization and Connection are future classifier destinations; they are not inferred indirectly in this first slice.

Ownership classification may change only the destination owner. It cannot change grouping, retained propositions, authority, provenance, category, type, lifecycle, or candidate identity. Candidate semantic keys therefore remain independent of ownership.

Project-owned drafts are published into the active REL exactly as before. User-owned drafts are published into an explicitly attached Phylactery. Before either publication, Insomnia resolves `source_time_ns` from the authoritative semantic/content source while the REL Archive is available. Before PHY publication, the validated REL-local provenance identities are converted into `MemorySourceRef { owner_id, source_episode_id, source_node_id, content_source_*, grounding_source_* }`; the REL-local provenance fields are then removed from the PHY draft. The reference contains identifiers only, while `source_time_ns` remains separate semantic chronology. The resulting Memory stays valid if the source REL is unavailable and can resolve exact provenance again when that owner is mounted.

Cross-file receipts use the general owner-qualified form established by ADR 0022:

```text
MemoryRef {
    owner_id,
    memory_id,
}
```

The current Insomnia completion transaction stores local Project `MemoryId`s separately from external owner-qualified `MemoryRef`s. It does not persist a special `user_memory_ids` field.

## Publication and retry order

There is no cross-file transaction manager. Routed publication therefore uses this order:

```text
prepare fixed REL/PHY drafts
→ publish + sync PHY user Memories
→ commit REL Project Memories + Insomnia completion receipt
```

This intentionally prefers a durable User Memory over a receipt that falsely claims it was written. If the process fails after PHY publication but before the REL completion receipt, retry uses the deterministic Insomnia mutation ID to find the already-written PHY Memory. The retry accepts it only when the routed semantics match; incompatible reuse fails closed.

The completion transaction records the PHY `MemoryRef`, making the REL receipt an explicit record of the external durable object it produced.

## Runtime surfaces

`Cva::process_claimed_insomnia_episode_routed` and `Cva::drain_insomnia_backlog_routed` accept an explicit mutable Phylactery. `ConfiguredRuntime::run_insomnia_files` owns configured endpoint resolution and the finite routed workflow; the repo-local CLI only exposes that operation as:

```text
insomnia run <REL> --phy <PHY>
```

The ownership endpoint is resolved by the library switchboard: use `insomnia_metadata` when configured, otherwise fall back to the main `insomnia` route. Without `--phy`, the ownership pass is not enabled and current behavior remains Project-only. If a User-owned candidate reaches a commit without a routing target, publication fails closed.

`ReliquaryRuntimeHost::start_with_phylactery` makes the long-lived host own both the active Project REL runtime and one optional user Phylactery. General-model inference remains outside the interaction-runtime lock. Routed publication acquires the REL and PHY only for bounded persistence work.

The finite drain and long-lived runtime vector worker fill missing Memory vectors independently in both owners using the same verified embedding endpoint/profile semantics. Memory authority remains independent of vector completion.

## Persistent format

Current identified-owner successful Insomnia writes use `CVAINSC6`, extending V5 with atomic Freshness birth proof. V5 remains readable and is emitted when owner birth proof is unavailable. It retains `CVAINSC4` transaction-time and owner-qualified external-Memory semantics, and additionally embeds the clock-neutral body-bound routing metadata produced for newly published Project Memories so REL Memory state and its Insomnia-derived Entity-mention attachment become visible atomically. Lexical Memory routing is derived from complete Memory title/content by the disposable owner-local lexical index and is not part of the Insomnia completion payload.

`CVAINSC1` through `CVAINSC4` remain decodable. V1/V2 reopen with no external Memory references; V2/V3 embedded global-version ranges predate explicit transaction time and therefore remain untimestamped rather than being backfilled from operational completion metadata. V4 carries transaction time and external references but predates embedded routing metadata.

Reconciliation preserves `external_memory_refs` when re-emitting an Insomnia completion receipt. It does not attempt to reconcile or copy the external PHY itself; the owner-qualified reference remains a reference to that separate durable owner.

## Consequences

- User-global extraction can be enabled without merging REL and PHY storage models.
- Project candidate IDs do not change merely because owner routing is available.
- Cross-file references are self-describing by durable owner ID.
- PHY Memories retain identifier-only owner-qualified provenance in `MemorySourceRef` without retaining source records; `source_time_ns` remains separate source-derived chronology, not a source pointer.
- A crash between PHY and REL writes is recoverable through deterministic mutation IDs rather than a cross-file transaction log.
- Runtime ownership remains deterministic except for the one bounded semantic classification call that is genuinely required.
- Organization/Connection routing can later extend the destination vocabulary without replacing `MemoryRef` or the owner-ID substrate.

## Implementation boundary

Current implementation is concentrated in:

- `src/runtime_inference/insomnia/ownership.rs`
- `src/runtime_inference/insomnia/extraction.rs`
- `src/runtime_inference/insomnia/processor.rs`
- `src/runtime_inference/insomnia/processor/application.rs`
- `src/runtime_inference/insomnia/completion.rs`
- `src/runtime_inference/insomnia/worker*.rs`
- `src/memory/memory_model.rs`
- `src/runtime_inference/runtime_host*.rs`
- `src/runtime_inference/runtime_vector_step.rs`
- `src/runtime_inference/configured_runtime*.rs`

`cli/src/insomnia_cmd.rs` is an interface adapter only; it does not own endpoint selection or Insomnia workflow composition.
