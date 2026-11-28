# Implementation documentation coverage

Parent index: [Development](./INDEX.md)

## Purpose

Account for every production package and cross-cutting public, persistent and stateful boundary, routing each to substantive current documentation.

## Overview

The [maintainer map](../architecture/maintainer-map.md) routes change intents. This inventory accounts for all 23 approved workspace packages plus the separately shipped CLI. The [system owner](../architecture/system.md) explains composition and the executable [DAG](../../architecture/target-crate-dag-2026-11-03.json) defines approved production edges. Tests are protecting surfaces, not evidence of a fresh pass; measured outcomes belong in [acceptance](../acceptance/INDEX.md).

## Production package inventory

Paths below are relative to the repository root. A focused owner serves several small store/semantic packages where they share responsibility; there is no page-per-file requirement.

| Package / code root | Canonical owner | Verification boundary |
| --- | --- | --- |
| `reliquary-memory` — `src/` | [Public API](../public-product-api.md), [system facade](../architecture/system.md) | `tests/public_api.rs`, `tests/external-consumer/` |
| `reliquary-container` | [Storage](../architecture/storage.md) | `crates/reliquary-container/src/lib.rs` covers selected-frame replay, physical clock continuity and skipped payload I/O. Grouped-clock tests cover range allocation. |
| `reliquary-chronos` | [Semantic time interpretation](../architecture/semantic-processing.md) | `crates/reliquary-chronos/src/tests.rs`, `chronos_analysis.rs` (time parse/detection/inference) |
| `reliquary-archive` | [Storage / Archive](../architecture/storage.md), [Owner-session](../architecture/owner-session.md) | `archive_tests.rs`, `echo_tests.rs`, inference redaction tests in `embedding_redaction.rs`, and the lexical postings and codec tests in `lexical.rs` and `lexical/codec.rs` |
| `reliquary-memory-core` | [Storage / Memory](../architecture/storage.md) | crate `src/tests.rs` covers retained revisions, body validation and rejection of partial groups |
| `reliquary-graph` | [Storage / Graph](../architecture/storage.md) | crate `src/lib.rs`, `dream_batch.rs`, `association.rs`, `community_names.rs`, and scan-and-merge tests |
| `reliquary-memory-audit`, `reliquary-ego`, `reliquary-perception` | [Observation inference](../architecture/semantic-processing.md#observation-inference) | Combined discovery validation, scoped lexical and graph lookups, PHY settled-mode evidence, synthesis and receptors, and the two native Jev contracts |
| `reliquary-ego` summary inference | [Ego summary verification](../architecture/semantic-processing.md#ego-summary-verification) | [REL summary tests](../../crates/reliquary-ego/src/rel_summary_tests.rs), [PHY summary tests](../../crates/reliquary-ego/src/phy_summary_tests.rs) |
| `reliquary-perception-store` | [Storage / Perception](../architecture/storage.md) | crate history and relationship replay/codec tests |
| `reliquary-ego-store` | [Storage / Ego](../architecture/storage.md), [Owner-session](../architecture/owner-session.md) | `src/tests/summary.rs` covers verified summary revisions, atomic completion, suppression, and Identity selection. Owner-session covers PHY replay and exact copy. |
| `reliquary-freshness-store` | [Storage / Freshness](../architecture/storage.md) | crate `src/` tests and Freshness workflow reopen tests |
| `reliquary-vector-store` | [Storage / Vectors](../architecture/storage.md) | crate `src/tests.rs`, generation/rebuild tests |
| `reliquary-insomnia-store` | [Storage / Insomnia](../architecture/storage.md) | crate `src/tests.rs`; claim/completion replay |
| `reliquary-owner-session` | [Owner-session](../architecture/owner-session.md) | crate `src/session/` and `src/records/` grouped admission/replay tests |
| `reliquary-application` | [Application](../architecture/application.md) | Named workflow publication, maintenance, import, management and [shared REL and PHY draft review tests](../../crates/reliquary-application/src/observation_drafts/tests.rs) |
| `reliquary-runtime-gateway` | [Gateway](../architecture/gateway.md) | queue/host/worker/authentication tests; retained-host consumer |
| `reliquary-switchboard` | [Switchboard](../architecture/switchboard.md), [configuration](../configured-runtime.md) | crate credential/key/route/provider tests, credential-free hosted route persistence/binding tests; external `configured_runtime` |
| `reliquary-insomnia` | [Semantic processing](../architecture/semantic-processing.md) | crate evaluator/routing tests; native worker reopen tests |
| `reliquary-dream` | [Semantic processing](../architecture/semantic-processing.md) | crate evaluator tests; Application Dream grouped publication tests |
| `reliquary-entity-resolution` | [Semantic processing](../architecture/semantic-processing.md) | crate evaluator/fast-path tests; entity worker replay |
| `reliquary-vectors` | [Semantic processing](../architecture/semantic-processing.md) | crate `contract_tests.rs`; profile/compatibility tests |
| `reliquary-freshness` | [Semantic processing](../architecture/semantic-processing.md) | crate state/transition tests; Freshness worker reopen |
| `reliquary-retrieval` | [Semantic processing](../architecture/semantic-processing.md) | crate candidate/ranking tests; Application agentic retrieval |
| `reliquary-context-engine` | [Semantic processing](../architecture/semantic-processing.md) | crate `behavior_tests.rs`; retained-host context consumer |
| `reliquary-project-environment` | [Semantic processing](../architecture/semantic-processing.md) | crate composition tests; project-source consumer |
| `reliquary-cli` — `cli/` (separate Cargo workspace) | [CLI](../reference/cli.md), [operator workflow](../operator-workflow.md) | `cli/tests/cli.rs`, `cli/tests/g4_corpus.rs` |

Unless otherwise stated, crate code roots are `crates/<package>/`. The `quinn-proto` vendor patch is covered by the system architecture and the build review. Verify upstream security fixes against the exact vendored source and run the locked workspace and Lore project-history checks. Test-only dev dependencies do not change production ownership.

## Public and machine-readable contracts

| Surface | Current owner | Source and protecting tests |
| --- | --- | --- |
| Root cross-REL Entity discovery, pair listing, comparison and receipt freshness | [API](../public-product-api.md), [Application](../architecture/application.md), [Gateway](../architecture/gateway.md), [Owner-session](../architecture/owner-session.md), [Semantic processing](../architecture/semantic-processing.md) | [Public tests](../../tests/entity_identity.rs) cover people, same-named programs, policy upgrades and publication races. [Evaluator tests](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs) cover paraphrases, exact citations and uncertainty. [Discovery tests](../../tests/entity_identity/discovery.rs) cover names, aliases, new identifiers, owner grants and paged listing. [Direct mention tests](../../tests/entity_identity/mentions.rs) cover automatic reconsideration and retained citations without Relationships. The [policy 4 copy trial](../acceptance/cross-rel-entity-identity-2026-11-23.md#direct-memory-evidence-policy-4) records the configured model result on migrated RELs. The historical [one-call policy check](../acceptance/cross-rel-entity-identity-2026-11-23.md#one-call-comparison-policy-3) records replay and live results. |
| Root exports, opaque `Rel`/`Phylactery`, immutable info, error taxonomy and authorized vector-presence summary | [API](../public-product-api.md), [Gateway](../architecture/gateway.md) | `src/lib.rs`, `src/{owner,error,retrieval}.rs`, Gateway read surface; `src/retrieval/tests.rs`, `tests/public_api.rs` |
| Trusted-host admission, grants, revocation, queue, stream, confirmed delivery | [Gateway](../architecture/gateway.md), [API](../public-product-api.md) | `src/host*.rs`, Gateway; `tests/external-consumer/tests/retained_host/` |
| Scope-free REL creation and read-side scope metadata | [API](../public-product-api.md), [Owner-session](../architecture/owner-session.md), [CLI](../reference/cli.md) | `tests/g4_public_product.rs`, `cli/tests/cli.rs`, and the fresh-process creation cases in `scripts/run_g4_acceptance.ps1` |
| CLI owner info/verify; Memory/history/proof; copy/maintenance/reconcile | [CLI exact contract](../reference/cli.md) | `cli/src/`; CLI parser/read-only/collision tests |
| CLI import/process/status/search/interaction and configuration families | [CLI](../reference/cli.md), [operator](../operator-workflow.md), [config](../configured-runtime.md) | CLI dispatch and configured runtime; import resume and inspection tests |
| Copied Hermes Echo and exact source evidence | [Application](../architecture/application.md), [Storage](../architecture/storage.md), [API](../public-product-api.md) | Application copied-source validation, Gateway grants, and owner-session epoch writer; Root `tests/echo_capture.rs` and external copied Hermes extension test |
| Configured Archive vector publication | [Public API](../public-product-api.md), [Storage](../architecture/storage.md) | Root `src/retrieval/archive_publication.rs` and configured publication test |
| Normalized import and copied Hermes source identity/digest/extension/terminal notice | [Application](../architecture/application.md), [API](../public-product-api.md) | `src/native_import.rs`, Application native import; `tests/native_import.rs`, external `operational_processing/hermes_copy.rs` |
| Live copied-source author and PHY admission, hold and recovery | [API](../public-product-api.md#native-and-copied-source-import-contracts), [Gateway](../architecture/gateway.md), [Storage](../architecture/storage.md) | Gateway `insomnia_worker/tests.rs` exercises mixed routing, interrupted publication, owner mismatch and credential revocation |
| Archive vector and passage publication and receipt during a staged native import | [API](../public-product-api.md), [Owner-session](../architecture/owner-session.md), [Gateway](../architecture/gateway.md) | `tests/native_import.rs::archive_vectors_publish_through_a_staged_import_without_processing` |
| Configuration file schema, precedence, model slots, endpoint compatibility, credential protection and host-owned inference injection | [Configuration](../configured-runtime.md), [Switchboard](../architecture/switchboard.md) | Root config/runtime wrappers and Switchboard; hosted identity and native Perception binding tests, mounted Perception fixture and external `configured_runtime.rs` |
| CLI JSON/error/exit, cut-bound inspection pages and direct Memory summaries | [CLI](../reference/cli.md), [API](../public-product-api.md) | `cli/src/`, `src/management.rs`; `tests/oc06_inspection.rs` |
| REL/PHY header, owner identity/generation, grouped records, typed replay, historical proofs | [Storage](../architecture/storage.md), [owner-session](../architecture/owner-session.md) | Container, eight stores, owner-session; corruption/replay/history tests |
| Root operator review, separation confirmation and undo | [Trusted-host API](../public-product-api.md#confirm-or-undo-separation), [Gateway](../architecture/gateway.md) | [Operator interaction and pair inspection tests](../../tests/entity_identity/separation.rs) |
| Entity separation confirmation audit in REL checkpoints, historical proofs and maintenance | [Owner-session](../architecture/owner-session.md) | [Owner-session tests](../../crates/reliquary-owner-session/src/session/checkpoint/tests_identity_separation.rs) cover existing 2.1 admission, exact and suffix replay, proof cuts, copy and pruning parity, and refusal before creating an unsupported rewrite or reconciliation destination |
| Offline REL and PHY history migration and independent parity | [Migration specification](../planning/owner-history-migration-spec-2026-11-18.md), [Storage](../architecture/storage.md) | `tools/d01-converter/`, `tools/d02-parity/`, native clocked Archive and PHY history fixtures, separate-process CLI and owner-grant check |
| Approved machine-readable package DAG | [System](../architecture/system.md) | `architecture/target-crate-dag-2026-11-03.json`; `scripts/check_architecture.py` |
| Authored G4 fixture JSON schema | [Development workflow](workflow.md), [historical G4 evidence](../g4-product-corpus-acceptance-2026-11-06.md) | `tests/fixtures/g4-native-rel21-v1.json`; `tests/g4_manifest.rs` |
| Documentation policy, synchronized manifest, Pitlord rule composition and CI contract | [Documentation policy](../documentation-policy.md), [Pitlord operations](../../tools/pitlord/README.md), [workflow](workflow.md) | `docs-standard.json`, `.standards/manifest.json`, `tools/pitlord/`, `.github/`, gate negative controls |

## Stateful, mutation, concurrency and recovery coverage

| Boundary | Canonical owner | Behavioral verification route |
| --- | --- | --- |
| Current Observations share Memory retrieval and retain exact citations | [Semantic processing](../architecture/semantic-processing.md), [Owner-session](../architecture/owner-session.md) | [Paraphrase, shared limit, review and reopen](behavioral-contracts.md) |
| Active PHY and optional REL ordinary recall, exact selection return, and retained Observation inspection | [Public API](../public-product-api.md#host-admitted-chat-retrieval), [Gateway](../architecture/gateway.md#external-host-contracts) | [Mixed recall, standing, switch fencing and read-only PHY delivery](behavioral-contracts.md) |
| Copied-source authority governs Memory, graph, audit and Observation visibility | [Owner-session](../architecture/owner-session.md) | [Provenance, historical citations, independent successor and reopen](behavioral-contracts.md) |
| Single writer, immutable reads, accepted evidence, grouped durable commit and typed reopen | [Owner-session](../architecture/owner-session.md) | [Atomicity, authority and history contracts](behavioral-contracts.md) |
| Ingest/import epochs, pinned processing profiles, REL/PHY routed writes, Dream cohort verification, grouped birth population replay and ordered retries, idempotent stage completion | [Application](../architecture/application.md), [Owner-session](../architecture/owner-session.md), [Gateway workers](../architecture/gateway.md) | Native import source, postprocessing, and Dream batch tests. The [large Dream cohort test](../../crates/reliquary-owner-session/src/session/checkpoint/tests_dream_suffix.rs) checks compact carrier size, shared historical populations during publication, and cold and checkpoint replay parity. The [configured import fixture](../../tests/external-consumer/tests/operational_processing.rs) covers inference route changes, embedding compatibility, and reopen. |
| Checked user Memory creation/correction/retirement/association and Community naming | [API](../public-product-api.md), [Application](../architecture/application.md) | Root management tests and external checked/mixed-write consumer suites |
| Cross-REL Entity identity keeps owner-qualified endpoints and publishes only in the recording REL; inspection checks both grants and reports receipt freshness | [Public API](../public-product-api.md), [Gateway](../architecture/gateway.md), [Owner-session](../architecture/owner-session.md) | Root `tests/entity_identity.rs` covers namesakes, automatic reconsideration after correction and reopening, dual grants, stale evaluation/revocation, read-only peer and concurrent equivalent requests |
| Community recompute uses exact active membership and a version-checked, durable commit | [API](../public-product-api.md), [Owner-session](../architecture/owner-session.md) | Root `tests/community_management.rs` and copied-REL acceptance |
| REL dependency declarations survive reopen and approved maintenance. Context uses a bound authorized closure | [API](../public-product-api.md), [Gateway](../architecture/gateway.md), [Storage](../architecture/storage.md) | Owner-session declaration replay, Root `tests/rel_dependencies.rs`, and maintenance acceptance |
| Queue durable intents, unknown host status, cancellation/finalization, generation stream checkpoints | [Gateway](../architecture/gateway.md) | Queue corruption/incomplete-tail/held-status tests, retained-host conversation |
| Mount lifetime, grant isolation, revocation during slow model/host work, worker stop/restart | [Gateway](../architecture/gateway.md) | Gateway auth/retrieval/worker lifecycle tests, Root read-only PHY detach, and consumer grant rejection |
| Archive packed exact and hybrid search and default exact Memory ranking | [Storage](../architecture/storage.md), [Public API](../public-product-api.md), [Semantic processing](../architecture/semantic-processing.md), [Gateway](../architecture/gateway.md) | `tests/public_api.rs`, Gateway `instance_vectors/tests.rs` and `instance_retrieval/inspection_tests.rs`, Retrieval `memory_retrieval.rs` |
| Temporal Memory and Archive search, per-query windows, and delivery recovery | [Root API](../public-product-api.md#temporal-search), [CLI](../reference/cli.md), [Temporal search specification](../planning/temporal-search-spec-2026-11-23.md) | Root retrieval, Archive ranking, agentic recovery and CLI tests |
| Turn-scoped chat retrieval and exact Archive and Echo disclosures | [Root API](../public-product-api.md#temporal-search), [Application](../architecture/application.md) | `src/retrieval/chat.rs`, `crates/reliquary-application/src/temporal_query.rs`, external copied Hermes extension test |
| Retrieval selection/refinement/traversal, passive versus confirmed consumption, owner-local Freshness | [Semantic processing](../architecture/semantic-processing.md), [Gateway](../architecture/gateway.md) | Agentic retrieval and confirmed-delivery replay tests |
| External source authorization, snapshot/change feed/hydration and project binding | [Application](../architecture/application.md), [Gateway](../architecture/gateway.md) | Dual-authorization retained-host external/project-source tests |
| Rewrite/prune/reclaim/reconcile, source freeze, destination collision, unsupported authority and independent reopen | [Application](../architecture/application.md), [operator](../operator-workflow.md) | Root maintenance, workflow negative/reopen and CLI copy tests |
| Config per-path write exclusion, stale handles, OS key protection, missing credentials and readiness | [Switchboard](../architecture/switchboard.md), [config](../configured-runtime.md) | Switchboard `protected_config/tests/key_lock.rs` exclusion test, stale-handle/key/crypto/provider tests and offline consumer; full concurrent credential-writer/migration interleavings remain outside that primitive-level proof |
| Managed REL Ego shared and independent audit scheduling | [Gateway](../architecture/gateway.md) | [Mounted Ego fixtures](../../crates/reliquary-runtime-gateway/src/instance_perception/tests/ego_pair.rs) |
| Shared audit handoff, Perception verification, exact delivery confirmation and replay | [Semantic processing](../architecture/semantic-processing.md#observation-inference), [Application](../architecture/application.md), [Gateway](../architecture/gateway.md), [Public API](../public-product-api.md#same-rel-observations) | [Ego handoff tests](../../crates/reliquary-ego/src/tests.rs), [Draft tests](../../crates/reliquary-application/src/observation_drafts/tests.rs) and [Gateway retrieval tests](../../crates/reliquary-runtime-gateway/src/instance_retrieval/observation_checks/tests.rs) |
| REL checkpoint admission, suspended import recovery and leased Echo reads | [Owner sessions](../architecture/owner-session.md) | Checkpoint tests cover exact restore, supported and unsupported import suffixes, suspension across Dream frontier, batch and settlement, on-demand Echo evidence, and full replay parity. The [Dream checkpoint test](../../crates/reliquary-owner-session/src/records/dream_checkpoint.rs) checks encoding size, shared historical populations, exact Begin reads, and pending settlement after restore. |
| Physical Archive import receipt position and epoch ordering | [Archive](../architecture/storage.md), [Owner sessions](../architecture/owner-session.md) | [Native import replay tests](../../crates/reliquary-owner-session/src/session/native_import_tests.rs) cover orphan receipts, physical-position validation, and the checkpointed legacy epoch fence. [Archive tests](../../crates/reliquary-archive/src/archive_tests.rs) cover source mapping to accepted nodes, branches and files. |
| REL Observation publication and recovery | [Owner sessions](../architecture/owner-session.md) | [Owner-session tests](../../crates/reliquary-owner-session/src/session/checkpoint/tests_observations.rs) cover lineage and standing, retry, historical proofs, exact and suffix reopen, verified copy, and refusal of unclocked carriers. |
| Ego pending work, mounted publication and context | [Ego API](../public-product-api.md#ego), [Gateway](../architecture/gateway.md) | [Pending work](../../crates/reliquary-application/src/ego_pending.rs), [PHY preparation](../../crates/reliquary-application/src/ego_phy_tests.rs), [Mounted pair](../../crates/reliquary-runtime-gateway/src/instance_perception/tests/ego_pair.rs), [Controls](../../tests/ego_controls.rs) |

## Related docs

- [Behavioral contracts](behavioral-contracts.md)
- [Document disposition](document-disposition.md)
- [Known limits](../limits/current-limitations.md)

## Notes

Coverage is a routing obligation, not an assertion that every possible behavior is tested or every test just passed. New independent flows, public schemas and recovery seams must update this inventory and their substantive owners in the same change.
