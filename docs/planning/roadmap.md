# Current planning owner

Parent index: [Planning](./INDEX.md)

## Purpose

Keep future corrections separate from implemented product behavior and dated acceptance evidence.

## Overview

The current public contract is owned by [API](../public-product-api.md), [CLI](../reference/cli.md), [configuration](../configured-runtime.md) and [architecture](../architecture/INDEX.md). Historical completion of individual rebuild gates does not establish future capability completeness.

## Current status and planned ownership

The [homogeneous-REL and dependency-DAG roadmap](../rel-homogeneity-and-dependency-roadmap-2026-11-10.md) is deferred. Current persisted `OwnerScope` labels and project routing must not be described as a verified arbitrary inter-REL dependency DAG. That correction spans owner metadata, host grants/routing, CLI compatibility and context composition; its original ADR evidence must be recovered and reviewed before implementation. Existing owners and corpus must remain preserved.

The [standards remediation specification](../engineering-standards-remediation-spec-2026-11-10.md) governs ES01–ES06. [Final local verification](../acceptance/es06-local-verification-2026-11-11.md) records completed local structural, policy, behavioral and independent source-to-document checks. Hosted CI and formal adoption remain unverified; no future product capability is inferred from this remediation.

Manual community recomputation follows the [Leiden specification](leiden-community-recomputation-spec-2026-11-18.md) and [execution plan](leiden-community-recomputation-plan-2026-11-18.md). The operator runs preview and apply through the existing REL management boundary.

## Implementation sequence and acceptance

For the homogeneity correction: inventory actual scope checks and missing graph behavior; recover the cited ADRs; specify authority/compatibility; implement focused independent changes; then verify old owner byte preservation, multi-parent/deep dependency closure, cycle rejection, sibling isolation and external host/CLI behavior. Do not change REL 2.1 fields or semantics as a documentation repair.

For standards remediation: restore inventory → canonical docs → coverage/status → architecture review/enforcement → continuous checks → independent final acceptance. The spec contains the full criteria. An unavailable required gate remains an open item, never a successful check.

## Open decisions

The scope-field compatibility transition and durable inter-REL dependency implementation/acceptance remain unresolved. Future format conversion, cross-owner/PHY reconciliation and additional provider or daemon surfaces require their own specification and acceptance. They are not promised by this roadmap.

## Related docs

- [Limits](../limits/current-limitations.md)
- [Document disposition](../development/document-disposition.md)
- [System](../architecture/system.md)

## Notes

Graduate implemented facts into their current owners and leave only unresolved work here. Do not present a historical specification as a current implementation guarantee.
