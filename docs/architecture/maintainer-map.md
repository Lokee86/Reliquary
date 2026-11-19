# Maintainer change routes

Parent index: [Reliquary architecture](./INDEX.md)

## Purpose

Route a maintainer's intended change to its canonical documentation, implementation authority and verification boundary.

## Overview

Start with the intent below, then read the owner page. This map does not define storage or runtime behavior. The [coverage map](../development/documentation-coverage.md) is the exhaustive package/contract inventory and [behavioral matrix](../development/behavioral-contracts.md) supplies critical negative/reopen checks.

## Change-area routing

| Change intent | Canonical owner | Primary implementation boundary | Verification surface |
| --- | --- | --- | --- |
| Change public owner handles, snapshots, error kinds or host facade | [Public API](../public-product-api.md) | `src/{owner,management,host,error}.rs`, `src/host*.rs` | `tests/public_api.rs`, `tests/oc06_inspection.rs`, and the Root-only external consumer |
| Change new REL creation or legacy scope compatibility | [Public API](../public-product-api.md), [Owner-session](owner-session.md), [CLI](../reference/cli.md) | Root and CLI entry points through the current REL writer | Root owner isolation, CLI create/verify, and verified copy fixtures |
| Change file headers, grouped publication clocks or format validation | [Storage](storage.md) | `reliquary-container`; eight store crates own their codecs | Container and store unit tests; independent reopen |
| Change writer admission, typed replay, receipt or history rules | [Owner-session](owner-session.md) | `reliquary-owner-session/src/{session,records}/` | Owner-session grouped/replay/admission tests |
| Add a named ingest, import, maintenance, reconciliation or management workflow | [Application](application.md) | `reliquary-application/src/*_workflow*` | Workflow publication/negative/reopen tests; `tests/g4_maintenance.rs` |
| Change Community recompute eligibility, solver, or publication | [Semantic processing](semantic-processing.md), [Owner-session](owner-session.md) | Graph scan-and-merge and owner-session capture/save via Application | Graph parity fixtures and Root `tests/community_management.rs` |
| Change REL dependencies or inherited context | [Owner-session](owner-session.md), [Gateway](gateway.md), [Application](application.md) | Declaration replay, mounted topology and authorized context preparation | Root `tests/rel_dependencies.rs` and `tests/g4_maintenance.rs` |
| Change model proposal evaluation, vector compatibility, retrieval, context or project composition | [Semantic processing](semantic-processing.md) | Nine semantic service crates; Application commits proposals | Service tests, agentic retrieval and workflow tests |
| Change mounts, grants, authenticated Instances, worker lifecycle or queue/host handoff | [Gateway](gateway.md) | `reliquary-runtime-gateway/src/`; Root host adapters | Queue recovery, worker replay, revoke/auth and read-only PHY detach tests; retained-host consumer |
| Change provider routing, credentials, OS key protection, readiness or hosted inference injection | [Configuration reference](../configured-runtime.md), [Switchboard](switchboard.md) | `reliquary-switchboard`; Root configuration/runtime wrappers | Switchboard security/precedence and hosted identity tests; `configured_runtime` external consumer |
| Change command family, argument, JSON result, exit behavior or user recovery | [CLI reference](../reference/cli.md), [Operator guide](../operator-workflow.md) | `cli/src/` through public Root API | `cli/tests/cli.rs`, `cli/tests/g4_corpus.rs` |
| Change exact production package edges or static architecture policy | [System](system.md), [Pitlord operations](../../tools/pitlord/README.md) | `architecture/target-crate-dag-2026-11-03.json`, independent DAG guard, `tools/pitlord/` | DAG guard; policy validation/evidence/check; negative controls |
| Change build/test/CI, fixtures, shared standards snapshot or local checks | [Development workflow](../development/workflow.md), [Documentation policy](../documentation-policy.md) | `.github/`, `scripts/`, `.standards/`, `docs-standard.json`, `tests/` | Local/CI gate and its negative controls; snapshot hash verification |
| Report unsupported behavior or propose future scope | [Limits](../limits/current-limitations.md), [Planning](../planning/roadmap.md) | Owning current subsystem; separately scoped correction | Reproduction and owner-specific acceptance before changing status |

## Cross-owner rules

Root exposes opaque supported contracts. Gateway authorizes and schedules; Application composes named workflows; owner-session alone provides concrete writable owner authority; durable stores and Container own their physical state. A new seam must identify authority, immutable/proposal values, commit owner and failure/retry contract. Test-only dependencies do not authorize production reach-through. Approved DAG changes require the authoritative graph, guard and current system documentation to change deliberately together.

## Related docs

- [System ownership](system.md)
- [Coverage](../development/documentation-coverage.md)
- [Documentation procedure](../documentation-procedure.md)

## Notes

Routes identify repository responsibilities rather than personal maintainers. No CODEOWNERS identity or external service ownership is inferred.
