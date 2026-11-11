# ES04 architecture obligations and measured enforcement

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record architectural obligations, reliable static enforcement, source-to-test review and remaining gate evidence. This report does not substitute for current architecture owners.

## Overview

Implementation baseline: `fa574eecd11dddbe22474f382f8018cf0c0f98c5`; unrelated source-worktree Rust edits are excluded. Luna agent `architecture_tooling` drafted the policy; integration review corrected evidence selection and compatibility. Initial scan attempts were stopped without publishing a snapshot. This is incomplete evidence, not proof that Rust semantics are unsupported. A fresh standard-path scan is measured separately in final verification.

## Architectural obligation inventory

| Obligation | Current owner | Enforcement and review boundary |
| --- | --- | --- |
| Approved packages, production edges, cycles and selected direct store bans | [System](../architecture/system.md) | `scripts/check_architecture.py` (hardened after the initial inventory without changing approved edges): 23 packages/76 edges, 15 test-only edges, exit 0 |
| Container framing, durable state, malformed records and physical recovery | [Storage](../architecture/storage.md) | Container/store tests; independent review matched partial-append refusal/recovery to `a_failed_partial_append_blocks_writes_until_explicit_recovery` |
| Concrete stores, writer authority, typed replay, authorization, grouped writes and history | [Owner-session](../architecture/owner-session.md) | Codec/admission/replay and Application grouped-publication tests in [behavioral matrix](../development/behavioral-contracts.md); graph cannot prove atomic durability |
| Named composition/publication and forbidden reach-through | [Application](../architecture/application.md) | Exact Cargo/direct-source guard plus workflow tests; permitted dev/test edges must not be misclassified as production |
| Bounded semantic decisions; no higher runtime orchestration in foundations | [Semantic processing](../architecture/semantic-processing.md) | Pitlord any-category upward-dependency rule; service and publication tests protect behavior |
| Mounts, Instances, grants/revocation, workers, queue/host/stream recovery | [Gateway](../architecture/gateway.md) | Source review of mount/quiesce and sync-before-handoff/status-only reconciliation; queue, retrieval, worker and retained-host tests |
| Provider routing, configuration/security and key lifecycle | [Switchboard](../architecture/switchboard.md), [configuration](../configured-runtime.md) | Source review of fallback/binding; security/key/fixture tests; no real credential/provider operation |
| Opaque Root API, exact CLI and external host seams | [API](../public-product-api.md), [CLI](../reference/cli.md) | Independent source review; separate Root-only consumer and CLI suites; host implementation obligations remain explicit |
| File ownership and observed import cycles | [Policy operations](../../tools/pitlord/README.md) | 24 package/public-surface areas; ownership, any-category upward-dependency and CLI Root-only prohibitions, and observed import/dependency cycles including tests |

## Reliable evidence selection

The composition includes generated architecture-core and documentation-core plus local semantic rules. Package areas include their tests; Root and CLI have explicit areas. File ownership selects `src`, `crates`, and `cli/src`, without claiming root integration tests are production files.

Rust Cargo `depends-on` facts include runtime, dev, build and target-conditioned edges. Pitlord v1 cannot filter those attributes; inline `#[cfg(test)]` imports also share production paths. Therefore the semantic rules select both `imports` and `depends-on`, prohibiting foundation/service dependencies toward Application/Gateway in every category and checking observed area cycles including tests. The current manifests have no edge in that forbidden orientation. They do not misclassify Application/Gateway dev-store dependencies. The hardened exact Cargo guard remains the independent production-edge/direct-source gate.

Static evidence cannot prove procedural-macro expansion, unsafe aliasing, dynamic registration, external host durability, unconstrained dispatch, single-writer behavior, runtime authorization, fsync, privacy or recovery. The behavioral matrix maps concrete positive, negative, concurrent and reopen cases. A focused synthetic per-path lock test was added for competing handles/thread acquisition/release. It does not prove every end-to-end credential-writer/migration interleaving; sequential stale-handle tests retain their narrower scope.

## Tool provenance and procedure

CI source pins: Pitlord `9dedd4b556b9e54de514e4cafe4d94093780178f`, Demon Docs `45318a8f2602aeebaeea4828894769256773f4f4`, Lexicon/Arcana `f09517262fdfad98fcede2ad93a947ccee8406ff`. Luna independently built all three Go command targets offline with Go 1.26.5, local cache/temp paths and exit 0. Local HEAD differences from the first two pins were documentation-only. No hosted CI run is claimed.

An older installed Pitlord and the current source build both report `0.1.2`; only the source build supports synchronized `require_content`. Source identity and capability validation matter. The source-built binary validated the integrated policy with **24 areas / 13 flattened rules, exit 0**. Demon Docs reports `0.3.5`; `lexicon version` reports `dev`.

`python scripts/check_policy_tools.py` validates policy, refreshes Rust evidence, synchronizes Arcana and checks policy with per-stage/total timeouts; timeout cleanup stops its spawned process tree. `--validate-only` proves syntax only. Standard `.lexicon`, `.arcana` and `target` paths are excluded. CI tool sources are under excluded `target/standards-tool-sources` so the product graph does not ingest analysis tools.

## Acceptance status

[Final local verification](./es06-local-verification-2026-11-11.md) records the fresh graph, snapshot, exits, suites and independent review: 24 populated areas, 568 owned files, 2,051 selected edges, 111 area dependencies and zero violations across 13 rules. Local gates pass; hosted CI and formal adoption remain unverified. No speculative Rust refactor, DAG relaxation, semantic baseline or real corpus/provider operation is part of this remediation. The deferred homogeneous-REL/inter-REL dependency-DAG issue remains in planning and limits.

## Related docs

- [Remediation specification](../engineering-standards-remediation-spec-2026-11-10.md)
- [Maintainer map](../architecture/maintainer-map.md)
- [Coverage](../development/documentation-coverage.md)
- [Limits](../limits/current-limitations.md)

## Notes

Source spot checks established no new material implementation violation. The final report qualifies local acceptance and retains the remaining hosted/adoption and behavioral evidence limits.
