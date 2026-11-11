# Architecture Standard

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines the shared architectural standard for Laughing Skull projects.

## Overview

Architecture is the assignment of responsibility, state, dependency direction, lifecycle, failure behavior, and change boundaries. A design is not architecturally complete merely because its code is organized into folders or because an interface exists.

The standard favors concrete ownership and useful seams over accidental coupling, speculative abstraction, and convenience-driven centralization. It applies most strongly to new systems, new durable responsibilities, cross-component changes, stateful flows, and significant refactors.

## Scope and enforcement status

These rules are normative for engineering work, architectural review, and repository policy.

Pitlord is the expected deterministic architecture-enforcement mechanism. Repositories encode their own ownership areas, dependency direction, forbidden coupling, cycles, and related static invariants in repository-local Pitlord policy. The shared standard supplies the enforcement model and reusable adoption policy, not one universal area graph.

Behavioral, lifecycle, concurrency, failure, recovery, and compatibility invariants that cannot be proven from static repository or graph evidence remain protected by focused tests, contract fixtures, runtime scenarios, and release gates.

Architectural enforcement is adopted repository by repository. Adding Pitlord to this standard does not authorize unreviewed bulk policy propagation. Repository-local architecture documents and ADRs may specialize these rules; exceptions must be explicit and justified, and adopted standards may not be silently weakened.

## Core rules

1. **Every durable responsibility has one clear owner.** Shared use does not imply shared ownership.
2. **Every mutable state domain has one authoritative mutation boundary.** Readers may be distributed; authority may not be ambiguous.
3. **Dependency direction is explicit.** Higher-level policy does not depend on lower-level details through hidden callbacks, globals, generated imports, or convenience cycles.
4. **Public boundaries are narrower and more stable than internal implementation.** Internal layout must remain changeable without forcing unrelated consumers to migrate.
5. **Useful seams are introduced before coupling hardens.** Add a seam when ownership, lifecycle, process, volatility, or independently testable behavior justifies it.
6. **Defer mechanics, not ownership.** A future capability may begin with minimal behavior, but its state and responsibility should already have a credible home.
7. **Concrete seams precede generic abstraction.** Prefer a small package, explicit interface, protocol, or data boundary over a framework-shaped layer with no proven consumer pressure.
8. **Helpers and wrappers require architectural value.** They must enforce an invariant, isolate volatility, reduce coupling, centralize unavoidable policy, or make a boundary testable.
9. **State transitions and lifecycle are designed explicitly.** Creation, activation, mutation, failure, retry, shutdown, replacement, and recovery are not incidental control flow.
10. **Cross-process and externally consumed contracts are versioned and validated.** Compatibility behavior is part of the contract.
11. **Authored state and generated state remain distinct.** Generated caches, indexes, snapshots, artifacts, and runtime state are reproducible or recoverable from named authorities.
12. **Failure behavior is part of the architecture.** Timeouts, partial availability, retry, idempotency, rollback, degradation, and operator recovery are defined at the owning boundary.
13. **Observability follows ownership.** Logs, traces, metrics, health, and diagnostics identify the responsible component and preserve correlation across boundaries.
14. **Architectural invariants have protecting tests or gates.** A prose-only invariant is an aspiration unless the design makes violations visible.
15. **Statically detectable architectural invariants are encoded in Pitlord.** Ownership gaps, forbidden dependencies, cycles, bypasses, and repository-policy violations should fail deterministically when reliable evidence exists.
16. **Migrations preserve explicit authority.** Transitional systems identify which store, service, protocol, or path is authoritative at each stage.
17. **Independent components are created for independent value, not aesthetic decomposition.** A component is independently usable only when it has a coherent contract, lifecycle, and consumer value outside its current host.
18. **Exceptions and consequential tradeoffs are recorded.** Significant irreversible, cross-cutting, or surprising choices require an architectural decision record.

## Minimum architectural evidence

A substantial architecture or architectural change must identify:

```text
Responsibility and owner
Non-ownership boundaries
Dependency direction
Public and internal contracts
State and source-of-truth ownership
Lifecycle and mutation flow
Concurrency or scheduling model when applicable
Failure, retry, shutdown, and recovery behavior
Observability and diagnostics
Migration and compatibility behavior when applicable
Protecting tests, Pitlord rules, or release gates
Known limits, tradeoffs, and explicit exceptions
```

The evidence may be distributed across focused architecture, protocol, operations, data, development, and policy documents. It must not exist only in conversation, issue comments, code layout, or an unexplained policy rule.

## Architectural evaluation

A design should be challenged when:

- two components can both plausibly claim the same state or responsibility;
- a new feature has no owning subsystem and is being threaded through unrelated code;
- a package or service exists only to move calls without enforcing a boundary;
- a shared utility accumulates policy from several unrelated domains;
- a protocol or persistent format can change without version or compatibility handling;
- shutdown, retry, recovery, and partial failure are left to callers to improvise;
- generated state is edited as though it were source;
- an architectural invariant lacks a focused test or deterministic gate;
- a statically detectable ownership or dependency invariant is repeatedly reviewed manually instead of encoded in Pitlord;
- a migration requires indefinite dual authority;
- the proposed abstraction is more general than the demonstrated problem.

## Relationship to documentation

Architecture must be documented according to the shared documentation standard. Maintainer maps route to canonical owners; focused code maps identify implementation and tests; neither substitutes for explaining ownership, flow, state, invariants, and failure behavior.

Pitlord policy must link back to those canonical owners through rule descriptions, repository policy documentation, or the local Pitlord operations guide. Machine enforcement without an explained architecture owner is not sufficient evidence of a sound design.

## Related docs

- [Ownership and dependency direction](ownership-and-dependency.md)
- [Seams and abstractions](seams-and-abstractions.md)
- [State, lifecycle, and concurrency](state-lifecycle-and-concurrency.md)
- [Data, processes, and protocols](data-processes-and-protocols.md)
- [Architectural enforcement with Pitlord](enforcement.md)
- [Architecture procedure](architecture-procedure.md)

## Notes

These rules intentionally allow small concrete boundaries early. Folding an unnecessary one-file package back into its caller is usually cheaper than extracting a mature responsibility after coupling has spread through the system.