# Insomnia semantic validation — 2026-08-24

Parent index: [Documentation index](INDEX.md)

## Purpose

This record captures the small-fixture validation of Insomnia semantic extraction, the evolution from the original two-pass authority-ledger experiment to the frozen three-model-pass tuning harness, model/provider comparisons against `gpt-5.6-sol`, `stealth/ox-alpha`, and `gpt-5.6-luna` at low reasoning, and the 2026-09-07 Luna medium/high reasoning follow-up on the frozen 11-Episode fixture.

At the time of this 2026-08-24 validation, the authoritative Insomnia runtime was extractor contract `v3-0`: clause-level semantic selection followed by wording-only synthesis, while the final tuning harness added a separate metadata-classification pass. Current runtime contract `v3-4` has since ported the fixed-group classification seam, added dedicated User/Project owner routing, and added post-wording Entity-mention enrichment constrained to verbatim final-Memory text; lexical Memory routing is now derived deterministically from full Memory text rather than model-extracted. See ADR 0023 and the Perception subsystem plan. The measurements below remain historical semantic-selection/model evidence and are not recomputed for those later routing/enrichment changes. The implementations in `examples/` and `tools/` remain benchmark/provider-compatibility harnesses rather than production execution paths.

## Overview

At the time of this validation, the focused fixture established `gpt-5.6-sol` at low reasoning as the selected semantic selector for the tested tuning architecture. The final tuning architecture is **Sol-low semantic ledger → deterministic fixed groups → Luna-low metadata classification → Sol-low wording**. Controlled metadata evaluation improved metadata agreement from `71.4%` to `93.3%` without changing the 51 frozen semantic groups. Final semantic tuning produces approximately `94.7–100%` anchor fidelity, `93.3–100%` durable-state coverage, and `100%` omit cleanliness on the evaluable tuning cases; remaining misses move between runs rather than exposing a stable prompt defect. Routine prompt tuning is therefore frozen.

## Fixture and evaluator

Routine tuning uses:

- `corpus/insomnia-tuning-v1.jsonl`: 11 isolated Episodes / 314 turns.
- `corpus/insomnia-gold-v3-tuning-v1.json`: 19 tuning cases, comprising 15 retained-state cases and 4 omit cases.
- `tools/evaluate_insomnia_gold.py`: reports anchor fidelity, durable state coverage, omit cleanliness, assistant-authority provenance, grounding provenance, metadata, and content/receipt guards. By default it now discovers the sibling `run.json` and scopes gold cases to source turns actually presented to that run; `--unscoped` is retained only for deliberate whole-gold diagnostics.

This scoping correction matters: the 11-conversation fixture contains only 19 of the 40 gold-v3 anchor source nodes. Scoring all 40 cases against the trimmed fixture had produced misleading `52.5%` anchor / `43.8%` state figures. The correctly scoped controlled v7 baseline is `89.5%` anchor (`17/19`), `93.3%` state coverage (`14/15`), and `100%` omit cleanliness (`4/4`). Semantic-target equivalence remains a manual/model judgment.

## Architecture under test

The final tuning harness decomposes extraction into three model calls with deterministic ownership between them:

1. **Semantic authority/disposition ledger — Sol-low:** account for every user turn at clause level; decide `retain`, `omit`, or `superseded`; assign authority/provenance; preserve the actual durable proposition and modality.
2. **Metadata classification — Luna-low:** classify each already-frozen semantic group with `category`, `type`, and `lifecycle`. It cannot add, remove, merge, split, rewrite, or re-own semantic state.
3. **Memory wording — Sol-low:** write title/body prose for every required frozen group and nothing else.

Deterministic grouping occurs before metadata classification. This is critical: an earlier implementation classified ledger entries before grouping, which allowed metadata changes to alter group identity and changed 51 semantic groups into 54. The final harness freezes groups first, then applies metadata to those group IDs and synchronizes it back to ledger entries.

Pass 1 remains the semantic gate. Neither metadata nor wording may resurrect omitted/superseded state or change group membership.

## Deterministic infrastructure ownership

Several fields proved inappropriate to leave to probabilistic byte reproduction or unconstrained identifier generation:

- Dynamic per-Episode schemas constrain user source IDs, assistant authority IDs, and grounding IDs to real Episode turns.
- Reliquary owns byte-exact provenance. If a model selects the correct source ID but slightly changes Markdown/punctuation in a quote, the harness repairs the quote from the authoritative source turn.
- Direct/correction authority cannot acquire assistant authority provenance merely because synthesis emits one.
- Self-grounding is stripped.
- Experimental resume support reuses completed ledger/synthesis artifacts instead of rerunning already-completed inference.

The Ox/Nous compatibility harness additionally requires every user-turn ID as a tool-schema key because Ox repeatedly skipped pure-question turns when completeness was expressed only in prompt text.

## Pass-1 tuning sequence with Sol-low

The initial full 11-Episode two-pass Sol run produced 94 Memories and scored:

| Metric | Initial two-pass |
| --- | ---: |
| Anchor fidelity | 84.2% |
| State coverage | 100.0% |
| Omit cleanliness | 50.0% |
| Authority | 79.3% |
| Grounding | 62.1% |
| Metadata | 75.0% |
| Guards | 96.6% |

The important positive result was immediate `100%` durable-state coverage. The dominant problem was over-permissive disposition/provenance rather than failure to discover state.

Pass 1 was then tuned specifically around measured failures:

- pure numbered execution/checkpoint receipts are omitted;
- recalled plans, challenges, open questions, and alternatives do not become decisions;
- a tag question may retain an independently asserted requirement (`X has to be Y, right?`);
- mixed checkpoint/status turns are split so durable status survives while numbered progress is stripped;
- terse assent to one clear assistant proposal may be a durable adoption;
- authority provenance and grounding provenance are separate roles;
- direct/correction state does not use assistant authority;
- grounding is used only to resolve a genuinely unclear referent;
- older overlapping state is superseded only when later user authority materially replaces a value, mechanism, requirement, or other part of the older clause; equivalent restatement/corroboration does not erase the original anchor, and a mixed obsolete clause is not salvaged by generalizing away the replaced detail.

An intermediate strict run (`v4`) reached 100% anchor fidelity, state coverage, omit cleanliness, grounding, and guards, but over-pruned part of the mixed Prompt-72 status. A broader classification experiment (`v5`) regressed and was rejected. The retained `v6` contract restored the mixed status while preserving the important selection behavior.

## Sol-low v6 result

The tested Sol-low v6 run produced 54 Memories:

| Metric | Sol-low v6 |
| --- | ---: |
| Anchor fidelity | 94.7% |
| State coverage | 100.0% |
| Omit cleanliness | 100.0% |
| Authority | 95.8% |
| Grounding | 100.0% |
| Metadata | 60.0% |
| Guards | 100.0% |

The Prompt-72 turn is semantically handled as intended: `everything seems to be working OK so far` and the remaining visual bugs/oddities are retained, while the `prompt 72 completed` checkpoint framing is removed.

The remaining anchor loss is ShipStats: the earlier correction/decision source was treated as superseded by a later valid direct restatement. Durable state coverage therefore remains 100% even though exact anchor fidelity is 94.7%.

The remaining authority noise exposed a second architectural issue: the original experimental pass 2 could choose provenance/classification independently from pass 1. In observed cases, a correct direct ledger entry was synthesized with an invented assistant authority source. That synthesis ownership leak motivated the fixed-group `v3-0` production boundary described below.

## Ox Alpha through Nous

Model: `stealth/ox-alpha`, low reasoning.

### Transport findings

Direct Nous endpoint:

`https://inference-api.nousresearch.com/v1/chat/completions`

Normal `response_format` / JSON-schema mode is not reliable for this model/provider combination. Ox repeatedly returned coherent Markdown despite a strict JSON-schema request. Earlier requests also encountered upstream capacity/timeout behavior.

Forced tool/function calling is usable. The experimental client now supports streamed `tool_calls` arguments and retries. Additional structural safeguards were necessary because Nous did not consistently enforce all declared required/enumerated fields:

- required per-user-turn tool keys for ledger completeness;
- deterministic quote canonicalization;
- deterministic repair of invalid structural authority values when the semantic/provenance state uniquely determines the valid value;
- inheritance of missing synthesis category/type/authority fields from the matching retained ledger only when unambiguous;
- adoption authority can inherit the immediately preceding assistant parent when the model selected adoption but omitted the source ID;
- resume support preserves completed Episode artifacts.

A one-Episode end-to-end probe succeeded with 11 Memories in 90.545 seconds. The full 11-Episode run completed with 62 Memories.

### Ox/Nous score

| Metric | Ox Alpha / Nous |
| --- | ---: |
| Anchor fidelity | 94.7% |
| State coverage | 100.0% |
| Omit cleanliness | 75.0% |
| Authority | 90.9% |
| Grounding | 86.4% |
| Metadata | 62.5% |
| Guards | 95.5% |

Ox is competitive on recall but more permissive than tuned Sol. Two clear pass-1 semantic errors remained:

- `prompt-41-receipt`: Ox explicitly retained `completed through Prompt 41` as durable project progress.
- `prompt-72-mixed-status`: Ox retained the durable status but left `prompt-72 milestone` framing in the proposition.

ShipStats was also classified as adoption with assistant authority rather than user correction plus grounding.

The creatureServer authority/grounding mechanical failure understates the semantic output: Ox emitted two Memories from the same mixed user turn, including a correct direct `creatureServer is mothballed` Memory with correct grounding, but the evaluator selected the sibling surface-review adoption as the anchor match.

## Luna-low comparison

The same v6 semantic contract was run with `gpt-5.6-luna` at low reasoning and four workers.

The raw Luna run omitted six user turns from two ledgers. None of those turns were gold anchors; they were a pasted Prompt 64K, pure questions, transient cleanup/tooling complaints, and pasted terminal output. For completion of the model comparison only, those six missing turns were deterministically inserted as explicit `omit` ledger entries under the existing pass-1 rules. This repair did not supply any missing gold state.

The completed Luna run produced 48 Memories:

| Metric | Luna-low |
| --- | ---: |
| Anchor fidelity | 68.4% |
| State coverage | 73.3% |
| Omit cleanliness | 50.0% |
| Authority | 66.7% |
| Grounding | 86.7% |
| Metadata | 72.7% |
| Guards | 93.3% |

Luna genuinely failed to select four durable gold states:

- future ship variants;
- current custom-room-code state;
- server-authoritative collision constraint;
- deleted-assets contextual correction.

It also retained Prompt-41 progress and phase-renumbering. The result establishes a meaningful model-capability floor for the pass-1 semantic task; the two-pass decomposition alone does not make a weaker selector reliable.

## Luna medium/high reasoning follow-up — 2026-09-07

A focused follow-up tested only `gpt-5.6-luna` at `medium` and `high` reasoning on the same frozen `corpus/insomnia-tuning-v1.jsonl` fixture. Each measured run used all 11 Episodes / 314 turns, four workers, `corpus/insomnia-gold-v3-tuning-v1.json`, and `tools/evaluate_insomnia_gold.py`. The experiment used the frozen three-pass tuning harness from commit `80486f4` in detached worktrees so benchmark-only changes could not alter `main`.

Unlike the historical Sol/Luna/Sol architecture, these follow-up runs intentionally used Luna for all three model stages—semantic ledger, metadata classification, and wording—with the selected reasoning level applied consistently across the run. The intended comparison variable was therefore Luna reasoning effort: `medium` versus `high`.

The configured `codex` device credential had expired and failed before producing Episode output with HTTP 401 `token_expired`. The already-stored `codex-alt` ChatGPT/Codex credential was valid, so the experimental harness used that credential without changing the repository's active model configuration.

The measured results were:

| Metric | Luna-medium | Luna-high |
| --- | ---: | ---: |
| Memories | 63 | 70 |
| Anchor fidelity | 84.2% | 89.5% |
| State coverage | 86.7% | 93.3% |
| Omit cleanliness | 75.0% | 75.0% |
| Authority | 100.0% | 100.0% |
| Grounding | 81.8% | 96.0% |
| Metadata | 85.7% | 81.2% |
| Guards | 100.0% | 100.0% |

Both evaluator runs scoped to 139 presented user turns containing 19 anchor cases, 15 retained-state cases, and 4 omit cases.

Luna-medium failures:

- Anchor/state miss: `adopt-c-calibration-corpus`.
- Anchor/state miss: `custom-room-code-current-fact`.
- Omit failure: `phase-renumbering` was retained.
- Grounding failures: `creatureserver-mothballed`, `shipstats-now-grounded`, `collision-authority-narrow`, and `deleted-assets-contextual-correction`.
- Metadata failures: `future-ship-variants` and `deleted-assets-contextual-correction`.

Luna-high failures:

- Anchor/state miss: `adopt-c-calibration-corpus`.
- Omit failure: `phase-renumbering` was retained.
- Grounding failure: `creatureserver-mothballed`.
- Metadata failures: `workspace-repository-location`, `future-ship-variants`, and `deleted-assets-contextual-correction`.

High therefore materially improved the semantic-selection side of the Luna-only comparison: Anchor rose from `84.2%` to `89.5%`, State from `86.7%` to `93.3%`, and Grounding from `81.8%` to `96.0%`. It recovered `custom-room-code-current-fact`, but it did not improve the persistent `phase-renumbering` false positive, and metadata agreement fell from `85.7%` to `81.2%`. High also emitted seven more Memories (`70` versus `63`).

Observed end-to-end wall times were approximately `208.5 s` for medium and `485.6 s` for high. These are useful operational observations, not a clean throughput benchmark: the two runs overlapped for most of their execution and therefore competed for provider/account capacity and local resources. High was nevertheless visibly much slower in this run.

Luna again exposed a ledger-completeness weakness. A preliminary medium attempt aborted because one user turn was absent from the ledger. The harness was then given the same narrow deterministic repair used in the historical Luna-low comparison: any missing user turn is inserted as an explicit `omit` entry and contributes no semantic state. The measured high run exercised that repair once. This repair cannot improve a missed gold state unless the missing turn should itself have been retained, in which case it remains an omission by construction.

A separate resume attempt exposed a benchmark-harness defect and was discarded from the measurement. Saved ledger artifacts had already been mutated with post-group metadata; rebuilding synthesis groups from those post-classification ledgers could collapse groups that had been distinct when originally frozen, producing metadata group-count mismatches. The final medium and high measurements above were therefore fresh runs with no resume state. This is a harness-resume issue, not a model-quality result.

The historical Luna-low result below (`68.4%` Anchor / `73.3%` State / `50.0%` Omit) remains useful as evidence that low reasoning was below the pass-1 capability threshold, but it is not a strict three-way baseline for this follow-up because it was measured under the earlier v6 comparison path. The valid direct conclusion from the 2026-09-07 experiment is **Luna-high > Luna-medium for semantic selection and grounding on this frozen fixture**, at substantially higher latency, while neither reasoning level solved over-retention.

## Historical 2026-08-24 model ranking

On this 11-Episode tuning fixture and the then-current contract:

`Sol-low > Ox Alpha-low >> Luna-low`

Comparison:

| Metric | Sol-low v6 | Ox Alpha / Nous | Luna-low |
| --- | ---: | ---: | ---: |
| Memories | 54 | 62 | 48 |
| Anchor fidelity | 94.7% | 94.7% | 68.4% |
| State coverage | 100.0% | 100.0% | 73.3% |
| Omit cleanliness | 100.0% | 75.0% | 50.0% |
| Authority | 95.8% | 90.9% | 66.7% |
| Grounding | 100.0% | 86.4% | 86.7% |
| Metadata | 60.0% | 62.5% | 72.7% |
| Guards | 100.0% | 95.5% | 93.3% |

Sol-low remains the preferred Insomnia selector for the next implementation step. Ox Alpha is worth retaining as a compatibility/model benchmark, particularly because its state coverage matches Sol, but it currently needs more transport/structure normalization and loses on disposition/provenance discipline. Luna-low is below the observed pass-1 capability threshold.

## Pass-2 ownership validation and production integration

The controlled `v7` comparison reused the exact Sol-low v6 ledgers and changed only pass 2. Sixty-eight retained ledger clauses collapsed deterministically into 49 structurally compatible synthesis groups. The wording model received only required group keys and `{title, content}` output fields; provenance, classification, lifecycle, source IDs, and group membership were no longer model-editable.

The controlled result produced 49 Memories with `94.7%` anchor fidelity, `100%` state coverage, `100%` omit cleanliness, **`100%` authority**, `100%` grounding, `60%` metadata, and `100%` guards. Compared with v6, authority rose from `95.8%` to `100%` with no loss in coverage, omit cleanliness, grounding, or guards. The unchanged metadata score confirms those residual classifications belong to pass 1 rather than synthesis.

A fresh end-to-end `v7` Sol-low run produced 51 Memories and scored `89.5%` anchor fidelity, `93.3%` state coverage, `100%` omit cleanliness, `100%` authority, `100%` grounding, `71.4%` metadata, and `100%` guards. Its substantive coverage miss was `future-ship-variants`, which pass 1 stochastically omitted; no synthesis-side authority/provenance failure reappeared.

The production implementation reached by this 2026-08-24 milestone encoded this semantic/synthesis ownership boundary as extractor contract `v3-0`:

- **Pass 1 owns:** disposition, authority kind, lifecycle, source identity, assistant-authority identity, grounding identity, category, and type.
- Retained clauses are deterministically grouped only when all those structural fields match.
- **Pass 2 owns:** title/body wording for every required group and cannot add, drop, merge, split, or reclassify groups through its schema.
- Reliquary reconstructs exact source/authority/grounding provenance from selected IDs rather than asking the wording model to reproduce source bytes.
- One bounded Archive evidence round remains inside pass 1; the final ledger is resolved before synthesis begins.
- Candidate identity includes deterministic ledger semantic material so structurally distinct Memories from one source turn do not collide when exact full-turn provenance is reused.

The evidence still does not justify a general semantic review pass. The added third model call is deliberately metadata-only and is downstream of frozen semantic grouping; it does not perform state discovery, rejection, or rewriting.

## Final metadata and pass-1 tuning closure

A controlled metadata comparison reused the exact 51 semantic groups from the Sol-low v7 run. Moving `category` / `type` / `lifecycle` ownership into a dedicated Luna-low pass raised metadata agreement from `71.4%` to `93.3%` while leaving Anchor, State, Omit, authority, grounding, guards, and the 51-group semantic inventory unchanged. The sole remaining controlled metadata miss was `deleted-assets-contextual-correction`, whose upstream proposition is itself semantically ambiguous; no benchmark-specific rule was added for it.

The final pass-1 tuning then addressed the remaining stable semantic edges without adding another review stage:

- mixed question/request turns are evaluated clause-by-clause so independently asserted durable state survives the surrounding conversational purpose;
- future/possible durable clauses retain their modality instead of being discarded because the turn also asks a question;
- checkpoint labels such as prompt/phase numbers may not leak into retained propositions even when an adjacent project-status clause is durable;
- equivalent later restatements corroborate rather than automatically supersede an earlier authoritative source;
- actual replacement of any material mechanism/value/requirement supersedes the overlapping older clause as a whole rather than preserving a generalized remnant.

Targeted final-contract probes for `future-ship-variants`, `phase-renumbering`, Prompt-72 mixed status, ShipStats corroboration, genuine player-colour mechanism supersession, and workspace-repository location all reached the expected Anchor/State/Omit behavior. Two fresh full 11-Episode runs around the finalized prompt observed:

| Metric | Fresh full-run range |
| --- | ---: |
| Anchor fidelity | 94.7% |
| State coverage | 93.3–100.0% |
| Omit cleanliness | 100.0% |
| Grounding | 100.0% |
| Guards | 100.0% after checkpoint-proposition tightening |

One final full run missed `workspace-repository-location`; an immediate isolated rerun of that exact conversation reached `100%` Anchor and State. Earlier full-run misses likewise moved between cases. With only 15 evaluable retained-state cases, one stochastic miss moves State coverage by 6.7 points. The remaining semantic variation is therefore treated as model sampling variance at the current architecture/model capability boundary, not evidence for further prompt-specialization.

**Routine Insomnia semantic tuning is frozen at this point.** Further reliability gains should come from a materially different capability boundary—model fine-tuning/weight changes, stronger models, verifier/voting architecture, deterministic semantic preprocessing, or another explicitly justified structural stage—not continued fixture-specific prompt squeezing.

A full 66-Episode gold-v3 run remains useful as milestone confirmation, not as another routine tuning loop. Worker concurrency should be recalibrated for the eventual production model mix rather than inheriting the historical Luna/48-worker optimum.

## Future reliability architecture options

The remaining quality ceiling is no longer a prompt-tuning problem. If production evidence later shows that the current stochastic miss rate is operationally unacceptable, the next experiments should change the inference architecture rather than specialize the existing fixture further.

### Targeted semantic verifier / repair

Add a narrow verifier after the semantic ledger is produced. The verifier must not rediscover the Episode from scratch or rewrite every Memory. It receives the frozen source turn plus the selected ledger decision and may only flag bounded structural errors such as:

- durable clause incorrectly omitted;
- question/request/checkpoint incorrectly retained;
- genuine replacement incorrectly treated as corroboration, or corroboration incorrectly treated as supersession;
- proposition expanded beyond the user's authority.

Only flagged entries proceed to a repair call. This is materially different from the blanket candidate reviewer already tested and rejected, which reduced recall because it re-reviewed every candidate. A targeted verifier is preferable if production failures cluster in identifiable high-risk semantic boundaries.

### Multi-sample voting for ambiguous clauses

For clauses identified as ambiguous, sample the semantic selector more than once and compare `retain` / `omit` / `superseded`, authority, and proposition decisions. Unanimous or strong-majority decisions can proceed directly; disagreements can be escalated to a verifier.

This directly attacks stochastic variation instead of attempting to remove probability through prompt wording. It is also the most obvious cost multiplier, so it should be selectively routed rather than applied to every turn unless measured failure economics justify the additional inference.

### Deterministic clause-candidate preprocessing

Move some boundary detection out of the model. A deterministic or lightweight preprocessing stage could split a user turn into candidate declarative assertions, questions/requests, checkpoint spans, and other rhetorical segments before Sol performs semantic disposition.

The intended benefit is to reduce mixed-turn ambiguity such as `durable future plan + what do you think?` or `checkpoint + independent current status`. The risk is replacing model variance with brittle linguistic heuristics. Any preprocessor must therefore generate candidates rather than make final semantic decisions, and it must preserve the original source span so the selector can reject or recombine candidates when segmentation is imperfect.

### Ambiguity / confidence routing

Make Insomnia a tiered inference pipeline rather than sending every clause through identical work. The first semantic stage should emit a bounded ambiguity/risk signal or deterministic structural risk classification. Straightforward direct assertions can remain on the cheapest path. Higher-risk cases can receive additional processing, including:

- terse adoption/retention;
- correction requiring grounding;
- mixed receipt plus durable state;
- possible supersession/corroboration conflicts;
- uncertain future modality;
- provenance-sensitive deictic references.

This allows verifier/voting cost to be concentrated where the architecture already knows errors are more likely.

### Separate supersession resolver

Split state extraction from state-relationship resolution. One stage would extract durable propositions from each authoritative source independently. A later relationship stage would compare overlapping propositions and classify relationships such as `current`, `duplicate`, `corroborates`, or `supersedes`.

This reduces the semantic burden on the source selector, which currently both discovers state and reasons about whether later state invalidates it. It also approaches Dream/Graph relationship territory, so ownership must remain explicit: Insomnia may need enough local supersession logic to publish current working Memory safely, while broader cross-Memory relationship/lifecycle reasoning should not be duplicated if Dream becomes the canonical owner.

### Evidence-aware provenance verifier

Adoption and grounding are structurally different from ordinary direct authority and can receive their own narrow support check. A provenance verifier could answer only questions such as:

- does the selected assistant authority source actually contain the proposition the user adopted/retained?;
- does the grounding source merely resolve a referent, or is semantic content leaking from it?;
- is the selected grounding source necessary and temporally valid?;

This stage should never manufacture missing user authority or broaden the proposition. It exists only to validate the support relationship already selected by the semantic pass.

### Preferred escalation architecture

If production data eventually justifies additional reliability work, the most plausible next architecture is:

`deterministic clause candidates -> Sol semantic selector -> fixed semantic groups -> targeted ambiguity/verifier stage -> Luna metadata -> Sol wording`

The verifier should be invoked only for structurally ambiguous/high-risk cases. Multi-sample voting can be added inside that ambiguity path if the residual failure rate justifies its inference cost.

This is not current implementation work. The present architecture remains the baseline until production-observed failures establish both a distinct failure class and enough value to justify the added latency, cost, and complexity. Model fine-tuning/weight changes or a materially stronger selector remain alternative capability boundaries if available.

### User/project persistence-scope classification

This section records the design question that remained open on 2026-08-24. It has since been resolved by ADR 0023. Current extractor contract `v3-4` uses a dedicated narrow `user | project` ownership classifier after deterministic groups (and optional fixed-group metadata classification) but before wording. The classifier may change only the destination owner; it cannot modify retained semantics, provenance, metadata/lifecycle, grouping, candidate identity, or the later Entity-enrichment result. After wording, a separate Entity-mention pass extracts only verbatim final-Memory routing metadata and has no ownership authority. Lexical Memory routing is deterministic derived indexing over complete Memory text. Ambiguous ownership state defaults to Project.

Current User publication targets an explicitly attached Phylactery and strips REL-local source/Episode/conversation provenance so the PHY Memory remains valid independently of Project Archive retention. The REL completion records the result through `MemoryRef { owner_id, memory_id }`. With no cross-file transaction manager, PHY is synced first and deterministic mutation IDs make a crash before the REL receipt idempotently recoverable.

The historical question of folding scope into metadata was therefore rejected for the current slice in favor of a separately tunable ownership boundary. Organization routing and richer cross-file lineage remain later work; Connection/relationship routing is currently mothballed rather than an active expansion target. See [REL/PHY scope ownership](subsystems/scope-ownership.md) and [ADR 0023](decisions/0023-insomnia-durable-owner-routing.md).

## Experimental implementation

- `examples/insomnia_three_pass_sol_luna/main.rs`
- `examples/insomnia_three_pass_sol_luna/contract.rs`
- `tools/insomnia_two_pass_prompts.py`
- `tools/nous_json_client.py`
- `tools/run_insomnia_two_pass.py`
- `examples/run_two_pass_openrouter.rs`

The active Rust tuning harness uses the configured Insomnia route for Sol-low semantic selection and wording, derives a Luna-low metadata route from the same Codex credential configuration, and never saves those experimental overrides to config. The historical Python two-pass prompt module imports the current Rust ledger/synthesis contract text so Ox/Nous compatibility runs continue to share the same semantic rules. `target/insomnia-*` outputs are disposable local benchmark artifacts and are not repository inputs.

## Related docs

- [Development](development.md)
- [Roadmap](roadmap.md)
- [Architecture](architecture.md)
- [Current limitations](current-limitations.md)
- [ADR 0012 — deterministic Episodes and Insomnia Memory authority](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)

## Notes

These measurements are specific to the 11-Episode adversarial tuning fixture and current provider behavior. They establish a practical tuning ceiling and protect the ownership boundaries under test; they are not population-level model accuracy. The tuning fixture should now be treated as a regression suite rather than an optimization target. Full 66-Episode gold-v3 confirmation remains a future milestone for authoritative runtime validation.
