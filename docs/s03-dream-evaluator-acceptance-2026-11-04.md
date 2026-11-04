# S03 — Dream evaluator acceptance (2026-11-04)

**Scope:** S03 of the canonical 44-increment rebuild tracker: a **pure Dream evaluator** that returns candidate decisions, duplicate/canonical/lifecycle intents and community name candidates. The owner-qualified **Application** workflow (V02) remains responsible for validation, durable Graph/Memory/Freshness publication, first-cycle settlement, crash recovery and end-to-end semantic integration. S03 must not perform any owner store mutation.

**Acceptance decision: PASS for S03's specified pure evaluator contract.** Prerequisites S01, C03 and C04 were accepted before this decision. No G2 acceptance is implied.

## Verified behavior

- Classifier v3 canonicalizes pairs, enforces typed relation/direction and exact per-side quoted Memory evidence. Verifier v1 explicitly accepts, rejects or withholds uncertain conclusions. Invalid model output has bounded retries, while backpressure fails immediately.
- Frozen deterministic responses in `crates/reliquary-dream/src/acceptance_tests.rs` cover accepted, independently verified DuplicateOf evidence with the exact newer→older relation intent; rejected duplicate evidence with **no** accepted relation/lifecycle effect; and a valid zero-link classification with **no fabricated Graph link**. The zero-link lifecycle case deterministically advances an extracted Memory to knowledge through `plan_lifecycle`, without writing.
- Existing Dream tests cover supersession/lifecycle intents, duplicate representative selection, canonical promotion policies, community member/name selection, strict classifier/verifier schema and negative response handling, bounded pair-level concurrency and stable evaluation order.
- Streaming `DreamCandidateFrontier` retains only up to `semantic_limit + prior_semantic_quota + lexical_limit + temporal_limit` scored candidate entries. It explicitly preserves the prior-semantic lane independently of the main semantic truncation. Parity tests compare full-set lane fusion and bounded frontier results for **3,000** candidates per configuration, across multiple caps/source times/ties, checking identity and exact fused score.
- Application uses this pure frontier while paging authorized REL/PHY Memory IDs and reading current owner facts, Graph relationships and profile-qualified vectors. It retains only the bounded candidate frontier, not the complete Memory Web. The read-only owner-ranked proposal remains separately tested for authorization, model-call selection, evaluated-pair exclusion across reopen and missing source vectors.
- Source inspection of the 15 production Dream modules finds **no** GraphStore/FreshnessStore/Container writer access, grouped publication or owner-store mutation. The service returns value proposals only.

## Verification

From `C:\!bin\workspace\Reliquary-rebuild`:

- `cargo test -p reliquary-dream -p reliquary-application --locked --offline -j 1`: **46 tests passed**, no failures, plus doc tests.
- `cargo test --workspace --locked --offline -j 1 -- --test-threads=1`: **386 tests passed**, zero failures (47 unit/integration/doc suites).
- `cargo check --workspace --locked --offline -j 1`: PASS.
- `cargo fmt --all --check`: PASS.
- `python scripts/check_architecture.py`: PASS (23 production packages, exactly 76 allowed edges, no new dependency cycle).

## Remaining work belongs to the Application and product gates

- **V02:** Revalidate a Dream proposal against the current owner cut, atomically publish accepted relation/lifecycle/evaluated-pair/first-cycle effects through owner-session with durable recovery, and verify replay/zero-link behavior. S03 does not own that mutation.
- The current owner-ranked **Application** candidate scan still visits the full authorized Memory identity list (bounded resident memory, **not** bounded total read cost); production-scale index-assisted candidate gathering remains a V02 performance concern.
- When owner Memory records have stored model-derived temporal inference, the current **Application** reader returns `TemporalInferenceUnavailable`; a validated Chronos interpretation policy for that stored inference is required before V02 end-to-end acceptance. This is not an excuse to treat model expressions as verified timestamps.
- G2 and V01–V07 acceptance remain separate and pending. The frozen test outputs above are deterministic contract cases, **not** proof that a live model provides accurate classifications.

**Outcome:** S03's pure semantic service can be marked completed. Continue with dependent workflow V02 (and other prerequisites) rather than adding Graph or Freshness writers to the Dream crate.
