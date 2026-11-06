# GM — maintenance, history and reconciliation parity gate

Status: **accepted** on `rebuild/clean-architecture` (2026-11-06).

Authority: capability `GM` and
`maintenance-history-parity-phase-2026-11-05.md` in the canonical planning
repository. M01, M02, M03 and M04 are individually accepted. This review is
the combined gate; it does not expose the P01 public API/CLI or convert the
archived 2.0 corpus.

## Reference-to-rebuild behavior matrix

| Reference behavior evidence | Supported scenario | Rebuild 2.1 operation and proof | Negative proof | Deliberate difference and exact limit |
| --- | --- | --- | --- | --- |
| `src/rewrite/{model,plan,file,semantic_state}.rs`, `cli/src/rewrite_cmd.rs` | Policy-selected REL/PHY fresh history; NewOwner and PreserveOwner | Application `plan_*_rewrite` plus `execute_memory_rewrite`; selected stable IDs, closure, provenance, profiles, clocks and lineage independently reopen; REL and PHY tests now obtain an M04 proof from the reopened result | Contradictory policy, unknown IDs, stale source, destination collision, missing vector coverage and unsupported carrier create no output | Native 2.1 records and generation lineage replace 2.0 framing/offset equivalence. Operational Dream/Insomnia/import/Echo/external carriers are typed stops. Public command shape is P01. |
| `src/facade/{cva_history_prune,phylactery_history_prune,memory_history_prune}.rs`, `history_prune_tests.rs` | Terminal Memory body removal in REL and PHY | Existing owner-session terminal Memory prune retains revisions, tombstones and typed retention evidence, reopens independently and exposes unavailable history | Live/shared/vector-referenced bodies, corrupt proof and unknown frames fail closed | Payload bytes may be unavailable; semantic history is retained. This is not fresh-history rewrite. |
| `src/facade/cva_archive_history_prune.rs` and Archive prune fixtures | Terminal REL conversation content | `terminal_archive_prune_copy` retains turns, branches, Episodes, attachments, clocks and historical structure while marking unavailable text | Active lifecycle, Memory/file/vector provenance, shared content and forged markers refuse publication | PHY never gains Archive authority; attachment/file bodies are not implicitly deleted. |
| `src/facade/cva_description_prune.rs`, `entity_compact_prune_tests.rs`, `relationship_compact_prune_tests.rs` | Terminal Entity and Relationship description payloads | Owner-session Entity/Relationship prune preserves stable identities, participants, evidence, revisions and clocks in REL/PHY | Active mentions, Graph/Relationship references, stale plans and forged/missing evidence stop | Graph edges and Ego are not destruction targets; no independent supported reference contract was found for them. |
| `src/facade/{cva_compatibility_profiles,phylactery_compatibility_profiles}.rs`, `cli/src/repair_cmd.rs` | Profile retirement/repair and retired packed vectors | Owner-level repair freezes the active set and verifies Memory/Archive coverage; vector prune retains mappings and generation receipts and reopens | Shared/active backing, stale plan, incomplete coverage and unactivated generations stop | No inference or re-embedding; CLI exposure remains P01. |
| `src/facade/{storage_reclamation,storage_reclamation_scan}.rs` and reclamation fixtures | Proven unreachable Memory/vector/Archive backing | Verified REL/PHY copy reclaims only unreachable storage, relocates retained generation evidence and independently reopens; a second pass is exact no-op | Unknown carriers, inbound references, copy/sync/reopen injection, source change and destination collision retain source and publish nothing | Reclamation is separate from semantic prune. No generic offset relocator or old-format runtime reader is introduced. |
| `src/facade/cva_reconcile*.rs`, `src/imports/*reconcile*` | Same-owner REL identical, extension and supported nonconflicting divergence | `compare_rel_histories`, verified-copy fast paths and `reconcile_divergent_rel_copy` replay Archive/Memory/Perception/Graph/Freshness/Echo evidence exactly once; merged proof is stable on repeat | Owner/scope/generation mismatch, identity conflict, open/divergent import authority, unsupported state and existing destination leave both inputs unchanged | No PHY, cross-owner or arbitrary merge; unsupported operational/maintenance divergence is a typed conflict rather than a chosen winner. |
| `src/facade/historical_*fingerprint.rs`, `historical_*tests.rs`, historical Cva/Phylactery reads | Early/middle/final cuts, tombstones, pruning, imported chronology, private Ego and whole-owner proof | Store-owned 2.1 projections feed bounded owner-session SHA-256 proof over Archive, Memory, Perception, Graph, Ego, Freshness and retained vector generations | Wrong owner/kind, future/inconsistent cuts, grouped interiors, ambiguous Freshness authority and over-limit proof fail closed | Semantic cuts replace raw 2.0 offsets and Cva names. Missing/pruned payloads remain unavailable; broad restore-and-continue is not claimed. |

No reference helper or test-only diagnostic outside these supported scenarios is
presented as rebuilt parity.

## Combined end-to-end evidence

- REL NewOwner: plan a selected rewrite, publish a distinct owner, reopen it,
  verify Archive provenance, effective Freshness and Memory/Archive vectors,
  then obtain a bounded historical proof at the reopened cut.
- PHY NewOwner: plan with referential closure, transform to a distinct owner,
  prove source bytes unchanged, independently reopen both retained Memories
  and obtain the private-owner historical proof.
- PHY PreserveOwner: preserve owner identity, record predecessor/new generation
  lineage, independently reopen and prove the policy-selected history. The
  verified-derived-state omission intentionally changes the semantic digest;
  the generation changes explicitly and prefix reconciliation refuses it.
- REL PreserveOwner: the M03 generation acceptance fixture proves an unchanged
  selected semantic digest at the same cut with a different explicit history
  generation, followed by typed cross-generation reconciliation refusal.
- Prune/reclaim: terminal Memory, Archive, Entity, Relationship and vector
  payload cases reopen with retained history. REL and PHY reclamation remove
  unreachable backing; referenced and second-pass no-op copies remain exact.
- Reconcile: independent REL Archive/Memory/Graph/Freshness branches merge once,
  independently reopen and produce a stable proof. Repeating against the
  merged history is byte-identical with zero replayed semantic effects. A
  conflicting Archive identity produces typed conflict, no output and exact
  source preservation.

## Gate invariants

- Planning and candidate construction never mutate the source. Transform and
  reconcile tests compare source bytes before/after success and injected
  failure. Publication uses a private synchronized candidate, independent
  owner reopen and no-replace publication.
- Owner identity and REL/PHY kind are checked before reads or writes. PHY Ego
  stays private and REL-only Archive/Freshness authority cannot cross scope.
- Stable retained Memory, Archive, Entity and Relationship identities remain;
  all Graph, provenance, vector and Freshness references are revalidated.
- Maintenance never calls a model, infers new state, re-embeds, reinforces
  activity or fabricates missing evidence.
- Idempotency identities prevent double Memory/Freshness/Graph application.
  Reconciliation reports unsupported conflict instead of silently choosing or
  dropping a history.
- Failed copy, sync, reopen, stale-plan, authorization and collision paths keep
  the source/backup recoverable and do not publish a partial destination.

## Performance and space observations

These are deterministic test-fixture observations, not production SLOs; G5
owns release-scale latency and memory baselines.

- The 14 focused Application rewrite/reclamation tests execute in **0.44 s**
  after build on this Windows host.
- The owner-session gate suite executes 125 unit tests in **3.67 s** and six
  integration tests in **0.15 s** after build.
- Archive prune, retired-vector prune and REL/PHY orphan reclamation assert the
  operation report's candidate byte count is strictly lower than its source
  byte count. Referenced-content and second-pass no-op fixtures assert exact
  byte equality. Absolute fixture byte counts are deliberately not treated as
  a compatibility promise.

## Verification

- `cargo test -p reliquary-application rewrite_workflow::tests --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **14 passed**.
- `cargo test -p reliquary-owner-session --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **125 unit + 6 integration passed**.
- `python scripts/check_architecture.py`: **passed**, 23 production packages and 76 approved edges.
- `cargo fmt --all --check`: **passed**.
- `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check`: **passed**.
- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **passed**, including all crate, integration and documentation suites.

Windows hard-link publication tests were run with ordinary local filesystem
permissions. Existing compiler, incremental-cache and `LNK4098` warnings remain
outside GM. No package, manifest or approved production dependency edge
changed.

## Remaining regression and conflict risk

- Unknown or unproven carrier families remain typed stops for rewrite,
  reclamation, reconciliation and historical proof. That fail-closed behavior
  is accepted; it is not a hidden deferral of a supported case.
- Divergent native-import authority is supported only where shared accepted
  evidence can be replayed without invention. Divergent settlement, Community,
  Ego, Dream/Insomnia, external-source, maintenance and pruned-payload state
  remains an explicit M03 conflict.
- Promotion requires same-volume no-replace/hard-link semantics. Filesystems
  without that facility cannot perform destructive promotion, but verified
  candidate construction and source recovery remain available.
- Public facade/CLI/host exposure is P01. Generated-corpus product acceptance
  is G4. Offline 2.0 conversion, converted-corpus verification and activation
  remain D01–D03; this gate makes no conversion claim.

## Decision

All GM prerequisites and required gate evidence pass with no uncovered
supported behavior silently deferred. GM is accepted; P01 may begin.
