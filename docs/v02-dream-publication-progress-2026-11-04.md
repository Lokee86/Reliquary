# V02 — Dream publication progress (2026-11-04)

**Status: IN PROGRESS — not accepted.** V02 is the Application workflow that admits S03's pure Dream decisions to a REL/PHY owner and durably settles Graph, Memory lifecycle, evaluated-pair and first-cycle Freshness evidence. S03's evaluator and G1 storage are already accepted; V01 ingestion remains an acceptance prerequisite.

## Implemented: one-effect zero-link publication

- Application exposes `commit_dream_zero_link` with `DreamOwnerWriter::Rel/Phy`. Only a draft created by the checked Dream preparation workflow can be passed from outside Application; draft fields are no longer publicly mutable or constructible.
- Publication requires one classifier-derived `none` decision, two observed Memories, no unsupported relation/lifecycle effect and no verifier or fabricated evidence. Positive or multi-pair proposals fail closed **before writing**. It does not treat an unverified positive relation as a zero-link result.
- Owner-session revalidates the **exact authorized owner ID**, current Graph version and both current Memory revision/body identities; archived, tombstoned, missing or pruned source bodies are rejected. All validation occurs while holding the canonical writer lease, immediately before the durable evaluated-pair write.
- The existing compact Dream pair record persists and replays for REL and PHY. Repeating the identical accepted zero-link result returns `false` without another frame; an interruption during the actual append uses existing Container writer-recovery requirements.
- Tests prove REL/PHY durable reopen, byte-stable retry, rejected cross-owner authority, refusal of positive effects without mutation, stale Graph version and stale Memory revision refusal.

This is a **single-effect publication**, not necessarily a single physical frame: new Memory identity reference frames may precede the pair marker. Complete unpaired references are replayable, and uncertain appends require reopening or explicit recovery. It deliberately does **not** issue independent multi-domain store writes and call them an atomic Dream transaction.

## Still required for V02 acceptance

1. Design and implement the owner-session **single recoverable grouped Dream publication** for accepted Graph relations, duplicate/supersession, Memory lifecycle and the evaluated-pair receipt. Validate the current cut, apply only after durable commit, replay from the same carrier after failure, and forbid stale/repeated transitions. Do not add separate uncoordinated Graph/Memory write calls.
2. Integrate exact first-cycle Dream Begin/settled evidence with Freshness birth/admission and eligible initial relation evidence. **Zero linked relations must still settle the initial pass**, including a pass with no model candidate; idempotent birth must not re-enroll or apply a rewire bonus. The new one-pair marker alone does *not* satisfy first-cycle settlement.
3. Handle rejected/uncertain classifications and multi-pair frontiers with explicit completion and retry policy. Preserve accepted verified duplicate/canonical policies and user-origin relation protection.
4. Complete validated Chronos interpretation of stored Memory temporal inferences and demonstrate bounded **read cost** for candidate gathering. The previously accepted S03 streaming frontier bounds retained scores but currently scans all owner Memory identities.
5. Verify V01 dependency, REL/PHY history/reopen, interruption-after-sync recovery, negative owner/graph/body tests and the full V02 acceptance matrix before closing V02 or running G2.

## Verification

Focused V02 publication tests: **5 passed** (3 PHY, 2 REL). Full locked offline workspace suite: **391 tests passed, 0 failed**, including doctests. `cargo check --workspace --locked --offline -j 1`, `cargo fmt --all --check`, `python scripts/check_architecture.py` and `git diff --check` all passed. The production graph remains **23 packages, 76 edges**. This file is continuation evidence; the canonical tracker must retain V02 as **in progress**, not completed.

The existing architectural invariant remains: owner-session owns physical mutation; Application owns publication decisions; semantic Dream remains pure; no direct Gateway/Graph/Freshness store imports are added to Application.
