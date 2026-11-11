# Repository and Component Structure

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines architectural standards for files, packages, libraries, applications, services, monorepos, umbrella products, and independently usable components.

## Overview

Repository structure should make ownership and dependency direction visible. It should not merely group files by language convention or create one directory for every noun in a design discussion.

The smallest useful boundary is preferred. Boundaries may grow as responsibility grows, but unrelated responsibilities should not be forced together for convenience.

## Files and packages

A file should have one coherent reason to change. A package should own a coherent responsibility, state boundary, contract, lifecycle, or reusable capability.

A package is too broad when it contains several independent:

- mutation authorities;
- lifecycles or schedulers;
- protocols or public contracts;
- persistence models;
- failure and recovery domains;
- extension mechanisms;
- product concepts that can evolve separately.

A package may be small or contain one file when its boundary is useful. File count is not an architectural metric.

## Internal libraries

An internal library is appropriate when several owners need the same low-level capability or when volatile implementation should be isolated behind one contract.

Internal libraries must avoid accumulating caller-specific policy. When consumers require meaningfully different behavior, split the policy from the shared mechanism or return ownership to the caller.

A shared library must have a named owner, release or compatibility policy when independently versioned, and focused tests for its contract.

## Applications and services

An application owns a user-facing or operator-facing lifecycle and composition boundary. A service owns a remotely or independently consumed capability with its own runtime lifecycle.

Do not split a service solely because a subsystem is conceptually distinct. Prefer a package or component boundary unless independent deployment, scaling, failure isolation, trust, runtime, or product use justifies process separation.

Services should not share a database schema as an undocumented private API. Shared storage requires explicit ownership and contract rules equivalent to any other protocol.

## Monorepos

A monorepo may contain several independently maintained components. Each significant component should identify:

```text
Product or capability boundary
Build and test boundary
Public contract
Owned state
Dependency direction
Release relationship
Independent-use claim, if any
Canonical documentation and maintainer map
```

Shared source does not remove the need for explicit cross-component contracts. Relative import convenience must not create dependency cycles that would be unacceptable across repositories.

## Umbrella products

An umbrella product may coordinate discovery, process lifecycle, shared repository watching, shared events, caches, UI composition, or common authentication.

It should preserve separately useful products when their independent value is real. It may depend on shared engines or libraries without erasing product boundaries.

The umbrella owns composition and supervision. Component products retain their domain contracts, state, and independent operating modes unless an explicit consolidation decision transfers ownership.

## Independently usable components

Claim independent usability only when a component has:

- a coherent user or consumer purpose;
- a usable entry point or API;
- a complete lifecycle and configuration model;
- owned state and failure behavior;
- build, test, and release support;
- documentation that does not require knowledge of the host product;
- compatibility expectations for external consumers.

Being located in a subdirectory or having a separate executable is insufficient.

## Cross-language structure

One normalized contract should be preferred over duplicate language-specific interpretations when several tools need the same semantic data.

A shared language-analysis or data engine may be a dependency of several products. Product independence does not require each product to reimplement adapters, parsers, schemas, or storage.

Language-specific code remains inside the owning adapter or runtime boundary. Cross-language normalization belongs to the shared contract owner.

## Generated and workspace state

Generated repository state, worktrees, indexes, build outputs, caches, runtime snapshots, and tool databases should use explicit ignored roots and must not be traversed as authored source unless a tool intentionally owns them.

Nested worktrees and generated mirrors must be excluded from scans, tests, formatters, documentation walkers, and file watchers by default.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Ownership and dependency direction](ownership-and-dependency.md)
- [Seams and abstractions](seams-and-abstractions.md)
- [Data, processes, and protocols](data-processes-and-protocols.md)

## Notes

Repository boundaries are deployment and ownership tools, not prestige. Moving code to another repository does not fix ambiguous responsibility; clear ownership should exist before and after the move.
