# Resilience, Observability, and Operations

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines architectural standards for failure handling, degradation, diagnostics, observability, health, recovery, and operational control.

## Overview

Operational behavior is architecture because failures cross ownership boundaries and because recovery often requires information unavailable inside an isolated function. A system that works only when every dependency succeeds has not defined its runtime contract.

## Failure ownership

The component that owns an operation also owns its failure semantics. It must distinguish at least:

- invalid input or rejected policy;
- unavailable dependency;
- timeout or cancellation;
- transient failure;
- permanent failure;
- unknown completion;
- stale or incompatible state;
- corruption or invariant violation;
- degraded but usable operation.

Callers may decide user-facing response or larger workflow policy, but they must receive failures that preserve the owning component's meaning.

## Failure containment

Boundaries should prevent one failure domain from unnecessarily corrupting or blocking unrelated work.

Containment mechanisms may include:

- isolated processes or workers;
- bounded queues and concurrency budgets;
- immutable publication;
- circuit breakers or admission control;
- per-request cancellation;
- transactional updates;
- capability degradation;
- stale-read policy with explicit freshness;
- restartable components with reconstructable state.

Containment must not hide failure. A degraded path remains observable and documented.

## Timeouts, retry, and backoff

Every remote, external, or potentially blocking operation requires an owner for timeout and cancellation policy.

Retry policy defines:

```text
Which failures are retryable
Maximum attempts or deadline
Backoff and jitter
Idempotency boundary
Duplicate and unknown-completion handling
Escalation after exhaustion
Operator visibility
```

Nested retries must not multiply unboundedly across layers. The highest layer with complete workflow context usually owns the overall deadline; lower layers may own narrow transport retries within that budget.

## Degraded operation

A degraded mode must identify:

- which capabilities remain valid;
- which evidence or data is stale, partial, or absent;
- what authority is retained;
- how the degradation is reported;
- whether automatic recovery is attempted;
- what action restores full operation.

Fallbacks must not silently weaken correctness. When certainty is reduced, the interface should preserve that uncertainty.

## Observability ownership

Observability follows component and operation ownership.

Logs, traces, metrics, and diagnostics should answer:

- what operation occurred;
- which component owned it;
- which repository, tenant, room, job, session, request, or entity was affected;
- which version and configuration were active;
- how long each major boundary took;
- where failure or degradation began;
- whether retry, fallback, rollback, or recovery occurred;
- what the operator should inspect next.

Cross-process flows use stable correlation identifiers. Sensitive data and secrets must not be logged merely to improve diagnostics.

## Logs

Logs should be structured when machines consume them. Severity must reflect actionability rather than emotional emphasis.

Repeated expected failures should be aggregated, sampled, or rate-limited without eliminating evidence. A log line is not a recovery mechanism or a stable API.

## Metrics and traces

Metrics should represent service health, resource pressure, throughput, latency, error classes, queueing, saturation, and recovery where those affect operation.

Traces should preserve causality across component boundaries and include operation names aligned with architectural owners. Instrumentation must not redefine authority or introduce hidden control flow.

## Health and readiness

Liveness answers whether a process can continue or be restarted. Readiness answers whether it can safely accept its owned work. Dependency status may affect readiness differently depending on documented degraded modes.

A health endpoint must not report success solely because the process loop is running. It should validate the minimum state required for the claimed capability.

## Recovery and operator control

Operational controls should expose explicit actions such as:

- inspect status and versions;
- stop accepting work;
- drain or cancel work;
- rebuild derived state;
- retry or replay a bounded operation;
- repair or discard incomplete publication;
- restore from backup or export;
- roll back or advance a migration;
- produce a diagnostic bundle.

Recovery commands must validate scope and current state before mutation. Destructive repair requires explicit targeting and should preserve evidence when practical.

## Related docs

- [Architecture standard](architecture-standard.md)
- [State, lifecycle, and concurrency](state-lifecycle-and-concurrency.md)
- [Data, processes, and protocols](data-processes-and-protocols.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)

## Notes

Observability is not a substitute for clear failure contracts. Rich telemetry attached to ambiguous ownership still produces ambiguous diagnosis.
