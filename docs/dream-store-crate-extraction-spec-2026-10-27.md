# Dream Store crate extraction — implementation specification

**Status:** Compiled integration and final verification complete (2026-10-27). See the [implementation evidence](dream-store-crate-extraction-evidence-2026-10-27.md).
**Branch:** `refactor/crate-first-restart`. **Baseline:** `d1de3eea` (Insomnia Store extraction); verify actual HEAD before cutting.
**Target:** `crates/reliquary-dream-store/`.
**Scope:** One compiled lower-store extraction only, with byte-identical current records, unchanged public behavior and no additional service. Preserve the unrelated working-tree edit in `docs/graph-crate-extraction-evidence-2026-10-26.md`.

## 1. Ownership result

Dream Store owns **durable Dream first-cycle Begin/journal evidence and its deterministic accepted replay state**, not Dream's inference, pair ranking, semantic settlement, graph editing, community generation, memory population selection or Freshness reinforcement policy. Owner Lifecycle and History must replay its records without importing higher Dream/Freshness services. Its direct Reliquary crate dependencies are limited to `reliquary-container`; current implementation declares no direct Reliquary crate dependency. No root, Memory Core, Graph, Freshness Store, Freshness service, Dream service, Runtime or History dependency is allowed. Any external `sha2` dependency is allowed only for the canonical population digest.

This is the support library listed in `docs/crate-first-layout-spec-2026-10-25.md` and `docs/architecture/crate-map.json`, not the later `reliquary-dream` semantic processor.

## 2. Observed source crosswalk

**Lower owner material:**
- `src/freshness_dream_journal.rs`: `DreamFreshnessJournal`, `InitialDreamPass`, `CVDREAM1` event codec/reader/writer, pass/root/settlement/birth/intent state, duplicate checks, retained projection and accepted graph-cut record values.
- `src/freshness/dream_begin_codec.rs` plus its tests: **`CVDREAM2`** Begin record, compact-cut and explicit population variants, exact framing and strict validation.
- `src/freshness/dream_population_digest.rs` and golden tests: canonical sorted Memory ID digest (membership only).
- The Dream-specific pure replay record/value portions currently exposed via root aliases or helper methods.

**Remain higher:** `src/facade/cva_dream_journal_replay.rs`, `cva_dream_population.rs`, `cva_historical_dream.rs`, `historical_import_base.rs`, and other root lifecycle/import/rewrite coordination; `src/runtime_inference/dream_*.rs` candidate, classifier, publisher, verifier, cooldown, graph and Freshness policy; Memory-local first-publication reader, direct birth/cohort authorization, source-global fence and per-frame physical-prefix selection.

Inspect actual `src/lib.rs` exports and all `InitialDreamPass`/`DreamFreshnessJournal` consumers before migration; file names are neighborhoods, not an instruction to move higher authority.

## 3. Hard boundary contracts — decide before moving call sites

1. **Canonical persisted Memory IDs:** Dream Store uses an exact fixed `[u8;32]` identity or owner-local newtype, with explicit conversion at root; no Memory Core import. Pass values crossing out of the store are **owned**, not borrowed root `InitialDreamPass` objects. This explicitly permits a narrowly scoped owned-return change to an internal Dream pass lookup if a lower-to-root borrowed view cannot be represented; preserve existing public product signatures. Do not create a duplicate root journal cache or shared generic ID crate.
2. **Freshness graph proof:** `freshness_dream_journal.rs` currently embeds `crate::FreshnessEventProof` in `accepted_graph_cuts` and `replay_payload`. Define one lower, wire-exact `AcceptedGraphCutProof` value containing graph version and optional community generation/graph-version; the root converts to/from the canonical Freshness proof **without changing encoding or validation**. The store validates intrinsic field relationships and recorded cut monotonicity. Freshness semantic correctness and actual accepted graph/community authority remain higher. Do not add a Freshness Store dependency as a shortcut.
3. **Compact Begin population:** `CVDREAM2` has a Memory-local cut and digest, but **a cut alone is not authority**. Root reconstructs the exact pre-Begin physical population from Memory-local first publications, source-global fence, exclusion of source/cohort, and contemporaneous birth/intent/Insomnia/external-source evidence, then compares the digest. Only after full authorization does it pass the materialized owned Begin to the lower journal. Neither the lower store nor its digest routine selects historical Memories.
4. **Replay order:** `cva_dream_journal_replay.rs` currently walks physical frames, calls `before_frame`, handles `CVDREAM2` via caller-materialized Begin, then `CVDREAM1` journal events, then advances the physical prefix. Preserve this exact order: **prefix strictly before Begin**; validate owner, complete framing, first revision, source exclusion, pass identity, accepted graph cut and settlement prior to mutating resident projections. Reject unsupported `CVDREAM*` versions rather than quietly ignoring.
5. **History/rewrite:** `replay_payload` maps turn, graph version and Freshness proof coordinates at higher owner-authorized destination cuts. Keep the lower operation pure and parameterized by validated scalar/owned coordinate mappings supplied by root; no higher-level callbacks that import root types, no second history writer, and no weakened match/intent/receipt proof.
6. **Write authority:** all append/sync remains through existing Container writer and higher caller transaction/owner grant. The lower store may produce exact bytes and apply accepted state; it must not independently decide to settle a pass, initialize Freshness, mutate Graph, or synthesize Memories. Preserve existing append/rollback/error ordering and read-only historical open.

**Avoid repeating the Insomnia API blocker:** for every root API returning `&InitialDreamPass`, `&MemoryId`, a collection of root types, or a closure accepting the journal, inventory the actual lifetime/type dependencies before implementing. Prefer owned boundary conversions for internal consumers, keeping all externally committed product API signatures unchanged unless a separate explicit review approves a change. No unsafe casts, generic adapter framework or permanent duplicate projection.

## 4. Persisted and semantic invariants

- **Current format:** byte-identical `CVDREAM1` event family and `CVDREAM2` compact/explicit Begin family; preserve owner UUID, pass ID, exact Memory IDs, roots, settlement and accepted graph/community fields. Reject truncated payloads, bad variants, duplicate/conflicting state, foreign owner and unknown family versions.
- **Population digest:** SHA-256 of `reliquary:dream-population:v1\0`, LE u64 set cardinality and sorted raw ID bytes. Preserve golden empty/one/two vectors and distinct byte order.
- **Lifecycle:** roots must belong to the Begin's accepted preexisting population; acceptance turn cannot predate Begin; accepted graph version cannot regress; settlement must have required accepted cut; intent mutation IDs/consumption and replay birth/settled state preserve exact idempotence/conflict behavior.
- **Recovery:** pending passes, settled IDs, accepted graph cuts, publication intents/births and completed first-cycle historical proofs replay identically for physical full/prefix scans and rewritten destinations, without interpreting future birth markers as past facts.
- **Resource behavior:** avoid unnecessary large materialized duplicate populations and repeated full-file scans within one Begin; no new persisted index/cache. Existing bounded population reconstruction remains higher.

`CVAFRS07` authoritative six-file corpus recovery is a distinct product gate, not automatically resolved by this extraction.

## 5. Bounded execution checkpoints

1. **Inventory and freeze:** status/HEAD, source references, worktree and baseline test families. Do not touch the unrelated Graph evidence modification. Enumerate root pass/journal borrowed return sites and current append/read/physical replay seams first.
2. **Extract wire values/codecs:** lower Memory ID/proof/Begin/Pass types and exact `CVDREAM1`/`CVDREAM2` codecs/digest with golden vectors. Compile lower with Container only (plus used external dependencies).
3. **Extract one journal owner:** pending/settled/roots/intent/birth/graph-cut state and deterministic ingestion. Validate duplicate/conflict, strict framing and proof field relationships. Remove duplicated root state once product cutover is integrated.
4. **Bridge root reconstruction:** caller authenticates physical Begin population and Freshness graph proofs, supplies owned IDs/Pass/Proof and maps output. Retain higher `before_frame`/prefix traversal order and no-append historical reads.
5. **Rewire consumers:** Cva/REL owner open, first-cycle Dream writes, History/import/rewrite/retained proof and tests use the single compiled journal; adjust internal owned return interfaces where required without new caches or public API changes.
6. **Acceptance and commit:** update Cargo/lock, root exports, documentation (`architecture.md`, `api.md`, `maintainer-map.md`, `storage-format.md`, Dream/Freshness subsystem docs), record actual commands/results in `docs/dream-store-crate-extraction-evidence-2026-10-27.md`; commit only after working product cutover and relevant tests. No new graph/ratchet infrastructure.

## 6. Verification matrix

**Lower crate:** `cargo check -p reliquary-dream-store --locked --offline`; `cargo test -p reliquary-dream-store --locked --offline`. Verify Cargo's sole direct internal dependency is Container.

**Wire and proof:** byte-equivalent begin/journal golden vectors, compact/explicit sets and digest, malformed framing/version, incorrect owner/pass/cut, roots outside population, duplicate/contradictory intent/receipt, graph/community proof mismatch, replay-settled/replay-birth and exact idempotence.

**Physical replay:** real REL write/reopen and prefix-before-Begin reconstruction, mixed local/global order, grouped cohort sibling exclusion, direct and Insomnia births, compact Begin vs explicit fallback, retained history proofs, historical import coordinate remap and prune/rewrite. Compare before/after replay results rather than just compiling the new crate.

**Root/CLI:** focused `dream`, `freshness_dream`, `dream_population`, `historical`, `import`, `rewrite` tests as applicable; `cargo check --workspace --locked --offline`, `cargo fmt --all -- --check`, `cargo test --manifest-path cli/Cargo.toml --locked --offline`. Run broader library/integration acceptance if root replay behavior changed and record exact counts.

**Completion:** compiled lower owner actually used in production; no root duplicate journal, no upward dependencies, exact current bytes and replay, live/historical/rewrite callers green, evidence and commit. Other corpus/architecture/documentation limitations must be reported separately rather than silently counted as passed.

## 7. Explicit exclusions

No semantic Dream processor extraction, Graph/Community redesign, Freshness policy change, Memory publication redesign, new Dream persistence version, new historical authority rule, external corpus migration, or generalized type/identity framework.
