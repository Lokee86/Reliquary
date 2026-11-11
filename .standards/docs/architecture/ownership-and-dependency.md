# Ownership and Dependency Direction

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines how responsibilities, state, interfaces, and dependencies receive clear architectural ownership.

## Overview

Ownership answers who decides, who mutates, who validates, who recovers, and who must change when a contract changes. Dependency direction answers which parts may know about which other parts.

A system may have many consumers of a capability while retaining one owner of its policy and state.

## Responsibility ownership

Every durable responsibility must have one canonical owner. The owner:

- defines the responsibility's invariants and public contract;
- controls authoritative mutation and lifecycle decisions;
- validates inputs at its boundary;
- owns failure semantics and recovery behavior;
- owns the canonical implementation documentation and tests;
- decides compatibility and migration policy for its contract.

Callers may coordinate or compose the owner, but they must not duplicate its policy.

## Non-ownership

Architecture documentation must state important things a component does not own. Non-ownership prevents coordinators, facades, and integration packages from gradually absorbing the systems they invoke.

Typical non-ownership statements include:

- an orchestrator schedules work but does not reinterpret component results;
- a client library transports requests but does not own server policy;
- a cache accelerates reads but is not the source of truth;
- a UI presents state but does not become the mutation authority;
- an adapter translates language-specific facts but does not publish repository snapshots;
- a shared daemon supervises processes but does not absorb each product's domain logic.

## Dependency direction

Dependencies should flow from policy toward stable abstractions and from integration layers toward independently owned components.

Acceptable dependency direction is explicit in source imports, build configuration, runtime invocation, and data contracts. Hidden inversion through globals, registries, callbacks, generated code, or shared mutable state is still a dependency and must be reviewed as such.

A dependency cycle is permitted only when the cycle represents one true component that should not be split, or when an explicit runtime protocol breaks source-level ownership. Convenience is not sufficient justification.

## Public and internal boundaries

Public boundaries include externally consumed APIs, commands, protocols, schemas, persisted formats, plugin contracts, and stable component interfaces.

Public boundaries must be:

- smaller than the internal implementation surface;
- explicit about validation and failure;
- versioned when externally persisted or independently deployed;
- free of internal storage, scheduler, or package details unless those are intentionally contractual;
- backed by compatibility tests when consumers can evolve independently.

Internal interfaces should exist only where they express a real ownership, volatility, lifecycle, or test boundary. An interface with one implementation is valid when it protects a meaningful seam; it is not automatically valid merely because mocking is possible.

## Coordinators and umbrella products

A coordinator may own sequencing, discovery, lifecycle supervision, shared events, or user-facing composition. It must not silently become the owner of every component it coordinates.

Umbrella products should preserve component contracts when components remain independently useful. Shared dependencies are allowed when ownership remains explicit. Independence does not require zero dependencies; it requires a coherent separately usable product boundary.

## Ownership conflicts

When ownership is disputed, resolve it by asking:

1. Which component has the information needed to enforce the invariant?
2. Which component controls authoritative mutation?
3. Which component must recover after partial failure?
4. Which component can evolve the behavior without coordinating unrelated callers?
5. Where would a new consumer naturally depend?

If the answers point to different owners, the proposed boundary is incomplete or the responsibility must be split more precisely.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Repository and component structure](repository-and-component-structure.md)
- [Data, processes, and protocols](data-processes-and-protocols.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)

## Notes

Shared terminology, schemas, or low-level libraries may be jointly consumed, but their evolution still requires a named owner. "Shared" is a usage pattern, not an ownership model.
