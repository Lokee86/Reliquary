# Data, Processes, and Protocols

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines architectural standards for sources of truth, generated state, persistence, process boundaries, protocols, compatibility, and migration.

## Overview

Data and process boundaries become durable architecture when independently deployed components, persistent formats, language runtimes, or external consumers depend on them. These boundaries require stronger contracts than ordinary internal calls.

## Sources of truth

Every important data set must identify one authoritative source at a given stage of operation or migration.

The architecture must distinguish:

- authored source;
- authoritative runtime state;
- persisted records;
- immutable snapshots;
- derived indexes and caches;
- replicas and materialized views;
- exports and interchange formats;
- temporary transaction or recovery state.

Derived state should be reproducible from named authorities or have an explicit recovery path when reproduction is impractical.

## Authored and generated state

Generated state must live in recognizable locations, carry enough identity to detect staleness, and be excluded from ordinary authored-content ownership.

Generated state should record the inputs and versions necessary to determine validity, such as:

```text
source identity or content hash
schema or format version
producer version
configuration identity
model or adapter identity
creation generation
upstream snapshot identity
```

Generated output must not silently become the only copy of user-authored information.

## Persistence boundaries

Persistent formats and storage models define:

- authoritative records and derived records;
- identity and key stability;
- schema or format version;
- validation and corruption behavior;
- transaction and publication model;
- compatibility and migration policy;
- backup, export, restore, and rebuild behavior;
- retention and garbage-collection rules;
- ownership of operational repair.

Storage access should not leak broadly through the system. The persistence owner exposes domain-meaningful operations or a focused data contract rather than allowing unrelated callers to encode storage assumptions.

## Process boundaries

A process boundary is justified by independent lifecycle, resource isolation, deployment, trust, language/runtime needs, scaling, failure isolation, or independently useful product value.

A separate process must define:

```text
Startup and readiness
Discovery or addressing
Authentication and authorization when applicable
Request and response contract
Timeouts and cancellation
Concurrency and backpressure
Failure and retry behavior
Shutdown
Version negotiation or compatibility
Logs, traces, metrics, and health
```

A process should not be introduced solely to create conceptual separation that a package boundary can provide more cheaply.

## Protocol contracts

Protocols include network APIs, IPC, JSONL streams, command invocation, plugin messages, event schemas, file handoffs, and shared database contracts.

A protocol contract must define:

- stable message or operation identity;
- required and optional fields;
- validation and bounds;
- ordering and delivery guarantees;
- error representation;
- timeout and cancellation behavior;
- replay, duplicate, and idempotency behavior;
- version and compatibility policy;
- security and trust assumptions;
- test fixtures or contract tests.

Consumers must not infer semantics from undocumented field combinations, log text, or implementation-specific ordering.

## Compatibility

Compatibility policy must say which changes are:

- backward compatible;
- forward compatible;
- additive but capability-gated;
- migration-requiring;
- unsupported and intentionally breaking.

Unknown fields, enum values, relation labels, message types, or format sections require explicit handling. Safe degradation must preserve uncertainty rather than silently invent semantics.

## Migration and dual systems

A migration defines authority at every phase:

1. old authority and new shadow path;
2. backfill or import ownership;
3. validation and comparison;
4. controlled cutover;
5. new authority and old compatibility path;
6. removal of fallback, dual writes, and migration-only code.

Dual writes and bidirectional synchronization require a bounded end state. Indefinite dual authority is architectural debt and must be recorded as such.

Use strangler migrations when they preserve clear ownership and permit verified incremental replacement. Do not retain migration fallbacks after the old owner is intentionally retired unless compatibility requires them.

## Cross-repository contracts

A cross-repository dependency must have an explicit owner, versioning method, release coordination policy, and consumer verification path.

Source copying is not a contract. Vendored generated snapshots must identify their canonical source and must not be edited as independent authorities.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Ownership and dependency direction](ownership-and-dependency.md)
- [State, lifecycle, and concurrency](state-lifecycle-and-concurrency.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)

## Notes

A local file handoff can be as architecturally significant as an HTTP API. Durability and independent consumption, not transport technology, determine the strength of contract required.
