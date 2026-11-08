# Public API / CLI operational gap audit — 2026-11-07

Parent: [operational completion specification](public-api-cli-operational-completion-spec-2026-11-07.md). Interaction details: [query and CLI contract](public-api-cli-interaction-contract-2026-11-07.md).

**Status: source-backed gap inventory, not implementation acceptance.** Inspected rebuild HEAD `a3c3a9643f494bced51faf92ba057b16bcdac8a7` plus the existing uncommitted two-day smoke changes. Those changes are not part of the accepted baseline. No runtime source was changed for this specification.

## Purpose

Determine which operations a real consumer can reach through the supported `reliquary-memory` Root package, rather than assuming that `pub` methods in an internal crate are product APIs. The same inventory governs the standalone CLI and any embedding application. This is unfinished product integration, not D01 old-file conversion or an HP/HU Hermes deliverable.

## Overview

There are three independently relevant facts: an internal implementation exists, the public product exports a complete usable call chain, and a consumer has executed that chain successfully. G4 and the earlier package tests do not collapse those into one fact. The evidence below supports presence/absence of source entry points, not a new full behavioral verification.

The existing [P01 map](p01-consumer-contract-map.md) intentionally assigned import, vectors and Insomnia to hosts instead of the CLI. The [public API guide](public-product-api.md) repeats that policy. The new specification supersedes that exclusion: the CLI is a trusted local host and must use the same product methods. Historical acceptance records remain historical evidence; they are not rewritten to imply that this new closure already passed.

## Capability inventory

| Capability | Existing implementation / source evidence | Root and CLI gap | Completion owner |
| --- | --- | --- | --- |
| REL/PHY creation and current-format admission | `src/owner.rs`; `src/host.rs`; public API tests | Existing public creation/opening retained. CLI creates REL only; paired PHY creation/binding must be usable without another executable. No old-format runtime reader. | OC01, OC06 |
| Host identity, grants, mounts and revocation | `src/host.rs`, `host_admission.rs`, `host_instance.rs` | Public values exist. New operations must enforce View/Operate/Manage distinctions, not export the less-restricted internal Instance. Mounted-owner information must not open a competing writer/read path. | OC01 |
| Provider transport, persisted config and route policy | Switchboard transports and accepted reference config owner: `CVCFG` framing, route codecs, encrypted credentials, master key, device login/refresh | Rebuild omitted the existing `reliquary.cfg` bootstrap. OC02 must restore that exact persisted boundary and CLI config management; the unaccepted JSON/env-only loader is superseded. Existing transports/fallbacks stay canonical. | OC02 |
| Normalized native import | Application `native_import_workflow.rs`; owner `native_import_writer.rs`, `native_import_work.rs` | No Root import operation or CLI importer. A source adapter is required for the ChatGPT export, but may only normalize source data; no Memory/Graph construction in CLI. | OC03 |
| Import-scoped extraction publication | Application `ingest_publication.rs` and owner `native_import_work.rs` | New local changes add prepared-import publication, scope validation and claim handling; treat these as candidate fixes needing public-API coverage, not accepted completeness. | OC03 |
| Insomnia and user-Memory routing | Runtime `insomnia_worker.rs`, `instance_routing.rs`; Application native extraction/evidence and publication | Internal worker entry points exist; Root has no usable configured processing handle. Full import drain includes real routing and source-grounded publication, not only queued Episodes. | OC04 |
| Dream / Entity / vectors / Freshness | Runtime `workflow_workers.rs`, `instance_dream.rs`, `instance_entity.rs`, `instance_vectors.rs`; Application workflows | Root lacks the complete named orchestration/control surface. Historical import requires first-cycle Dream evidence before settlement; a worker reporting idle is not proof that every required stage completed. | OC04 |
| Passive Memory search | Runtime `instance_passive_retrieval.rs`: `rank_rel_memories`, `route_rel_memories`; Application `owner_retrieval.rs` | No product Instance wrapper or CLI query command. Ordinary callers should provide text and options, not fabricated ranking evidence or hand-produced query vectors. | OC05 |
| Agentic query / refine / traverse | Runtime `instance_retrieval_prepare.rs`; Application `agentic_retrieval.rs`, `agentic_traversal.rs` | Public Root exports neither the usable controller/route nor the query operations. Query, refinement and traversal must share real turn state through the product boundary. | OC05 |
| Retrieval display versus confirmed use | Runtime `instance_retrieval.rs`, `instance_retrieval_delivery.rs`, `instance_retrieval_recovery.rs` | Existing prepared delivery is opaque and can leave a pending handoff. A CLI inspection path cannot fake delivery to advance it. Add bounded authorized inspection and cancellation/preview transitions without Freshness credit; expose actual handoff/recovery separately. | OC05 |
| Memory listing, exact lookup and source provenance | `src/management.rs`; Application `management_workflow.rs` | Bounded REL list/get/provenance exists. CLI exposes history summaries, not the complete read/inspection workflow. Expand provenance to exact accepted source/revision evidence where needed; do not manufacture it from a title or current text. | OC06 |
| Graph / Entity / relationship / community inspection | Accepted owner/semantic values; runtime semantic operations found in `instance_entity.rs` and `instance_perception_lifecycle.rs` | No complete bounded product inspection/CLI path. Query traversal is not a replacement for deterministic edge/neighborhood inspection. Missing read methods need narrow immutable projections. | OC06 |
| Supported user-origin corrections | Root direct Memory creation; runtime semantic retirement and relationship methods | Some named mutation paths exist below Root. Expose supported typed operations with expected revision/grants; do not add a generic raw-store or arbitrary graph edit interface. Any operation without an accepted owner primitive is recorded separately, not silently claimed. | OC06 |
| Native conversations and durable streams | `src/host_instance.rs` exposes submit/status/finalize/cancel and stream operations | Partially available already; audit complete execution, Episode finalization and restart through Root. CLI must support bounded feed/status/stream controls; a new general chatbot is not required. | OC07 |
| Context and compaction | Runtime `instance_context/prepare.rs`, `instance_context/finalize.rs`; Application context workflow | Existing accepted context operations are not exported coherently by Root. Expose those operations without claiming the future full Context Engine or Ego synthesis is implemented. | OC07 |
| External source manifest/index/hydration | Runtime `instance_external_source.rs` and its feed/publication/project-attachment modules | Existing trusted-host source interactions need named public exports and bounded inspection where retained. A missing live source must stay unavailable; a new Hermes-specific source provider is not part of this work. | OC07 |
| History / maintenance / reconciliation | Root `owner.rs`, `maintenance.rs`; existing CLI commands | Already substantially exposed. Retain current behavior, negatives and no-replace guarantees; do not implement a second maintenance API. | OC09 regression gate |

## Retrieval semantics that must not be redesigned during exposure

The [accepted retrieval record](retrieval-confirmed-use-acceptance-2026-11-05.md) distinguishes passive hybrid ranking, semantic community routing and agentic selection. It specifies REL-scoped agentic retrieval, query candidate union/Freshness ordering, complete-query refinement over previously searched candidates, concurrent one-hop traversal and confirmed external consumption.

Source inspection confirms that `evaluate_retrieval` and `rank_owner_snapshot` are different paths. Exposing them is not permission to silently combine them into a new retrieval algorithm, turn refinement into another global search, or apply a caller-supplied rank as authority. OC05 must run the accepted behavior cases through Root and document any real discrepancy against the governing retrieval contract before changing behavior.

Agentic PHY inclusion, dependency-wide merged retrieval and arbitrary SQL/graph query languages are not established by this audit. PHY creation, routing, authorized exact inspection and already-supported management are required; cross-owner retrieval expansion remains a separate explicit decision. Denied scope must never look like a harmless empty result.

## Configuration correction

The existing `reliquary.cfg` is persisted user data, not an internal migration shape. The accepted pre-rebuild architecture assigned its framing, model/credential codecs, encryption, master key, provider auth and device refresh to Switchboard. The rebuild must therefore restore that owner directly. See the [Switchboard configuration restoration spec](switchboard-configuration-restoration-spec-2026-11-08.md).

The isolated OC02 JSON/environment implementation is not accepted and must not become a second supported format. A missing dedicated Memory Retrieval decision object is valid because route policy already inherits shared General Decision and then adapted General.

## Two-day smoke evidence: what it does and does not establish

The [smoke record](native-two-day-live-smoke-2026-11-07.md) reports successful normalized Archive import/reopen of 778 visible messages, but no completed live Memory corpus. Its inference path used `tools/native-corpus-live/src/endpoint.rs` and `scripts/live_corpus_endpoint.py`, rather than the normal configured Switchboard transport. The first run did not enforce the supplied schema; subsequent runs exposed provenance/wording mismatches.

These are diagnostic inputs, not a verified defect assignment to the normal provider path. Reproduce through the canonical configured path before changing extraction rules. Never weaken quote, authority, timing or identity validation merely to obtain a nonzero Memory count. Preserve failed-run evidence; do not promote it to successful corpus acceptance.

The smoke runner links Application/owner packages directly. It is not the required external product consumer and cannot prove public API completeness. Its normalization logic may become source-adapter tests, but semantic orchestration and provider execution must move into their legitimate library layers.

## Existing work overlap

The isolated `Reliquary-postg4-pgc02-hp02-sleipnir-2026-11-07` worktree contains proposed generic product exports. Review reusable hunks against this specification; do not merge the Hermes branch or make generic operational closure depend on HP01 service protocol decisions. Python contracts/dashboard work remain separate. D01/F03 migration agents retain their own code, artifacts and caches.

## Testing and audit limits

Read directly: Root exports/host/management, runtime retrieval preparation/delivery/passive search, Application agentic retrieval, earlier P01 and retrieval acceptance documents, CLI grammar previously inspected, and current worktree/HEAD. A bounded runtime method search additionally identified worker/context/external-source entry points; their presence is not a comprehensive semantic review of every implementation.

Lexicon configuration is absent in this checkout. No refreshed graph evidence is claimed. The accepted executable package-edge contract remains 23 packages and 76 production edges. No build, provider call, corpus mutation or source change is required to write this audit.

## Related docs

- [Operational completion specification and ticket dependencies](public-api-cli-operational-completion-spec-2026-11-07.md)
- [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md)
- [Interaction and CLI contract](public-api-cli-interaction-contract-2026-11-07.md)
- [P01 contract map](p01-consumer-contract-map.md)
- [Public API guide](public-product-api.md)
- [Accepted retrieval semantics](retrieval-confirmed-use-acceptance-2026-11-05.md)

## Notes

This inventory prevents another claim of whole-product usability based only on internal tests. OC01 freezes exact method/type exports and test names for these rows; it is an execution task against this inventory, not another open-ended architecture study. Missing behavior is either implemented at its proper owner or explicitly left unaccepted. The API/CLI work is a separate necessary operational closure item, not a new prerequisite for old-byte source verification.
