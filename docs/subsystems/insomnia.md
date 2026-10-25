# Insomnia

Parent index: [Subsystem documentation](./INDEX.md)

## Purpose

Describe the implemented Episode-to-Memory processing subsystem, including durable work identity, extraction, evidence, ownership routing, publication, recovery, and worker scheduling.

## Overview

Insomnia converts accepted conversational/source evidence into candidate Memories and publishes accepted semantic results through the owning REL or PHY boundaries. It is the semantic author for extracted Memory propositions; callers may request processing, but they do not supply authoritative Memory bodies.

Processing is durable and retry-aware. Model calls and evidence gathering occur outside mutable owner locks; publication revalidates source/work identity before commit.

## Work and scheduling

Insomnia work is owner-qualified and Episode/source-bound. The durable queue tracks priority, attempt/lease state, completion, and recovery. Workers use bounded leases and backpressure rather than holding owner locks across model calls.

Ordinary Episode processing may validly produce zero Memories. Explicit-retention semantics are a separate future extension in [the explicit Memory commit plan](../insomnia-explicit-memory-commit-plan.md).

## Extraction and evidence

The extractor receives bounded source evidence, produces structured candidates, and records exact provenance roles. Additional evidence requests are bounded by turn count, request count, and byte budget.

Candidate text, category/metadata, ownership, and provenance are validated independently. Grounding evidence cannot silently become semantic authority.

Extraction emits bounded semantic-stage events before provider work. Gateway consumes each event, attaches worker context and acknowledges reporting before extraction proceeds. Insomnia receives no Gateway execution callback. If reporting panics, its panic is resumed before the next provider call; stage events do not become durable completion authority. Worker progress, heartbeat and publication remain Gateway responsibilities.

## Ownership routing

Insomnia classifies accepted candidates into the appropriate durable owner, currently including REL/project and PHY/user placement. Ownership routing does not transfer source authority or bypass destination authorization.

The destination owner publishes the accepted Memory and its exact provenance. Publication/retry identity prevents duplicate semantic state.

## Metadata and enrichment

Insomnia may attach deterministic or separately verified metadata, including Entity mentions and Chronos-derived nondeterministic conclusions, without changing Memory semantic authorship.

Entity mention spans are bound to the exact Memory body. Chronos inference is persisted only after deterministic verification and is bound to exact body/reference chronology.

## Completion and recovery

Completion is atomic with respect to the accepted semantic outputs owned by the work item. Crash/reopen reconstructs queue/progress state and must not mark partially published work complete.

Retryable model/output/publication failures retain durable work identity. Exhausted attempts become explicit terminal outcomes rather than silent success.

## Failure behavior

- malformed structured output is rejected;
- invalid source/provenance cannot publish;
- stale leases cannot complete work;
- destination/source drift is revalidated before publication;
- duplicate retry cannot duplicate Memory publication; and
- queue recovery does not infer semantic success from a prior model call alone.


## Owner-local persistence boundary

The Insomnia store consumes its own work/attempt records and completion metadata. Its scheduling input is the transient `InsomniaEpisodeSource` coordinate value; it receives no Archive or Memory store. The existing REL admitting caller obtains accepted Episode coordinates, validates historical Episode and Memory references, and supplies those values before queue/replay acceptance. The store keeps lease, attempt, retry, completion and scheduling invariants.

`facade/insomnia_admission.rs` composes these reads and maps grouped completion metadata to an Insomnia attempt receipt. The existing Insomnia author still coordinates atomic Memory publication; this changes no work identity, durable layout, model execution, retry policy or explicit-retention semantics.

## Code Map

- `src/runtime_inference/insomnia.rs`
- `src/runtime_inference/insomnia/**`
- `src/runtime_inference/insomnia/**/*.rs`
- `src/runtime_inference/insomnia/worker/runtime/*.rs`
- `src/runtime_inference/runtime_host_rel_insomnia.rs`

## Related docs

- [Architecture](../architecture.md)
- [Rust API](../api.md)
- [Chronos](chronos.md)
- [Perception](perception.md)
- [Explicit Memory commit plan](../insomnia-explicit-memory-commit-plan.md)
- [ADR 0012](../decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)

## Notes

This document owns current Insomnia behavior. Calibration reports and explicit-retention planning do not replace it.
