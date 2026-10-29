# Ego semantic service crate — activation and implementation specification

**Status:** **Specified, not implementable as a correction-only extraction** (2026-10-28). The current `reliquary-ego` crate is an empty reservation; there is no implemented semantic Ego pipeline in Root to move. Ego's durable Store is separately implemented. This document is an implementation boundary and conditional stage plan, **not** evidence that Ego service behavior already exists or that its full design is settled.
**Repository/branch:** `C:\!bin\workspace\Reliquary-crate-first` / `refactor/crate-first-restart`.
**Verified baseline at drafting:** `4936804b` — integrated Dream semantic service. Recheck HEAD and status before any implementation.
**Unrelated edit to preserve:** `docs/graph-crate-extraction-evidence-2026-10-26.md`. Do not change, stage, discard, or commit it as part of Ego.
**Target:** `crates/reliquary-ego/`; separate existing owner: `crates/reliquary-ego-store/`.

## 1. Objective and hard scope decision

The next semantic destination is `reliquary-ego`, ultimately responsible for **Ego's user-relative semantic synthesis and selection policy**. Its owners and consumers must remain independent:

- **Ego Store:** already owns the actual current Identity, Personality, Anchor and singleton `EgoWebSynthesis` document persistence, exact compact codecs and replay.
- **Ego service:** reserved for selecting/auditing authorized source evidence, preparing provenance-linked summary content, generating summary text from bounded extracted content, planning summary refresh and providing budgetable semantic items, **when those capabilities are implemented**.
- **Context Engine:** eventually assembles the final model-facing context using the Ego service's authorized output, other context lanes and the actual usable input budget.
- **Product Root / Runtime:** remains responsible for authorization, source collection and validation, model endpoint configuration, durable store writes, scheduling/queues, accepted-use provenance and commit.

A **correction-only crate extraction cannot legitimately complete Ego today**. The source audit found no production Ego generation engine, selection controller, cross-chat selector, cross-REL summary consumer or model-context injection pipeline in `src/runtime_inference/**` or `src/ego/**`. The existing `Cva`/`Phylactery` methods in `src/facade/{cva_ego,phylactery_ego}.rs` merely read and write the Store and map product owner/lifecycle admission. Copying those methods into a service would violate lower ownership and create no semantic service.

**Do not mark the Ego crate extracted/complete by implementing re-exports, moving Store code, or adding an unconsumed toy policy.** To activate it as a working service requires explicit approval to implement the already designed product capabilities and to resolve the remaining open contracts below. Treat that as feature construction, not the continuation of the behavior-preserving service migrations.

## 2. Current source and implementation inventory

| Actual implementation | Current owner and disposition |
| --- | --- |
| `crates/reliquary-ego/src/lib.rs` and Cargo manifest | **Placeholder:** comment-only, no production API/dependencies; do not mistake a green compile for extraction. |
| `crates/reliquary-ego-store/src/ego_{model,store*,codec*,compact*}.rs` | **Keep in Ego Store:** canonical Ego Identity/Personality/Anchors/`EgoWebSynthesis`, source Memory version watermarks, current compact records/replay, historical revisions, activation and tombstones. This is already a compiled/store integration. |
| `src/facade/cva_ego.rs`, `src/facade/phylactery_ego.rs` | **Keep in Root:** supported REL/PHY public operations, active Identity, owner-specific admission, Memory watermark checks, Container-backed publication, error/lifecycle translation and borrowed singleton accessors. |
| `src/ego/ego_error.rs` and `src/ego/*tests.rs` | **Keep Root:** product error mapping and storage/reopen/REL-PHY integration tests; no semantic model engine here. |
| `src/facade/historical_ego_fingerprint.rs`, `src/imports/*ego*`, rewrite/reconciliation tests | **Keep higher owner:** historical cut, reconcile/rewrite and multi-owner proof. Do not move them into inference or duplicate their stores. |
| `src/runtime_inference/freshness_access.rs` | **Existing Root helper, not Ego service:** `ego_memory_freshness` is a non-reinforcing source Freshness read. `select_ego_routine_context` is a budget/rank helper with a `hard_keep` input, **not** the approved seven-stage Memory-Web Summary engine; do not silently promote its hard-keep behavior into the new service design. |
| `src/runtime_inference/agentic_memory_retrieval.rs` | Retrieval consumes existing Freshness source facts; this is not Ego's audit/controller implementation. |
| `docs/ego-web-synthesis-plan.md`, `ego-cross-chat-context-plan.md`, `ego-cross-rel-summary-inclusion-plan.md` | **Future design** with unresolved values and contracts. Plans are not current service source or proof of implementation. |

**Present persisted `EgoWebSynthesis` is a singleton text/revision/source-Memory-watermark record.** It is not the future PHY-owned, source-REL-keyed itemized Memory-Web Summary with sections, points and provenance. No in-place reinterpretation, automatic migration, or compatibility layering is authorized by this crate plan.

## 3. Hard service and authority boundaries

| Component | Allowed authority / non-authority |
| --- | --- |
| **`reliquary-ego` service (future)** | Deterministic whole-Web admission and ordered reduction, candidate/audit decision and controller metadata, summary extraction/creation validation, point/section provenance association, non-reinforcing **policy** for budgeting, and pure cross-chat/cross-REL candidate selection on **authorized immutable inputs**. May call supplied Switchboard endpoint for approved model stages; no owner writes. |
| **`reliquary-ego-store`** | Exact current documents, validation, compact format and replay. Any **new** per-(PHY, source REL) itemized summary must first have a separately approved owner-local record/key/provenance/byte contract. No semantic inference, permission checks, model routing or runtime scheduler. |
| **REL owner/Memory/Graph/Dream/Freshness** | Source Memory lifecycle, source-time and body identity, duplicate/supersession/canonical authority, Freshness grade at cut, Graph/Community proof and non-reinforcing authorized reads. Passing a Memory ID alone is not authorization. |
| **PHY owner** | User-private Identity/Personality/Anchors and future user-relative per-source-REL summary perspective; no silent copy into REL-shared durable records. Existing singleton behaviors unchanged until an explicit new-format design. |
| **Root/Runtime/Gateway** | Active PHY and source REL authorization, exact dependency/related-scope permissions, source selection/hydration, queue and source cut, Insomnia/Dream readiness, chosen model endpoint, durable write and receipts, accepted-use confirmation. No inference inside a held lower-store lock. |
| **Context Engine** | Final assembly/projection of Ego plus retrieval/current-session lanes and provider-specific usable-context budget. Ego can supply ordered budgetable content, not seize final prompt construction. |
| **Archive / Retrieval** | Authorized REL conversation/branch compactions, transcript/evidence lookup, optional Community routing; source searches and internal reads do **not** reinforce Freshness or advance activity. |

The current `docs/architecture/crate-map.json` permits at most these direct Reliquary dependencies for Ego: **Archive, Ego Store, Freshness, Graph, Memory Core, Retrieval and Switchboard**. Permissions are maxima, not mandatory dependencies. In the eventual implementation, import only actually used immutable value/pure interfaces. **No dependency on Root, Container, Owner Lifecycle, Runtime/Gateway, Context Engine, History, Import, Dream, or mutable owner stores.** Context Engine depends on Ego, not vice versa. Retain pure owner-specific IDs instead of creating an all-service contract crate. Cargo and focused checks, not new large architecture ratchets, enforce boundaries.

### 3.1 Product API split

Current public `Cva::ego_*` and `Phylactery::ego_*` methods stay in Root, with unchanged signatures, borrowed return types, revision semantics, and errors. Any future service evaluator has its **own** immutable input/output contracts; Root may adapt owner-authenticated facts and call it. Do not attempt to implement inherent `Cva`/`Phylactery` methods in the service (would introduce an upward dependency). Do not create a permanent duplicated Root projection or generic `EgoManager` just to make types fit.

## 4. Accepted product semantics to carry forward

These are **decided behavior constraints**, not a claim that all mechanics have been specified for implementation.

1. **Pipeline identity:** Candidate Selection → Candidate Audit → Summary Extraction → Summary Creation → Storage → Budgeting → Injection. Controller retains source `memoryID` and provenance through each stage; the final wording model receives extracted summary content rather than raw identifiers.
2. **Whole-Web-first:** if the complete authorized source REL Memory Web fits the audit budget, audit it unchanged—including archived, historical, duplicates, superseded and cold items. Otherwise apply **only in order** Archived → Historical → Dream-recognized duplicates → Superseded → entire Dormant band → entire Stale band. Recheck budget after each stage and stop reductions as soon as it fits. No partial-band opportunistic drops, no automatic exclusion of Fresh Memories; batch the rest for mandatory audit coverage.
3. **Freshness grades:** Fresh +1…+100, Stale −50…0, Dormant −100…−51; grades are relevance, not truth or membership. All source evaluation/retrieval/selection is non-reinforcing. Freshness boosts require a later host-proved accepted external consumption event, never an internal Ego read.
4. **Audit and provenance:** every Memory admitted after reductions is exposed to the audit at least once even when multiple batches are required. Audit can search the full authorized Web separately; that does not replace mandatory exposure. Preserve controller-bound `SummaryProposal` section IDs and `SummaryCandidates` associations. Model output cannot invent or mutate source authority.
5. **Summary form:** one maximum-detail itemized summary **per (active PHY, source REL)** with sections, independent points, generation/source cut, explicit source-Memory provenance and eventual token data. No pre-generated short/medium/long variants. A source REL summary is user-relative, not REL-shared.
6. **Freshness for summaries:** point grade is the arithmetic mean of the **distinct supporting source Memory grades**; section grade is the arithmetic mean of its points (equal point weight). Re-evaluate through non-reinforcing reads at budget time; do not regenerate prose on injection merely because available budget changes.
7. **Refresh boundary:** first REL eligibility at 25 eligible durable Memories; later due by accepted-turn growth since last success with percentage+absolute cap **not yet calibrated**. A queued source cut waits for required Insomnia/Dream settlement; later traffic cannot indefinitely move the cut. No wall-time backstop. PHY-local refresh follows PHY Memory mutation progress, not REL activity or Graph churn.
8. **Cross-chat:** use existing compaction/tail evidence from the **current REL**, rank other conversation/branch histories by latest activity, preserve branch lineage, and consume only remaining Ego budget. Never create another summarizer or mix current-session compaction into Ego.
9. **Cross-REL:** include only **existing active-PHY** summaries from source RELs explicitly authorized by the host. Dependencies are not a license for arbitrary reverse/sibling traversal. Keep source identity/attribution, avoid recursive inclusion and never synthesize a default mega-summary.
10. **Budget and assembly:** approximate design ceiling 20% of provider-usable input context, **not** a fixed allocation or required fill. Identity/Personality/Anchors and authorized Web Summary precede Cross-chat. Current-session compaction/live tail are outside Ego. Context Engine owns actual assembly.
11. **Store and historical behavior:** do not alter current `CVAECR01`, `CVAEDL01`, `CVAETM01`, `CVAEIDA1` bytes, current Ego revision and terminal semantics, REL/PHY admission, historical cuts, reopen or rewrite. `CVAFRS07` six-file corpus recovery is an independent gate.

## 5. Unresolved contracts that prevent a truthful full implementation

**No implementer may silently choose defaults and then report Ego complete.** The current design documents explicitly leave open:

- audit batch size/overlap, source-set criteria and subject continuity; precise Candidate Selection/Audit trimming;
- source evidence-to-point mapping details, handling unresolved contradictions, and section/point coherence dependencies;
- **new PHY-owned (source REL keyed) itemized summary durable identity, codec, history, revisions, source-cut/provenance authority and migration relation to the existing singleton**;
- exact maximum-detail generation ceiling, token accounting, deterministic point/section ordering and budget tie-breaks;
- REL/PHY refresh percentage thresholds, caps and affected-source triggering; model retry/abstention and update behavior;
- Personality synthesis/generation/edit/refresh input policy (storage exists, generator does not);
- precise provider usable-input reservation, cross-chat tail truncation and currently open conversation behavior; and
- cross-REL relevance/selection thresholds, any reverse authorization mechanism and disclosure constraints.

Resolve each from a separate, focused reviewed decision **before the affected implementation slice**, not by importing speculative structures from retired plans. Preserve the already accepted absence of generic hard-keep lanes, `CandidateResolver` frameworks, tier taxonomies or invented contradiction resolution. The seven-stage plan is binding in sequence, not permission to fabricate its missing policy.

## 6. Bounded implementation gates (conditional feature implementation)

These gates define an actionable order **after feature construction is expressly authorized**; they do not certify that such authorization or prerequisite decisions already exist. Each stage has one owner and one measurable witness. Slice changes into reviewable files (prefer under 200 lines, never over 300).

| Gate | Work | Required acceptance |
| --- | --- | --- |
| **E0 — freeze present behavior** | Inventory actual Root/Ego Store sources, public borrowed and owned `Cva`/`Phylactery` signatures, source cut/Freshness readers, current stack test matrix, status/HEAD. Confirm service is empty, store already integrated and source-only worktree. | Source inventory; no invented extraction count or Graph doc mutation. |
| **E1 — classify scope and decision gates** | Write explicit accepted-vs-unresolved matrix and obtain approval to implement new Ego semantics. Where a decision is open, gate the corresponding later slice instead of choosing one. | Approved scope; no net-new inference/persistence sneaked into correction-only refactor. |
| **E2 — value-only service kernel** | Establish `reliquary-ego` entrypoint and immutable owner-authorized input types **only for the first implemented policy**; test compiled external consumer. Require actual behavior in the crate, no placeholder/re-export-only completion. | Cargo DAG valid, stable exact canonical owner IDs, no Root/Container/store-handle import. |
| **E3 — working-Web admission** | Implement **decided only** whole-Web fit, ordered reduction, full-band logic and mandatory audit batch coverage over caller-provided lifecycle, Dream and Freshness facts. No search, Graph or owner writes inside the service. | Frozen input fixtures prove whole-Web audit, overflow stages, Fresh never dropped to meet a single window, and one-visit minimum in batching. |
| **E4 — audit/extraction controller** | After audit selection/overlap decisions, implement bounded model selection/audit/extraction preserving section proposals and controller-owned source provenance. Root authorizes all external search/hydration. | No lost/foreign IDs, mandatory coverage, unsupported model output rejected; deterministic replay of identical input and simulated endpoint responses. |
| **E5 — summary creation** | After section/point and conflict policy is approved, generate itemized maximum-detail sections/points from extracted source content; pair generated text to controller metadata and validate source provenance. | Verbatim source linkage retained, no invented Memory authority, no raw ID requirement for creation model, bounded retry/backpressure, malformed output rejection. |
| **E6 — durable summary contract and publication** | **First approve** new PHY-owned source-REL summary shape and byte contract separately; implement in its actual lower owner. Root validates source and active-PHY rights, frozen cuts and publication before writing. | Exact current/retained format unchanged, exact new format/golden vectors, owner isolation, conflict/retry/partial-write and reopen tests. |
| **E7 — dynamic budgeting** | Budget existing maximum-detail sections/points at caller-supplied actual input budget using current non-reinforcing grades, valid source lifecycle and coherent item selection; never run summary inference during injection. | Average grades and section averages exact; zero, insufficient, generous and changing-budget results deterministic; no source write/reinforcement. |
| **E8 — cross-chat selection** | After open tail/oversize and allocation decisions, consume authenticated current-REL branch-specific compactions with latest-activity ordering; current-session content excluded. | Branch forks, resumed old chats, empty/stale compactions, tight budget, no extra summarization or cross-REL transcript exposure. |
| **E9 — cross-REL inclusion** | After host authorization contract and per-REL summary persistence, select/dedupe/attribute only active-PHY source REL summaries from explicitly authorized ambient or reverse scopes. | Denied sibling/reverse links, other PHY, duplicate/cyclic candidate sets and missing source all fail closed; no combined synthesis. |
| **E10 — Root/Context Engine integration** | Root orchestrates source cuts, endpoint routes, queue and store publication; Context Engine assembles under provider budget. No service-owned runtime or automatic Freshness reinforcement. | Live REL/PHY paths and current-session exclusion, exact source/acceptance receipts, no owner unlock or cross-context data leak. |
| **E11 — verification and commit** | Run focused service/API tests, cross-owner integration, current-format/History/REL-PHY tests, workspace/CLI full checks, docs and architecture; write evidence and commit only completed stages. | Actual tests and source ownership proven, unavailable corpus and unresolved decisions labeled separately; no full-Ego completion claim before E0–E10. |

**Stop rule:** If operating under the present *extraction-only* mandate, complete E0–E1 and **stop**. E2–E10 create missing product behavior and are not a behavior-preserving extraction. Do not create a hollow crate to satisfy milestone reporting.

## 7. Acceptance commands and evidence discipline

Before implementation, record a baseline using relevant currently existing owner tests: `cargo test -p reliquary-ego-store --locked --offline -j 1`, Root `cargo test --lib ego --locked --offline -j 1`, historical Ego/rewrite/reconciliation/REL/PHY cases, `cargo check --workspace --all-targets --locked --offline -j 1`, CLI `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`, and `cargo fmt --all -- --check`. Do not report prior counts as fresh results.

For each implemented semantic gate, add **real cross-crate behavioral consumer tests**: complete deterministic input → actual output, malformed/authority boundary → rejection, plus Root caller integration. Use only existing tests plus boundary-specific cases; avoid artificial test-count growth and duplicate legacy tests. Assert no new persisted bytes unless an explicitly approved owner-store format is required, and verify any new format with replay/golden and historical-transform tests.

Before claiming final integration, rerun the workspace check, committed service + Store tests, focused owner/admission/context tests, full library (including actual REL and PHY reopen and retry), CLI, formatting, `ddocs check --indexes --links`, current architecture checks, direct Cargo dependency tree and `git diff --check`. Review source ownership with scoped `rg`, not a new repository-wide ratchet. Record Windows documentation-policy `target` path failures or stale Arcana evidence explicitly; never count an interrupted test as passed. The promoted `CVAFRS07` corpus gate is separate unless independently executed. Record exact HEAD, pass counts, omitted fixtures, commit and only pre-existing dirty files in `docs/ego-service-crate-implementation-evidence-<date>.md`.

**Definition of completion:** a real, consumed service with accepted policy, complete authorized integration, preserved owner semantics, explicit new-data format where necessary, tests, docs and commit. **Today's documented Ego Store does not satisfy this**, nor does an unused pure function in an otherwise empty semantic crate.

## 8. Explicit exclusions and authority

No extraction of `EgoStore`, migration of existing singleton synthesis to the future itemized data model, new implicit PHY↔REL data sharing, new global Memory/Freshness clocks, new default cross-REL rights, broad Context Engine buildout, unapproved Personality generator, automatic write on open, generic Ego manager/context budget framework, or implementation of unresolved contradiction rules.

Governing documents: [Ego state](subsystems/ego.md), [Ego Store extraction specification](ego-store-crate-extraction-spec-2026-10-26.md), [Memory-Web Summary design](ego-web-synthesis-plan.md), [Cross-chat plan](ego-cross-chat-context-plan.md), [Cross-REL inclusion plan](ego-cross-rel-summary-inclusion-plan.md), [Ego status inventory](ego-status-decision-inventory-2026-10-14.md), [crate-first architecture](crate-first-architecture.md), [crate map](architecture/crate-map.json), [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md).

**Specification only.** This commit changes no Rust, Cargo, disk format, model behavior or runtime operation.
