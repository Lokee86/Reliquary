# First cross-REL Entity identity comparison

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Verify that Core can leave two owner-qualified Entities unresolved, reconsider them after a relevant change and retain the reasons for its earlier decisions.

## Overview

The initial acceptance run used Core `fbfbe353` and comparison policy 1. The policy 2 results below cover the follow-up correction. The public fixture creates two native REL 2.1 files. Both contain an Entity named Alex Chen with the same local ID bytes. The two sources were authored independently. The test drives the configured Entity route with a scripted endpoint and uses the real owner writers, grants, worker and reopen path. [Public fixture](../../tests/entity_identity.rs)

## Observed decision history

| Revision | Change to the evidence | Decision and observed calls |
| --- | --- | --- |
| 1 | A says Alex signed the Meridian contract. B says Alex attended a briefing about it. | `Unresolved / Ambiguity`. Repeating the comparison returned revision 1. One endpoint call in total. |
| 2 | B adds an independent source identifying Alex as the signatory. | The running worker recorded `SameIdentity`. Both citations returned the original sentence. An unrelated Memory and an explicit retry kept revision 2. Two calls in total. |
| 3 | With the worker stopped, A corrects its source to name Morgan Lee as signatory. | Reopening A showed revision 2 as `Pending`. Restarting the worker recorded `Unresolved` without another endpoint call. Inspecting revision 2 returned its historical verdict and both retained quotes. |

## Authorization and publication

The public tests passed with a writable recording REL and a read-only peer. The peer file stayed byte-identical. A caller with both read grants could inspect the receipt. Writing with view-only access and inspecting without the peer grant both returned authorization errors.

Pausing the endpoint allowed a peer source correction and a credential revocation to be tested before publication. After each case a fresh authorized reader reopened both files and found no receipt. An unrelated peer write during evaluation preserved the judgment and produced one receipt from one call. Two equivalent comparisons evaluated concurrently and both returned the same first revision. [Public race fixtures](../../tests/entity_identity.rs)

## Initial evaluator checks (policy 1)

The 15 Entity resolution tests passed. They covered invalid citations, repeated provenance, input overflow, generic roles and two different contracts that the scripted model tried to use to distinguish the people. Policy 1 used a special case for conflicting birthplaces. Policy 2 retires that behavior. [Evaluator tests](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs)

## Storage and recovery

The owner-session checks restored receipts by cold replay, exact checkpoint and full replay of a receipt-bearing suffix. On Windows, releasing the previous checkpoint's file handle before replacing it allowed the refreshed checkpoint to cover the successor receipt. Reopening the old prefix, copying it forward, pruning descriptions and reclaiming storage preserved the expected chains. An interrupted write recovered the complete prefix. Replay rejected a bad predecessor and an impossible local revision. Unsupported rewrites refused before creating a destination. [Receipt tests](../../crates/reliquary-owner-session/src/session/identity_receipt/tests.rs) · [Checkpoint test](../../crates/reliquary-owner-session/src/session/checkpoint/tests.rs)

## Integration checks

The locked offline workspace build passed. The full test run passed 737 tests. The two existing opt-in tests for the Archive vector corpus and offline migration candidates were skipped by default. Formatting, the 23-package dependency check and the documentation checks passed.

## Follow-up verification for comparison policy 2

At Core `ec6a86f5` the evaluator applies the same evidence criteria to all Entity kinds. Its structured response distinguishes an ordinary description, corroboration of identity, evidence of separate referents and conflicting accounts. Core verifies exact quotes, indexed citations and independent source lineage. Every automatic result remains open to reconsideration.

| Fixture | Observation |
| --- | --- |
| Two programs named Relay, public Core API | Version 1.4 versus 2.0 yielded `Unresolved`. Independently authored inventories then identified one record as the editor and the other as the backup program. Comparing again appended `RelatedDistinct` with the two exact source quotes. |
| Two places named Eagle Point, evaluator | One source identifying a separate site was insufficient. With one source identifying the northern ridge and the other identifying the southern shore, and both documenting two sites, the evaluator accepted a distinction. In a second case, two names for a summit matched when independent sources placed it at the same distinctive survey marker. |
| Conflicting accounts about Alex Chen, evaluator | Different reported birthplaces supplied conflicting accounts. The pair stayed unresolved even when the scripted endpoint proposed a distinction. |
| Stored policy 1 receipt, public Core API | Inspection reported pending. On unchanged evidence and the same bound route, processing appended a policy 2 successor and retained the old receipt as history. |

These checks use a scripted endpoint and exercise Core's judgment validation and persistence. The locked offline workspace build passed, with 740 tests passing and the two opt-in corpus tests skipped. Formatting, the package dependency check and the documentation checks passed. [Public fixtures](../../tests/entity_identity.rs) · [Evaluator fixtures](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs)

## Automatic reconsideration during normal operation

A fresh Gateway reopened both RELs with the peer mounted read-only. The initial receipt had been unresolved with only one side grounded and zero model calls. After the peer gained an independent witness, Core used the session authorized for both owners and the configured Entity route to append revision 2. The scripted endpoint received one call. Revision 1 remained available as history. A separate fixture upgraded an old-policy receipt using only an authorized session, with no runtime configured. Replacing an inference route during a paused comparison discarded its late response and left the REL without a receipt for that pair. [Public fixtures](../../tests/entity_identity.rs)

## Three-call trial

**Cutover gate failed.** Both live runs used the same four authored pairs through the current evaluator and the staged prototype. The declared labels were withheld from the models. Sol 5.6 (`gpt-5.6-sol`, low reasoning) handled the current comparison and the proposed interpretation pass. The proposed verdict and support routes used `typesafe/jev-1.13` through the configured OpenRouter decision endpoint. Sol used the signed-in Codex client with the actual evaluator's structured output schema. The archived profile selected the intended models, but its Codex login returned HTTP 401. No source RELs, PHYs, release binaries or installed model settings participated. [Paired pilot](../../crates/reliquary-entity-resolution/src/identity_trial/pilot.rs)

| Authored case | Expected | Current, runs 1 / 2 | Staged, runs 1 / 2 |
| --- | --- | --- | --- |
| Independent surveys locate the summit at the same named monument | Same | Same / Same | Unresolved / Unresolved |
| Two inventories distinguish the namesake editor and backup utility installed side by side | Distinct | Distinct / Distinct | Unresolved / Unresolved |
| Version 1.4 in March and 2.0 in September, without a distinguishing witness | Unresolved | Same / Unresolved | Unresolved / Unresolved |
| Conflicting birthplace records for Alex Chen | Unresolved | Unresolved / Unresolved | Unresolved / Unresolved |

The current evaluator matched 3/4 and 4/4 labels. The staged evaluator matched 2/4 in both runs. In the instrumented run Jev selected unresolved at the verdict step on all four, with returned choice scores of 0.90, 0.93, 0.99 and 1.00 respectively. The selector therefore received no calls in that run. The scores include two confident misses. These four examples diagnose behavior on this input and prompt. They cannot establish a population accuracy rate.

The nine deterministic protocol tests cover exact call order, complete grounded interpretations, optional scores and ties, insufficient support on one or both sides, common provenance, uncertain verdicts, malformed replies and retrying a failed selector with the same evidence and verdict. A separate forced-error experiment gave both evaluators version-only evidence. With an affirmative proposal and two deliberately mistaken support selections, the staged path accepted the match. The current guard rejected the scripted proposal. Valid references on both sides establish where the selected text came from. They do not establish that it distinguishes the Entities. [Protocol tests and forced-error experiment](../../crates/reliquary-entity-resolution/src/identity_trial/tests.rs)

The fixed-proposal diagnostic exercised the selector directly because the instrumented end-to-end run stopped after every verdict. It supplied the full original synthetic quotes and a predetermined provisional choice. Only the attribution call went to Jev. These are independent yes scores for the left and right evidence items.

| Fixed proposal | Support scores, left / right | Observed settlement |
| --- | --- | --- |
| Same summit, same monument | 0.35 / 0.73 | Jev selected only the right source. The bilateral gate left the true match unresolved. |
| Distinct editor and backup utility | 0.75 / 0.64 | Jev selected both. The gate accepted the correct distinction. |
| Distinct programs from version numbers alone, deliberately wrong | 0.58 / 0.59 | Jev selected both. The gate accepted a false distinction. |

The selector produced the expected final outcome in 1/3 conditional cases. This isolates two separate problems: the verdict prompt misses clear positive examples, and affirmative source selections can still endorse irrelevant evidence. Before cutover, address both and repeat on additional held-out pairs. Tuning a score threshold on these three examples would not establish a general solution. [Selector diagnostic](../../crates/reliquary-entity-resolution/src/identity_trial/pilot/fixed.rs)

### Same-model control

Two additional paired runs used `gpt-5.6-sol` at low reasoning effort for every call. Both arms received the same four original evidence fixtures. The one-call arm ran first on each pair. The staged arm used Switchboard's general decision adapter for the verdict and support selection, sharing the signed-in Sol endpoint with the baseline and interpretation pass. Prompts, policy checks and labels were held fixed across runs. An initial transport check showed the Codex client rejected a decision schema before returning a verdict. The pilot client now expresses the same single allowed tag as a typed string enum. The results below cover the two complete runs after that correction.

| Evaluator | Run 1 | Run 2 | Model calls across both runs | Mean elapsed per pair |
| --- | --- | --- | --- | --- |
| One Sol call | 4/4 | 4/4 | 8 | 6.0 s |
| Staged Sol | 4/4 | 4/4 | 20 | 13.9 s |

Both runs identified the same summit and the two separate programs, and left the version-only and conflicting birthplace pairs unresolved. For both affirmative outcomes the staged selector chose one supporting source from each REL. For unresolved verdicts the staged path ended after the second call. On the conflicting birthplace example the one-call evaluator cited both conflicting records in both runs, while the staged evaluator returned an empty citation list. Measured time includes a fresh Codex client invocation per call.

Using Sol throughout recovered the two affirmative examples that the Jev verdict step missed. The single and staged Sol evaluators tied on these four distinct inputs. There was no evidence of improved label accuracy from splitting the calls in this control. A broader comparison would require held-out pairs and would still compare the validators as well as the prompts. Production continues to use the one-call evaluator. [Control harness](../../crates/reliquary-entity-resolution/src/identity_trial/pilot.rs)

### Reproduce

Run the default suite with `cargo test -p reliquary-entity-resolution --locked --offline --lib`. To reproduce the scripted false positive run the same command with `observe_version_only_false_affirmative -- --ignored --nocapture`.

For an intentional live run, set `RELIQUARY_IDENTITY_TRIAL_CONFIG` to the existing protected config at its registered path and run `cargo test -p reliquary-entity-resolution --locked --offline --lib configured_models_on_authored_pairs -- --ignored --nocapture`. Cargo's offline flag applies to dependency resolution. This test contacts the selected models and fails its gate on a missed staged label or a provider failure. If using an existing signed-in Codex client for the configured Sol model, set `RELIQUARY_TRIAL_CODEX` to its native executable and `RELIQUARY_TRIAL_SCRATCH` to an existing local scratch directory. Jev continues to use Switchboard. To exercise only the real selector on fixed proposals, run the same ignored test command with the filter `configured_support_on_fixed_proposals`. [Proposed implementation and release gates](../planning/cross-rel-entity-staged-evaluation-2026-11-23.md)

For the same-model control, use the same profile and signed-in client settings and run `cargo test -p reliquary-entity-resolution --locked --offline --lib same_model_on_authored_pairs -- --ignored --nocapture`. This test binds Switchboard's general decision adapter to the exact general endpoint used by the baseline and interpretation. It produces plain verdict and support choices. Each call runs in a fresh Codex session at low reasoning effort.

## Related docs

- [Public comparison and inspection API](../public-product-api.md)
- [Comparison contract](../planning/cross-rel-entity-identity-spec-2026-11-23.md)
- [Implementation plan and acceptance cases](../planning/cross-rel-entity-identity-plan-2026-11-23.md)

## Notes

These fixtures establish Core's decision, authorization and persistence behavior. Accuracy on a retained corpus with a live model is a separate milestone.
