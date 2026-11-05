# M phase — Maintenance, History and Reconciliation Parity

Parent: [capability tracker](implementation-rebuild-capabilities-2026-11-03.json) · [rebuild plan](../architecture-rebuild-implementation-plan-2026-11-03.md) · [F02 behavior inventory](../supported-behavior-inventory.md)

**Status: specified; not implemented or accepted.** Execution starts **after G3 acceptance** and ends at **GM acceptance**. P01 (public Rel/PHY API and CLI) is blocked on GM. This phase restores supported behavior that was excluded from the narrowed V06/G2 acceptance; it does not reopen the already verified V06/G2 tests or halt Gateway H03/H04/G3.

## 1. Authority and boundary

- **Implementation:** `C:\!bin\workspace\Reliquary-rebuild`, branch `rebuild/clean-architecture`.
- **Immutable comparison:** `C:\!bin\workspace\Reliquary-reference` at F01 pin `f62b1fb44ca7f0bd8351e3a30fb5d76211f85102`. Read only. Run reference tests only in disposable fixtures/worktrees.
- **Plan authority:** `C:\!bin\workspace\Reliquary-crate-first\docs\architecture\implementation-rebuild-capabilities-2026-11-03.json`. The archived F02 disposition is superseded **only for already implemented, externally supported maintenance, historical reads and supported divergent REL reconciliation** by this new explicit gate. This does not resurrect all deferred history/synchronization ideas.
- **Format:** rebuilt normal runtime supports only current REL/PHY **2.1**. Reference 2.0 output is an *observable behavior oracle*, not bytes or codecs to transplant. D01 handles offline 2.0→2.1 conversion later. Preserve external deterministic IDs when the retained semantic identity is the same; changed history-generation identity must be explicit.
- **Existing target:** retain `V06` native import, owner-cut Memory reads, terminal Memory-body pruning, verified copies, prefix reconciliation and offline promotion. Build on their approved owner/session/application boundaries. No new owner-of-owners, old `Cva` aliases, legacy runtime readers or generic relocation framework.
- **Ordering:** `G3 → (M01 and M02) → M03 → M04 → GM → P01 → G4`. M01/M02 may be implemented independently; M03 requires generation/retention semantics established by both.

**Policy:** Source is never modified during planning or copy construction. A destructive rollover/promotion needs an exclusive, authenticated offline owner admission, frozen source identity/cut/hash, independently reopened new copy, retained rollback and a crash-safe replacement protocol. A source or destination that changes between plan and execution is refused. Do not infer that a successful `Container` open proves semantic or historical correctness.

## 2. Contract matrix and reference evidence

| Behavior to restore | Reference evidence (under `Reliquary-reference`) | Rebuild starting point | Required decision |
| --- | --- | --- | --- |
| Policy-driven fresh-history reconstruction | `src/rewrite/{model,plan,file}.rs`; `cli/src/rewrite_cmd.rs` | No equivalent planner/executor | **M01**: restore behavior with 2.1 semantics |
| REL/PHY terminal pruning across proved domains | `src/facade/{cva_history_prune,phylactery_history_prune}.rs`; `src/facade/*compact_prune_tests.rs` | Terminal Memory-body slice in owner-session | **M02**: expand with owner-local evidence |
| Unreachable-space reclamation and pointer relocation | `src/facade/storage_reclamation.rs`; `src/storage_reclamation_scan.rs` | No full reclaimed-copy equivalent | **M02**: explicitly proven discard/relocate |
| Compatibility profile repair/retirement | `src/facade/*compatibility_profiles.rs`; `cli/src/repair_cmd.rs` | Lower vector store supports retirement; not a complete owner workflow | **M02**: owner-checked repair and dry-run planning |
| Diverged same-owner REL reconciliation | `src/facade/{cva_reconcile,cva_reconcile_repack}.rs`; `src/imports/*reconcile*` | Verified exact-prefix REL/PHY copy | **M03**: supported semantic replay and typed refusal |
| Exact historical queries/proof | `src/facade/historical_*fingerprint.rs`, `src/facade/cva.rs`, `src/facade/phylactery.rs`; retained historical tests | Owner-cut Memory read and domain-specific store replay | **M04**: compare the supported evidence/query surface |

This matrix is the *minimum* known gap list, not a claim that every internal helper above is a public API. Each M increment first inspects the relevant reference tests and supported external call sites. Map **observed contracts**, not function names. Record which cases need different 2.1 representation, which are already implemented and which cannot honestly be supported.

## 3. M01 — Fresh-history rewriting

**Result:** A read-only policy/plan and an explicit executor for new REL/PHY **histories**; this is not version-preserving pruning, ordinary file compaction, reconciliation, or 2.0 conversion.

**Requirements**

1. Accept a validated selection policy: explicit included/excluded Memories; deterministic lifecycle, duplicate/supersession/canonical, age/source-time, revision and Freshness filters; selected embedding profiles including **zero**; REL Archive/provenance retention; explicit Freshness and derived-state modes. Reject invalid, contradictory or unsupported combinations, never silently widening a filter.
2. Produce an inspectable plan with retained/dropped counts, provenance and referential-closure expansion, omitted-state warnings, estimated output characteristics and a frozen owner/source proof. Dry-run does not create or mutate a destination.
3. Execute to a new, independently reopened 2.1 owner. Carry exactly the chosen semantic state and its necessary closures: Memory, Archive source (REL only), Entity/Relationship/Graph, Community, vectors, Freshness, Ego, import/Dream/operational evidence where retained-state correctness requires them. Never manufacture historic events, derived identity, model output or re-embeddings.
4. Support **NewOwner** (independent owner identity) and **PreserveOwner** (explicit new history generation plus predecessor/rewrite evidence) as separate, auditable modes. Preserve-owner generations are not normal divergent replicas and cannot be reconciled by M03.
5. Enforce PHY-private Ego authority and REL-only Archive operations; define and test unsupported state as a typed stop, not silently lost content. Retained references and accepted provenance remain valid at reopen.
6. Verify filtered results against a suitable reference semantic oracle, then verify new owner provenance, selected profile/vector coverage, generation identity, clocks, independent reopening, safe failure and repeat attempts.

**Acceptance cases:** default all-current selection; explicit include/exclude and lifecycle filter; no profiles; multiple profiles; provenance closure; tombstoned/duplicate selection; PHY private Ego; REL Archive selection; source changed after plan; unknown references; unsupported policy; NewOwner; PreserveOwner lineage and cross-generation merge refusal. No output appears on failed preflight.

## 4. M02 — Multi-domain pruning, repair and reclamation

**Result:** Version/history-preserving *pruning* of supported terminal payloads, and separately a verified *reclamation* operation for truly unreachable physical storage. The two must not share a misleading generic "rewrite" semantic.

**Requirements**

1. Inventory actual supported terminal cases from reference fixtures and map each to a 2.1 owner proof. Required classes include terminal Memory bodies, retired compatibility-profile packed vectors, selected terminal Archive content in REL, and terminal Entity/Relationship description payloads. Include Graph/Ego/other state only to the extent the reference demonstrates a supported destruction contract; otherwise explicitly record non-applicability. PHY must not acquire REL Archive authority.
2. Before pruning, verify terminal lifecycle, exclusive payload ownership, historical cut, provenance dependencies, vector references and entire owner closure. Persist typed retention evidence/tombstone markers that distinguish intentional removal from corruption. Do not destroy a payload still referenced from an active or unselected owner record.
3. Reclamation separately detects **provably unreachable** backing content, orphan bodies, superseded indexes, unactivated generations and retired physical payloads. Translate any physical offsets in retained records using domain-owned relocation rules, then compare version clocks, owner state, historical evidence and current retrieval before/after. Unknown carriers or inbound offsets **fail closed**.
4. Restore owner-level compatible-profile retirement/validation and repair planning, including retaining specified active profile sets and checking coverage without inference or re-embedding. Expose workflow operations here; public CLI command shape belongs to P01.
5. Every operation creates a separately verified candidate; durable manifest or lineage, independently reopened REL/PHY and exact negative-case proof precede any promotion. Source remains safe on interruption, destination collision, stale lease, authorization failure or incompatible history.
6. The existing terminal Memory prune is a retained implementation, not a reason to duplicate or replace it without evidence of a defect.

**Acceptance cases:** one supported terminal target per proven domain; REL/PHY symmetry where applicable; shared body/vector still referenced; active target; unsupported domain; no-op reclamation; reclaimed bytes on a fixture with unreachable chunks; stable historical cuts; retained active vector coverage; source unchanged after injected copy/sync/reopen failures; no ambiguous profile retirement.

## 5. M03 — Supported divergent REL reconciliation

**Result:** Same-owner **REL** reconciliation with identical/strict-extension fast paths and supported semantic divergence. This does not implement a general Git merge, cross-owner merge, arbitrary PHY merge or conflict auto-resolution.

**Requirements**

1. Compare owner ID, scope, history generation, common accepted semantic prefix and validated provenance; report **Identical**, **LeftExtendsRight**, **RightExtendsLeft**, **Diverged** or typed incompatibility.
2. Continue using verified-copy prefix path when valid. For divergence, replay supported branches through their owning semantic APIs in an order that preserves source/Archive, Memory, Entity/Relationship, Graph, Dream, Freshness, import epochs, Echo and dependent receipts where their reference contract allows. Rebase clocks/cuts only with exact accepted evidence and correctly record derived-vector rebuild requirements.
3. Detect exact semantic duplicates without double-applying Memory/Freshness or replaying source turns twice. Reject conflicting identity/version evidence, incomplete imports, mismatched active writer lineage, unsupported derived owner state, conflicting Ego/Entity resolution and different history generations using typed conflict results. Do not choose a "newer" side or silently drop one history.
4. Open and validate the candidate output independently, compare preserved accepted histories and import settlement authority, and ensure neither source changed. Failed or conflicted reconciliation leaves both owners and existing destination untouched.

**Acceptance cases:** identical and both extension directions; independent nonconflicting divergent Memory/Archive changes; overlapping duplicate transaction; valid graph and import/Freshness replay where supported; conflicting source/Entity/Ego; diverged generation; wrong owner/scope; crash and retry; repeated reconciliation creates same semantic result without duplicate credit. Unsupported cases are explicit, not counted as successful merge parity.

## 6. M04 — Historical coverage and proof

**Result:** An evidence-based check of historical access across all retained semantic owners, restoring *supported* historical views missing from 2.1.

**Requirements**

1. Inventory actual reference historical tests/consumer reads at explicit version/activity cuts for REL Archive/Branch/Episode/Fragments, Memory, Entity/Relationship/Graph, Ego, Freshness and vector generations, including whole-owner fingerprints where proven. Distinguish publicly supported operations from test-only or diagnostic projections.
2. For supported reads, verify the same **observable** state, accepted ordering, timestamps and unavailable/tombstoned/pruned semantics at corresponding semantic cuts. Boundaries inside grouped publications, ambiguous cut authority, retired embedding generations and missing provenance must not be fabricated or silently normalized.
3. Verify history *before and after* M01 rewrite (retained history only by policy), M02 prune/reclaim (minimum lossless retained history) and M03 reconciliation (replayed history plus explicit conflicts). REL/PHY scope and privacy are checked at each entry point.
4. Use reference fixtures or independently generated equivalent 2.1 fixtures. The 2.0 reference's raw framing, `Cva` API names and exact physical offsets are **not** parity requirements.
5. Distinguish restore-and-continue and broad speculative history/synchronization features (still deferred) from the reference implementation's existing supported read/maintenance contracts.

**Acceptance cases:** representative early, middle and final cuts; pre/post tombstone; pruned-but-retained metadata; grouped commit cut; imported chronology; Entity/Graph and vector historical consistency; PHY-private state; unauthorized/cross-owner query; unsupported/unavailable cut; restart and transformed-copy parity.

## 7. GM — Mandatory maintenance/history parity gate

**GM is a genuine acceptance gate, not a documentation checkbox.** It can be accepted only if M01–M04 are each accepted and the following evidence is recorded in a single review in `Reliquary-rebuild/docs/`:

- A **reference-to-rebuild behavior matrix**: source API or test, supported scenario, 2.1 operation, proof result, negative case, deliberate format difference and exact limitation. No unsupported feature may be presented as supported parity.
- At least one end-to-end REL and PHY scenario spanning plan→transform→independent reopen→history/query, with separate new-owner and preserve-owner rewrite cases; version-preserving prune plus reclaim; successful and typed-conflict divergent REL reconcile.
- Explicit invariants: no source mutation before authenticated promotion; source/backup recoverable after failure; no unauthorized cross-owner disclosure; no lost stable retained identities; no dangling references; semantic scores and activity not reinforced by maintenance; no double-processing or silent reconciliation.
- Deterministic tests that prove each distinct contract and important fault boundary. Do not copy entire reference test suites or multiply cosmetic cases. Run `python scripts/check_architecture.py`, `cargo fmt --all --check`, `cargo check --workspace --locked --offline -j 1`, `cargo test --workspace --locked --offline -j 1 -- --test-threads=1` in the rebuild; record unavailable environment-dependent checks honestly.
- Independent owner reopening and relevant performance/space measurements for representative scenarios, plus the remaining regression and conflict-risk inventory. Do **not** claim the original corpus converted: D01–D03 are later.
- Focused commits with clean unrelated custody; update this tracker and GM evidence. Any uncovered required behavior blocks acceptance unless the user explicitly authorizes a separately recorded scope change.

**P01 cannot start acceptance until GM passes. G4 also names GM as a direct safety dependency.** The prior V06/G2 evidence remains valid for the narrower behaviors it actually proved.

## 8. Explicit exclusions and execution discipline

- Not in scope: new Perception Observation synthesis, new Ego model synthesis, unimplemented restore-and-continue ideas, unproven future provider formats, arbitrary cross-REL/PHY synchronization, old 2.0 runtime readers, compressed-vector research, broader retrieval tuning, and any migration/cutover operations belonging to D01–D03/G5.
- Reuse the existing approved Container/owner-session/semantic/Application split. Container owns physical framing/locks; individual owners validate their semantic retention/replay; Application coordinates multi-owner read-only planning and transformation lifecycle; Gateway authenticates/offline-quiesces the owner. No broad inference or embedding inside an owner writer lease.
- Build functional slices from contracts, then verify. Keep coordination files small (prefer under 200 lines and split before 300). Avoid temporary compatibility layers, new redundant owners/crates, placeholder capability completion, huge speculative decomposition or test-per-line scaffolding.
- **H03/H04/G3 can finish first without disruption.** At G3 acceptance, start M01 and M02. Complete M03 and M04, then GM. P01/CLI subsequently exposes the accepted operations. The tracker is the sole acceptance authority.
