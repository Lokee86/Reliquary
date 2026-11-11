# Seams and Abstractions

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines when architectural seams and abstractions should be introduced, retained, simplified, or removed.

## Overview

A seam is a deliberate boundary where behavior, ownership, state, lifecycle, process, or implementation can change without forcing unrelated code to change with it.

The standard prefers early concrete seams where future ownership is credible. It rejects abstraction added only to make code appear layered or theoretically reusable.

## Add a seam when

A seam is justified when one or more of these conditions apply:

- a responsibility has a distinct owner or lifecycle;
- state mutation needs one enforceable boundary;
- behavior crosses a process, repository, language, or trust boundary;
- the implementation is volatile while the consumer contract is stable;
- a subsystem needs focused tests without booting its entire host;
- two consumers require the same policy rather than merely similar syntax;
- failure, retry, recovery, or observability must be centralized;
- a likely future capability needs an explicit home before mechanics expand.

## Defer mechanics, not ownership

When a likely capability is not fully implemented, create the smallest credible owner and contract rather than scattering placeholders through unrelated systems.

Examples include:

- adding health and damage ownership before introducing shields, resistances, or healing mechanics;
- creating a protocol owner before adding multiple transports;
- defining a lifecycle owner before adding restart, pause, or recovery behavior;
- establishing a persistence boundary before several call sites write directly to storage.

The initial implementation may remain simple. The architectural gain is that future behavior has somewhere correct to grow.

## Concrete before generic

Prefer:

- a small package with one responsibility;
- a narrow interface named after an actual consumer need;
- an explicit data structure or protocol message;
- direct dependency injection at a real boundary;
- one focused implementation with a clear replacement seam.

Avoid beginning with:

- framework-wide provider registries;
- generic service locators;
- multi-layer repositories around trivial storage;
- abstract factories without demonstrated construction variation;
- event buses used to conceal ordinary control flow;
- catch-all utility packages that accumulate domain policy.

Generalize only after repeated pressure demonstrates the shared invariant. Similar code is not automatically the same responsibility.

## Packages and files

A one-file package is acceptable when it establishes a useful ownership boundary. It should be folded back when the boundary proves false or adds more navigation than isolation.

A package should not exist solely to rename a call. It should own at least one of:

- policy or invariants;
- state or lifecycle;
- protocol or data contract;
- volatile implementation behind a stable boundary;
- independently meaningful verification;
- a coherent reusable capability.

Large catch-all packages should be split when they contain several independent mutation flows, lifecycles, protocols, or failure domains.

## Helpers and wrappers

Helpers and wrappers are late tools, not default architecture.

They are justified when they:

- enforce validation or ordering that callers must not bypass;
- isolate an external dependency or unstable API;
- provide one authoritative resource-management or recovery policy;
- preserve a compatibility boundary;
- remove duplicated policy rather than duplicated syntax.

They are suspect when they:

- forward arguments unchanged;
- hide which component owns the operation;
- convert explicit dependencies into ambient access;
- exist only to make tests mockable;
- create naming indirection without reducing change impact;
- duplicate another component's contract.

## Extension points

An extension point must define:

```text
Who may extend it
What is stable
What may vary
How implementations are discovered
How inputs and outputs are validated
How failures are isolated
How compatibility is maintained
How extensions are tested
```

Do not expose an extension point merely because future customization is imaginable.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Ownership and dependency direction](ownership-and-dependency.md)
- [Repository and component structure](repository-and-component-structure.md)
- [Architecture procedure](architecture-procedure.md)

## Notes

Removing an unnecessary seam should be straightforward when ownership never materialized. Extracting a missing seam after many callers depend directly on the wrong owner is usually more expensive.
