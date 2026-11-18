# Current limitations and verification gaps

Parent index: [Limits](./INDEX.md)

## Purpose

State supported-surface boundaries, known architecture debt and outstanding compliance evidence without hiding them in historical acceptance reports.

## Overview

This owner distinguishes current constraints from [future work](../planning/roadmap.md). Check the exact [public API](../public-product-api.md), [CLI](../reference/cli.md) and [configuration](../configured-runtime.md) before depending on a capability.

## Product and compatibility boundaries

Normal runtime supports REL/PHY 2.1. Old FRS07/2.0 readers and conversion are not a supported runtime surface. Mutable Container/store handles are not part of the public Root contract. There is no deployed network daemon or unrestricted provider administration API. Maintenance is an explicitly planned verified copy with independent reopen and no overwrite, not arbitrary in-place editing.

Fresh-history rewrite and divergent reconciliation fail closed on authenticated user Memory correction, retirement, association and Community naming commits where authority/request history cannot be preserved. Cross-owner reconciliation and public Root/CLI PHY reconciliation are unsupported. Internal Application verified-copy workflows can reconcile same-owner, same-kind REL/PHY prefix histories. Public REL reconciliation additionally supports approved divergent records through semantic replay, refusing unsupported authenticated user-operation tails; it is not an arbitrary merge. Treat unsupported operational evidence as a refusal requiring a separately accepted implementation, not permission to drop history. Protecting examples include `unsupported_operational_evidence_stops_before_output` in Application rewrite tests and Root/external-consumer management replay tests.

Model-dependent processing requires configured compatible endpoints. Unconfigured optional relationships remain unavailable; passive read/inspection is not confirmed host delivery and does not create consumption credit. Fixture tests establish bounded contracts, not the reliability or quality of a live external provider. The dated real-provider/corpus run is [historical evidence](../public-api-cli-operational-verification-2026-11-07.md), not a fresh live verification in this remediation.

Leiden recomputation is an explicit REL management operation. It covers active Memories in the stored graph closure. Eligible Memories with no structural neighbor form singleton groups. Exact lifecycle eligibility is enforced at save time. Reopen validates the recorded Graph cut and memberships under the 2.1 Community record rules.

## Known architecture debt

Existing `OwnerScope` values remain readable as header metadata. New REL creation is scope-free. Current inter-REL dependencies use durable owner IDs across all supported REL scopes. A port-aware 2.1 build is required for dependency-bearing files.

## Compliance evidence

The [initial ES01 inventory](../acceptance/es01-initial-audit-2026-11-11.md) measured 184 findings before canonical owners. No baseline suppresses them. [Final local verification](../acceptance/es06-local-verification-2026-11-11.md) records zero documentation findings, current 13-rule Pitlord ownership/dependency enforcement, Demon Docs, negative controls, Rust/CLI/external-consumer and fresh-process passes, plus independent semantic review. Hosted CI is unrun and formal adoption remains `audit-pending`. The focused per-path configuration lock test does not prove all high-level credential-writer/migration interleavings; static analysis does not prove runtime invariants. These limits and the deferred architecture follow-up prevent an unqualified full-product compliance claim.

## Related docs

- [Behavioral contracts](../development/behavioral-contracts.md)
- [Architecture audit](../acceptance/es04-architecture-audit-2026-11-11.md)
- [Documentation status policy](../documentation-policy.md)

## Notes

This remediation changes documentation and governance, preserving runtime code. Report a newly observed violation with precise reproduction, implementation owner, severity and focused correction rather than refactoring speculatively.
