# Crate-first Root dependency and ownership audit — 2026-10-30

**Verdict: architecture extraction NOT COMPLETE.** Separately compiled lower crates are real, but the dominant Root dependency hub has not been dismantled. A passing Cargo DAG and a count of nonempty crates cannot establish the intended final ownership.
**Baseline:** `refactor/crate-first-restart` HEAD `de1ca3c1`. The unrelated pre-existing dirty `docs/graph-crate-extraction-evidence-2026-10-26.md` is excluded from this audit and must be preserved. No production code changed.
**Scope:** tracked Rust source under `src/` and `crates/`, Root facade/owner aggregates, production gateway, semantic/service migration seams, Cargo's current dependency surface, retained format/recovery gate. External Warlock/Hermes integration is outside this refactor.

## 1. Measurement, not completion by crate count

Computed over `git ls-files 'src/*.rs' 'crates/*.rs'` at the audited HEAD, counting physical lines and omitting files named `test.rs`, `tests.rs`, `test_support.rs`, `*_test.rs`, `*_tests.rs` and `*/tests/*`. **Not** an AST-based estimate of executable lines: inline `#[cfg(test)]` and comments in other files remain included, and name-based exclusions are approximate.

| Area | Tracked Rust files (all) | Files excluding test-named paths | Lines excluding test-named paths |
| --- | ---: | ---: | ---: |
| Root `src/` | 723 | 407 | **81,996** |
| Extracted/reserved `crates/` | 521 | 413 | **66,134** |
| Both | 1,244 | 820 | **148,130** |

**55.4% of measured non-test-named Rust source remains in Root.** This measures remaining concentration, **not** the percentage of illegal dependencies or work left. Some Root code legitimately owns cross-owner transactions, host admission and public API.

| Root neighborhood | Non-test-named files | Measured lines |
| --- | ---: | ---: |
| `src/runtime_inference/` | 149 | 36,971 |
| `src/facade/` | 117 | 24,882 |
| `src/perception/` | 35 | 4,483 |
| `src/semantic_graph/` | 17 | 2,858 |
| `src/imports/` | 12 | 2,646 |
| `src/rewrite/` | 5 | 2,614 |
| `src/retrieval/` | 38 | 2,244 |
| `src/external_source/` | 8 | 1,636 |
| Remaining Root neighborhoods and top-level files | 26 | 3,662 |
| **Total** | **407** | **81,996** |

Lexical dependency indicators over the same 407 files: **186** mention `Cva`, **75** mention `Phylactery`, **21** mention `InteractionRuntime`, **38** mention `GatewayRuntime`; **68** contain `impl Cva` and **24** contain `impl Phylactery`; **345** contain `use crate::`. These are **text matches, not resolved graph edges or confirmed violations**. They indicate the size of the Rust package-internal coupling surface that Cargo cannot enforce.

`cargo tree -p reliquary-memory --depth 1 --locked --offline` succeeded; Root directly depends on the **28 live non-placeholder domain/support crates** (plus external libraries). Those library-to-library edges are compiled; Root-internal `crate::` use remains outside that physical separation. `reliquary-ego` and `reliquary-runtime-gateway` remain comment-only (four lines each). Ego's semantic service is explicitly **future feature work**, not an extraction blocker; Gateway has substantial **existing** production code not yet extracted.

## 2. Confirmed architectural knot: Root owns the concrete aggregates

**Evidence:** `src/facade/cva.rs` declares `Cva` with `Container` and mutable Archive, Memory, Graph, Freshness, Ego, Entity, vector, Dream, Insomnia, Import, external, history/rewrite and other owner fields. `src/facade/phylactery.rs` declares the corresponding private-personal aggregate. `src/facade/cva_lifecycle.rs` and `phylactery_lifecycle.rs` construct, initialize and replay these concrete aggregates. `reliquary-owner-lifecycle/src/lib.rs` has useful lower-level frame traversal/classification and core-frame ingestion, but **does not own the concrete Cva/Phylactery aggregate**.

**Consequences:** Root is at once public facade, owner assembly/transaction coordinator, semantic-adapter host, and execution host. Independently compiled store crates cannot stop a Root module from reading and coordinating arbitrary Root-owned mutable fields. The owner aggregate is a real architectural responsibility, not just a re-export, and current field access is a significant reason other modules cannot move downward.

**Decision needed before declaring the target reached:** Choose one canonical owner-aggregate boundary below the execution Gateway, with authorized owner mutations and immutable read/proof seams. Evaluate whether the existing `reliquary-owner-lifecycle` can own the aggregate **after** separating policy-dependent fields, or whether an explicit lower owner-composition library is necessary. Do **not** blindly move the current `Cva` shape downward: its existing `FreshnessDueDispatcher` and other policy/adapter state would import higher semantic services into a low owner crate. Alternatively, intentionally keep Gateway and aggregates in the top product package and **amend the original target** to say so; this is a design change, not completion of the presently declared 30-crate target. Preserve published public type paths by re-export, without a duplicate mutable aggregate or generic all-service interface bucket.

**Priority: P0, confirmed blocking target architecture.** The exact final type split is not established by this audit; pretending to have selected it would repeat the same planning failure.

## 3. Confirmed Gateway under-extraction and dependency wall

**Evidence:** `src/runtime_inference/runtime_host_gateway.rs` owns the actual `GatewayRuntime` and directly imports Root `InteractionRuntime` and `Phylactery`; `runtime_host_rel_execution.rs` stores `Arc<Mutex<InteractionRuntime>>`; the PHY execution uses the Root `Phylactery`. `crates/reliquary-runtime-gateway/src/lib.rs` has no production behavior.

**The current Gateway spec is insufficient as final extraction acceptance:** `docs/runtime-gateway-service-crate-extraction-spec-2026-10-30.md` G1–G5 would extract `InstanceRuntime` (~455 lines) while leaving the principal Gateway runtime, REL/PHY worker coordination, durable queue, journals, streams, host handoff, subscriptions and recovery in Root. The spec openly states that this does not prove the entire host was relocated. That is a useful intermediate cut, **not the final target**.

**Verified promising movable source seams** (actual type/call graph and full formats still require pre-cut proof):
- `runtime_host_instances.rs` — 455 lines: instance grants, incarnation, selections. Move as one canonical state, retaining host-only mutator authority across the crate boundary (the existing spec's controller requirement).
- `runtime_host_gateway_host.rs` — 777 lines: host capability/queue contracts, router and handoff types; inspection shows only standard library, Serde and SHA imports at file level, with no direct `crate::` or `super::` references. Candidate for a proper lower Gateway host-contract module, **not** permanent Root policy.
- `runtime_host_owner_operation_journal.rs` — 1,046 lines: gateway-owned `RQOP0001` journal; no direct Root module imports found in the inspected production source. Candidate for an independent lower Gateway persistence module, with exact replay/byte evidence mandatory.
- `multiplexing_journal.rs` — 2,203 lines: `RQMUXJ01/RQMUXS01/RQMUXH01` conversation queue/snapshot/host payload; directly references `HostCancelOutcome`, `HostOperationStatus`, `RuntimeCapability` and `HandoffReceipt` from the current Root host-contract module. Assess a **single coordinated move** with the preceding host contract instead of treating the journal as inherently Root-bound.
- `runtime_host_gateway_conversation.rs` — 2,125 lines and `runtime_host_gateway.rs` — 1,593 lines: actual live coordinator and Gateway objects; currently coupled to Root owner aggregate. Requires the owner-aggregate decision; do not mechanically move and introduce a back-edge.

These figures describe **production file sizes**, not proven extractable LOC. The existing G4 “optional second cut” is demonstrably too weak to finish Gateway. A corrected spec must explicitly account for all remaining `runtime_host_gateway_*`, owner operation, journals, REL/PHY worker, delivery, reconnect and stream families: **move to canonical Gateway, delegate to rightful lower owner, or identify a documented, narrowly justified public-product boundary**. No automatic Root exception.

**Priority: P0, confirmed architectural gap.**

## 4. Other Root neighborhoods: classification and required disposition

| Family / evidence | Confirmed current role | Audit disposition |
| --- | --- | --- |
| **Owner aggregate, open, historical and mutation** — `facade/cva*.rs`, `phylactery*.rs`, `graph_owner.rs`, ~24,882 facade lines total | Real concrete cross-owner state, lifecycle, admission, public API **mixed** | **P0 structural split**. Physical owner/transaction assembly must have one target; public facade and permission checks may remain top-level. Field-by-field cut and consumer conversion required. |
| **Gateway/runtime** — ~36,971 root lines, journal/host examples above | Gateway authority and host streams **mixed** with Insomnia/Dream/Perception worker adaptation | **P0**. Classify files by real owner; Gateway/external host coordination belongs in canonical runtime, not Root by default. Do not move semantic evaluation into Gateway. |
| **Insomnia service** — `runtime_inference/insomnia/processor/application.rs` (615 lines); `insomnia/completion.rs` (712 lines) | Grouped Memory+Insomnia publication, source validation, retained completion encoding/decoding and owner replay with Root types | **P1 proof-sensitive boundary**. Cross-owner commit/attribution may need upper coordination; **codec, value and service policy** must each be checked for rightful owner. Current lower Insomnia (3,407 measured non-test lines) and Insomnia Store (1,016) do not prove all owner-local behavior has moved. |
| **History** — `reliquary-history` 108 non-test lines; `facade/historical_manifest.rs` 1,138 lines and many Cva/PHY history methods | Lower crate evaluates supplied revisions; substantial history reconstruction/manifest handling remains in Root | **P1 boundary not accepted as full service ownership**. Separate immutable historical evaluation/owner-owned proofs from true cross-owner historical composition. Require concrete move/stay crosswalk. |
| **Maintenance/rewrite** — `reliquary-maintenance` 167 non-test lines; `rewrite/file.rs` 1,044, `rewrite/plan.rs` 738, plus facade prune/reclaim | Lower selection helper; full REL/PHY planning, transformation and file emission remain in Root | **P1 contract discrepancy**. File emission must respect owner publication, but original master specification assigned Maintenance coordination rather than just two selection helpers. Decide canonical destination and move nonproduct policies. |
| **Import** — `reliquary-import` 787 non-test lines; `src/imports/` 2,646 and `facade/cva_historical_import.rs` 585 | Lower normalization/preparation; retained native epoch workflow, readiness, replay and reconciliation depend on `Cva` | **P1**. Some cross-owner admission is legitimate, but the original Import owner must be compared against every retained Root workflow; do not call DTO extraction complete architecture. |
| **Entity Resolution/Perception** — `src/perception/` 4,483; `entity_resolution_boundary.rs` 446 | Lower resolution service exists, Root still hosts store admission/validation and owner coupling | **P1**. Preserve cross-owner validity at the correct owner boundary; extract pure service and local persistence behavior where still resident, with product mutation coordination above. |
| **Graph/Community** — `src/semantic_graph/` 2,858; Root `facade/graph_owner.rs` 529 | Lower Graph owner exists, Root still owns projections and cross-owner admission | **P1 verification**. Classify owner-local work vs genuine aggregate view; do not equate Root adapter names with automatic defects. |
| **Retrieval** — `src/retrieval/` 2,244 vs `reliquary-retrieval` 1,867 | Lower ranking/index policy plus Root source hydration and public projection | **P1 verification**. `retrieval/lexical_index.rs` is a legitimate owner-data bridge candidate; inspect remaining ranking/algorithm logic individually. Prior Retrieval acceptance documentation still records pending integration gates. |
| **External sources** — `src/external_source/` 1,636 | Index store separately extracted; host-source authentication/hydration/publication still tied to Root aggregate | **P1 Gateway-associated**. Assign to actual Gateway/host integration and store boundaries; no second transcript owner. |
| **Config, Chronos, Freshness, Ego adapters** — remaining smaller Root neighborhoods | Primarily reexports/bridges or owner-conditioned semantics | **P2 classification**. Do not manufacture work solely because a file resides in Root. Ego semantic generator does not exist and remains separately deferred. |

**Important:** This is a source-neighborhood and representative-file audit, not a per-method proof that every remaining line is misplaced. The accept/reject decision for each live function must follow the real owner aggregate redesign; relocating by path or file size alone would recreate the knot in a new crate.

## 5. Separate verified correctness and acceptance gaps

1. **Promoted corpus / current format — blocking release.** `CVAFRS07` has no literal source occurrence across current `src/` and `crates/`; `crates/reliquary-freshness-store/src/freshness_semantic_settlement.rs` still declares `SEMANTIC_SETTLEMENT_MAGIC = b"CVAFRS06"`. `tests/current_format_boundary.rs` pins six authoritative 2026-10-23 hashes, but its six-file gate is `#[ignore]` and expressly cannot pass until current Freshness/Import recovery is restored. `docs/current-limitations.md` records three REL copy reopen failures, three PHY reopen successes. Restore the actual current implementation first and run on disposable copies, preserving source hashes. This is **not** resolved by extracting Gateway or creating a fallback legacy reader.
2. **Ownership guard / documentation.** The earlier six unmapped Context Engine files and pending fresh Lexicon/Arcana/Pitlord acceptance are recorded in Maintenance/Context Engine evidence. Architecture source maps and `docs/crate-first-layout-spec-2026-10-25.md` contain early-state claims, not present-day final acceptance. Reconcile only after canonical ownership is fixed.
3. **Arcana graph cannot certify this cut.** The available Arcana `architecture_summary` groups the code into one community with `boundary_edge_count=0`, not a useful proof of surviving Root-to-crate ownership edges. Its snapshot is not established current to HEAD. Direct file inspection and Cargo are the evidence used here; a refreshed graph is optional supporting evidence, **not** a substitute for source verification.
4. **Fresh final build/test gate not run during this audit.** `cargo tree --locked --offline` passed; earlier individual acceptance suites belong to their recorded commits and scopes. No fresh whole-workspace compile or 6-file corpus acceptance is claimed.
5. **Unrelated preexisting Graph physical publication limitation.** The modified Graph evidence document records possible physical mapping/tail frames after I/O failure even when live semantic state did not commit. Treat as separate hardening investigation, not evidence the Root dependency knot is solved or as permission to mutate that document.

## 6. Correction to the execution plan / closure conditions

The plan is **not** “one last Gateway crate, then mechanical cleanup.” It has an unresolved structural owner-aggregate decision and a concrete set of missing Gateway extractions. Before authorizing the present Gateway G1–G5 as final:
1. **Decide and specify the owner-aggregate authority boundary** (`Cva`, `Phylactery`, owner lifecycle, concrete host access, errors, and public reexports). Preserve one canonical state and no cycles. A lower owner must not import upper semantic policy to reconstruct old fields.
2. **Rewrite Gateway acceptance for a full coherent runtime cut**, including independent host contracts/journals, REL/PHY execution, queues, streams, grant authority, Root callers and all deleted local implementations. If a particular operation legitimately must remain in the product facade, justify it with actual type/authority dependencies. The `InstanceRuntime`-only cut cannot close Gateway.
3. **Complete the source-level disposition of remaining Root domains** against the accepted target DAG. For each live production file/function, mark **move to existing owner**, **authorized Root cross-owner coordinator**, or **requires owner-boundary redesign**, with an actual compile/caller witness. Do not count “left in Root until easier” as accepted.
4. **Recover current-only `CVAFRS07` and run exact corpus proofs**, independent of architecture acceptance.
5. **Final integration:** no duplicate live owner, no lower-to-Root imports, no undeclared policy/backreach, accurate ownership/doc maps, nonzero crate tests, Root/CLI/public API, history/rewrite/recovery/stream tests, and six-file current corpus; document any genuine unresolved defect before promoting. Rust/Cargo boundaries are primary; source traces are required inside Root.

### Stop/go

**STOP on claiming crate-first complete or treating the existing Gateway spec as final. GO on a corrected root owner-aggregate/Gateway boundary specification and targeted hard-cut migrations.** Preserve the already extracted 28 live crates; their separation is useful. No return to the abandoned massive graph-proof machinery; use Rust compilation, source crosswalks, concrete tests and bounded recovery evidence.

**Source anchors:** `src/facade/{cva,phylactery,cva_lifecycle,phylactery_lifecycle}.rs`; `crates/reliquary-owner-lifecycle/src/lib.rs`; `src/runtime_inference/{runtime_host_gateway,runtime_host_rel_execution,runtime_host_gateway_host,runtime_host_owner_operation_journal,multiplexing_journal}.rs`; `src/imports/reconciliation.rs`; `src/rewrite/file.rs`; `src/runtime_inference/insomnia/completion.rs`; `crates/reliquary-{history,maintenance}/src/lib.rs`; `tests/current_format_boundary.rs`; `docs/crate-first-layout-spec-2026-10-25.md`; `docs/architecture/crate-map.json`.
