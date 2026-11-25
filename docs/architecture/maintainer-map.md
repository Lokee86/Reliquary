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
| Change file headers, physical replay, grouped publication clocks or format validation | [Storage](storage.md) | `reliquary-container/src/{lib,replay_scan,version_clock}.rs` and the eight store codecs | Container clock and selected-frame tests, store unit tests and independent reopen |
| Change writer admission, typed replay, receipt or history rules | [Owner-session](owner-session.md), [Storage](storage.md) | `reliquary-owner-session/src/{session,records}/` and `reliquary-memory-core/src/store.rs` for grouped Memory replay | Owner-session grouped/replay/admission tests and Memory Core grouped-creation validation |
| Add a named ingest, import, maintenance, reconciliation or management workflow | [Application](application.md) | `reliquary-application/src/*_workflow*` | Workflow publication/negative/reopen tests; `tests/g4_maintenance.rs` |
| Change copied Hermes Archive and Echo publication or exact source reads | [Public API](../public-product-api.md), [Application](application.md), [Storage](storage.md) | Root native import through Gateway grants, Application copied-source validation, and owner-session epoch publication | Root `tests/echo_capture.rs`, external copied Hermes extension test |
| Change cross-REL Entity discovery, evidence, pair judgment, receipts or reconsideration | [Public API](../public-product-api.md), [Semantic processing](semantic-processing.md), [Application](application.md), [Owner-session](owner-session.md), [Gateway](gateway.md) | Root pair listing, Entity resolver candidate index, Application pair evaluator, owner-session evidence and receipts, and Gateway discovery and reconsideration | [Public identity tests](../../tests/entity_identity.rs), [direct Memory witnesses](../../tests/entity_identity/mentions.rs), [automatic discovery](../../tests/entity_identity/discovery.rs) |
| Change user confirmation or undo for a cross-REL Entity pair | [Trusted-host API](../public-product-api.md#confirm-or-undo-separation), [Gateway](gateway.md), [Owner-session](owner-session.md) | Root operator admission, Gateway review and owner-session audit history | [Public review cases](../../tests/entity_identity/separation.rs) |
| Change Memory Web audit, Observation creation or review, or derived evidence retrieval | [Perception semantics](semantic-processing.md#observation-inference), [Application](application.md), [Owner-session](owner-session.md), [Gateway](gateway.md) | Shared Audit, Ego and Perception semantic crates, Application draft and routing workflows, native Observation records and managed REL worker | [Structured stages](../../crates/reliquary-perception/tests/structured_stages.rs), [Observation recovery](../../crates/reliquary-owner-session/src/session/checkpoint/tests_observations.rs) |
| Change Community recompute eligibility, solver, or publication | [Semantic processing](semantic-processing.md), [Owner-session](owner-session.md) | Graph scan-and-merge and owner-session capture/save via Application | Graph parity fixtures and Root `tests/community_management.rs` |
| Change REL dependencies or inherited context | [Owner-session](owner-session.md), [Gateway](gateway.md), [Application](application.md) | Declaration replay, mounted topology and authorized context preparation | Root `tests/rel_dependencies.rs` and `tests/g4_maintenance.rs` |
| Change Archive vector publication, activation, or exact and hybrid search | [Storage](storage.md), [Gateway](gateway.md), [Public API](../public-product-api.md) | Packed matrix and row mapping in vector-store, lexical codec and inference redaction in Archive, sealed generation projection in owner-session, authorized Gateway read and writer operation | Gateway publication and reopen test, Root staged import publication test, Archive redaction, codec and postings parity tests, owner-session interrupted publication and activation tests, Root Archive reference test |
| Change temporal Memory or Archive search | [Root temporal search](../public-product-api.md#temporal-search), [Semantic processing](semantic-processing.md) | Retrieval owns the filter; Application combines Chronos with authorized source evidence; Gateway owns delivery and recovery | Root retrieval, Archive ranking and Gateway recovery tests |
| Change trusted retrieval delivery outcomes or exact selection replay | [Public API](../public-product-api.md#same-rel-observations), [Gateway](gateway.md#external-host-contracts) | Root retrieval facade, Gateway prepared delivery and REL Freshness receipt | [Observation-only and mixed delivery tests](../../crates/reliquary-runtime-gateway/src/instance_retrieval/observation_checks/tests.rs) |
| Change model proposal evaluation, vector compatibility, retrieval, context or project composition | [Semantic processing](semantic-processing.md) | Semantic service crates and Application proposal workflows | Service tests, agentic retrieval and workflow tests |
| Change mounts, grants, authenticated Instances, worker lifecycle or queue/host handoff | [Gateway](gateway.md) | `reliquary-runtime-gateway/src/`; Root host adapters | Queue recovery, worker replay, revoke/auth and read-only PHY detach tests; retained-host consumer |
| Change provider routing, credentials, OS key protection, readiness or hosted inference injection | [Configuration reference](../configured-runtime.md), [Switchboard](switchboard.md) | `reliquary-switchboard`; Root configuration/runtime wrappers | Switchboard security, precedence, hosted identity and native binding tests; mounted Perception and `configured_runtime` external consumer |
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
