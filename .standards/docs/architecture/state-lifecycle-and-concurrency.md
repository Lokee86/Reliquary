# State, Lifecycle, and Concurrency

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines architectural requirements for mutable state, lifecycle transitions, concurrency, scheduling, shutdown, and recovery.

## Overview

Stateful behavior is an owned system, not a collection of fields and callbacks. The architecture must identify who may mutate state, which transitions are valid, how concurrent work is coordinated, and what happens when work is interrupted.

## State ownership

Every mutable state domain must identify:

- its authoritative owner;
- its source of truth;
- permitted readers and writers;
- validation and invariant enforcement;
- transition triggers;
- persistence or reconstruction behavior;
- visibility and consistency guarantees;
- recovery behavior after interruption or partial failure.

Caches, indexes, replicas, views, UI models, and derived graphs must not be mistaken for authorities unless the architecture explicitly promotes them.

## Mutation boundaries

Authoritative mutation should pass through one narrow boundary. Callers may request state changes; they should not reproduce the owner's validation, ordering, or side effects.

A mutation boundary should make clear:

```text
Preconditions
Validated input
Atomic or multi-step changes
Observable result
Emitted events or follow-up work
Failure state
Retry and idempotency behavior
Rollback or repair behavior
```

When several stores change together, the architecture must define transactionality or reconciliation rather than imply atomicity.

## Lifecycle

Stateful components define their lifecycle explicitly. Relevant states may include:

```text
uninitialized
initializing
ready
active
paused
degraded
stopping
stopped
failed
recovering
replaced
```

The exact states depend on the system. The requirement is that creation, readiness, activation, shutdown, failure, and replacement are intentional rather than inferred from scattered booleans or process existence.

Lifecycle ownership includes resource acquisition, background work, cancellation, cleanup, restart, and publication of readiness.

## Concurrency ownership

Concurrency belongs to the component that owns the coordinated state or resource budget. Callers should not independently choose worker counts, lock ordering, retries, or scheduling policy for another component's state.

The design must identify:

- concurrency unit and work ownership;
- serialization boundaries;
- locks, queues, channels, actors, or transaction mechanism;
- ordering guarantees;
- cancellation propagation;
- backpressure and resource limits;
- starvation and deadlock considerations;
- publication and visibility rules;
- safe shutdown behavior.

## Scheduling and background work

Background tasks require a lifecycle owner. Fire-and-forget work without cancellation, error reporting, or shutdown integration is not an architectural boundary.

Schedulers must define:

- how work is admitted;
- duplicate suppression or coalescing;
- retry and delay policy;
- concurrency and resource budgets;
- persistence across restart when required;
- how failures become observable;
- how shutdown drains, cancels, or abandons work safely.

## Idempotency and retries

Retryable operations must state their idempotency boundary. Idempotency may be provided by operation identity, compare-and-swap, transaction keys, deduplication records, immutable publication, or reconciliation.

Blind retry is not recovery. A retry policy must distinguish transient failure, permanent rejection, unknown completion, and corrupted state.

## Shutdown and recovery

Shutdown is part of the normal lifecycle. Components must define whether they:

- reject new work;
- drain accepted work;
- cancel in-flight work;
- persist resumable state;
- publish a stopped or unavailable state;
- release locks and external resources;
- validate or repair state on restart.

Recovery should prefer immutable publication, explicit journals, durable pending records, generation pointers, or deterministic rebuilds over guessing from partially written state.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Data, processes, and protocols](data-processes-and-protocols.md)
- [Resilience, observability, and operations](resilience-observability-and-operations.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)

## Notes

Concurrency primitives are implementation details only after the ownership and consistency model is clear. Choosing a mutex, queue, actor, or database transaction does not by itself define the architecture.
