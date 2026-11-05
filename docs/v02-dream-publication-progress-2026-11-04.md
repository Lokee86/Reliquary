# V02 — Dream publication progress (2026-11-04)

**Status: IN PROGRESS — not accepted.** V02 is the Application workflow that admits S03's pure Dream decisions to a REL/PHY owner and durably settles Graph, Memory lifecycle, evaluated-pair and first-cycle Freshness evidence. S03's evaluator and G1 storage are already accepted; V01 ingestion remains an acceptance prerequisite.

## Implemented: one-effect zero-link publication

- Application exposes `commit_dream_zero_link` with `DreamOwnerWriter::Rel/Phy`. Only a draft created by the checked Dream preparation workflow can be passed from outside Application; draft fields are no longer publicly mutable or constructible.
- Publication requires one classifier-derived `none` decision, two observed Memories, no unsupported relation/lifecycle effect and no verifier or fabricated evidence. Positive or multi-pair proposals fail closed **before writing**. It does not treat an unverified positive relation as a zero-link result.
- Owner-session revalidates the **exact authorized owner ID**, current Graph version and both current Memory revision/body identities; archived, tombstoned, missing or pruned source bodies are rejected. All validation occurs while holding the canonical writer lease, immediately before the durable evaluated-pair write.
- The existing compact Dream pair record persists and replays for REL and PHY. Repeating the identical accepted zero-link result returns `false` without another frame; an interruption during the actual append uses existing Container writer-recovery requirements.
- Tests prove REL/PHY durable reopen, byte-stable retry, rejected cross-owner authority, refusal of positive effects without mutation, stale Graph version and stale Memory revision refusal.

This is a **single-effect publication**, not necessarily a single physical frame: new Memory identity reference frames may precede the pair marker. Complete unpaired references are replayable, and uncertain appends require reopening or explicit recovery. It deliberately does **not** issue independent multi-domain store writes and call them an atomic Dream transaction.

## Implemented: one grouped positive-relation publication (continuation)

- Application now exposes `commit_dream_relation` for a checked, classifier-derived **single Topical/Factual/Causal/Recurrent relation** with exact Memory-side evidence. The new path rejects unsupported lifecycle effects, duplicate/supersession, multi-pair proposals and mismatched owner/effect inputs before mutation. The semantic Dream crate remains read-only.
- Owner-session owns a new `RELDRG01` carrier: one accepted **active Dream Graph relation and its evaluated-pair receipt share a single synced frame**. REL and PHY writers validate the authorized owner, retained Memory identity/revision/body, current Graph version, write lease and existing pair receipt. In-memory Graph and pair projections update only after the frame is durable.
- REL/PHY reopen replay both effects from that one carrier. A retry of its exact accepted cut after Graph advancement is a byte-stable no-op; conflicting/stale cuts fail closed. Unaware standalone Graph readers reject grouped carriers rather than silently dropping relation history.
- Focused positive REL/PHY tests prove reopen and idempotent retry; negative PHY tests prove cross-owner and stale Memory revision rejection without mutation. A post-sync/pre-projection crash simulation writes the carrier and deliberately skips both in-memory updates, then reopens a PHY and verifies both recovered effects.
- **Deliberate limit:** This first grouped carrier accepts Memory **revision 1 only**, a single active non-lifecycle relation and one pair; it does not provide a general grouped lifecycle, multi-pair, first-cycle or Freshness transaction. More complex cuts remain unsupported, not silently decomposed into independent writes.

## Still required for V02 acceptance

1. Extend the now-proven **single-pair grouped DuplicateOf and Supersedes carriers** to bounded multi-pair publication and complete canonical selection policy. A supersession Memory lifecycle revision, Graph relation and pair receipt already commit together; broader lifecycle choices and multi-pair coalescing remain unimplemented. Preserve the one-frame commit/replay invariant and user-origin relation protection; do not use independent store writes.
2. Integrate exact first-cycle Dream Begin/settled evidence with Freshness birth/admission and eligible initial relation evidence. **Zero linked relations must still settle the initial pass**, including a pass with no model candidate; idempotent birth must not re-enroll or apply a rewire bonus. Neither the original zero-link marker nor the new positive grouped carrier satisfies first-cycle settlement.
3. Handle rejected/uncertain classifications and multi-pair frontiers with explicit completion and retry policy. Preserve accepted verified duplicate/canonical policies and user-origin relation protection.
4. Complete validated Chronos interpretation of stored Memory temporal inferences and demonstrate bounded **read cost** for candidate gathering. The previously accepted S03 streaming frontier bounds retained scores but currently scans all owner Memory identities.
5. Verify V01 dependency, REL/PHY history/reopen, interruption-after-sync recovery, negative owner/graph/body tests and the full V02 acceptance matrix before closing V02 or running G2.

## Verification

At this grouped-positive checkpoint: Application tests **34 passed** (new positive REL/PHY acceptance, reopen, retry and negative cut tests included); owner-session post-sync replay test **1 passed**. Full locked offline workspace suite **395 passed, 0 failed** across 47 suites including doctests. `cargo check --workspace --locked --offline -j 1` and `python scripts/check_architecture.py` passed; production graph remains **23 packages, 76 edges**. Formatting and whitespace checks were performed after edits before commit. Earlier zero-link-only checkpoint had 391 tests. This is continuation evidence; canonical tracker must retain V02 as **in progress**, not completed.

The existing architectural invariant remains: owner-session owns physical mutation; Application owns publication decisions; semantic Dream remains pure; no direct Gateway/Graph/Freshness store imports are added to Application.

## Continuation: verified duplicate and supersession publication

- **DuplicateOf:** the existing `RELDRG01` one-frame carrier now supports a classifier-derived **verifier-accepted** DuplicateOf edge with its evaluated-pair receipt. It does not invent canonical Memory metadata or archive either Memory; no unverified duplicate is admitted.
- **Supersedes:** a new owner-private `RELDRS01` carrier contains the typed owner/Graph/Memory cut, one accepted directed Supersedes Graph change, its evaluated-pair receipt and a **standard Memory metadata delta** transitioning the older Memory to `archived` with `superseded_by = newer`. The Memory owner prepares and replays the delta through its normal revision/clock/mutation rules; there is no parallel lifecycle projection.
- Application requires one accepted verifier result with matching classifier evidence and exactly one matching lifecycle intent for Supersedes. REL and PHY owner writers recheck the live Graph, body, revision, owner and writer lease before a **single physical append and sync**, then update in-memory Memory, Graph and pair projections. Repeated identical submissions are byte-stable no-ops, conflicting timestamps/cuts reject.
- REL and PHY tests prove accepted-history reopen and exact lifecycle revision. Negative tests reject wrong owner, stale Graph and withheld verifier decisions without changing bytes. A simulated post-sync/pre-in-memory-apply interruption reopens the one carrier and reconstructs all three effects from disk. Standalone Memory/Graph readers that cannot decode the owner-only carrier reject it rather than silently ignoring lifecycle effects.
- **Limit:** these carriers support **one evaluated pair per commit**. Only the supersession's defined `archived/superseded_by` metadata transition is implemented, not multi-pair/canonical selection, first-cycle Freshness settlement, accepted-uncertain completion policy or temporal inference admission. V02 and G2 remain **in progress**, not accepted.

**Verification:** `cargo test --workspace --locked --offline -j 1 -- --test-threads=1` **401 passed, 0 failed**, including doctests; focused Dream publication tests **13 passed**, and the owner-session post-sync lifecycle crash-replay test passed. The architecture, formatting, workspace check and whitespace gates were rerun for the recorded commit.
