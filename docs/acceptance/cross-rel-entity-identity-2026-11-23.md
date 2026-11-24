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

### Broader same-model comparison

The trial now has 34 authored evidence examples. Four are the original pilot. Six reuse the evaluator and public integration fixtures. The other 24 were written after the prompts and validators were fixed and before their first live run. The new set contains eight same, eight distinct and eight unresolved pairs. It includes people, places, events, organizations, artworks, vessels, applications, libraries, devices and brands. A separate Sol 6.2 review, given both witnesses and neither the labels nor rationales, agreed with all 24 case labels before the paired run. These are synthetic, model-reviewed labels. [Held-out evidence and rationales](../../crates/reliquary-entity-resolution/src/identity_trial/pilot/held_out.json)

Both arms received one grounded quote from each simulated REL, with separate source lineages, the endpoint names and kinds, and a named participant for each quote. The harness represents each quote with the same generic Relationship and role structure. The one-call evaluator ran first on each pair. The staged evaluator used the same `gpt-5.6-sol` endpoint at low effort for interpretation, decision and conditional support selection. Neither arm received the case ID, expected result or rationale.

Both arms scored 6/6 on the reused fixtures. These include a shared occupation, two different contracts, an attendee versus a signatory, and two sites with the same name. The briefing and later signature are two states of one pair. The later example isolates the newly added witness. The public integration test retains both peer witnesses and verifies the stored decision changes. The bilingual summit repeats the original marker pattern. Only the 24 fresh pairs enter the held-out scores. [Regression examples](../../crates/reliquary-entity-resolution/src/identity_trial/pilot/regression.json) · [Source evaluator tests](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs) · [Public history fixture](../../tests/entity_identity.rs)

| Expected outcome on fresh pairs | One Sol call, correct | Staged Sol, correct |
| --- | ---: | ---: |
| Same Entity | 5/8 | 7/8 |
| Distinct Entities | 7/8 | 8/8 |
| Unresolved | 8/8 | 8/8 |
| **All first attempts** | **20/24** | **23/24** |
| Model calls attempted | 24 | 62 |

Each arm produced 23 completed judgments and one rejected response. Both kept all eight uncertain pairs unresolved. The raw one-call verdict matched the expected outcome on all 24. Four settled results diverged from that raw verdict.

- On `acorn`, both sources identify the same software update from Tin Ledger to Tallies at different institutions. On `dogwood`, both inspectors identify the meter by its mounting point and a distinctive gouge. On `heath`, two surveys identify a yard through the same service arch. Sol proposed the correct match on each. The one-call validator requires matching normalized excerpt words and a shared named participant. The selected excerpts differed on all three, and the rename also lacked a shared non-endpoint participant. All three settled as unresolved. Staged Sol accepted the rename and meter.
- On `heath`, the staged interpretation inserted bracketed words into the survey excerpts. The exact-source check stopped before a verdict call. On `briar`, one-call Sol correctly distinguished two same-named C libraries but cited each witness twice. Its validator rejected the response. Staged Sol identified the separate libraries with one citation from each side. All model requests returned responses.

A diagnostic repeat of the two cases with rejected responses used the same prompts and evidence. Both evaluators correctly separated the libraries on the repeat. The yard remained unresolved in the one-call arm and again failed the staged interpretation check. The table retains the original first attempts.

This is one paired measurement on a balanced, authored set. Real REL pair frequencies, multiple-witness attribution, and the Jev decision route are outside this score. Results compare two complete evaluators, including their different citation requirements. Production uses the one-call evaluator.

### One-call comparison policy 3

Policy 3 keeps the existing Entity route. One model request judges the evidence and supplies its citations. Core checks every selected excerpt against its own source, verifies the indexed witnesses and their independent lineages, and requires compatible Entity kinds for a match. Two contextual excerpts can corroborate an identifying fact using different wording, Relationship kinds, roles, or named participants. The model judges the meaning of those excerpts. A compact named participant reference can still be supported by the recorded Relationships. Duplicate citations are invalid and prevent publication.

Replaying the saved first one-call responses from the 24-pair comparison isolates the validator change. The same 23 well-formed responses now all settle to their labels, including the yard, renamed application and meter. The two same-named libraries still produce an invalid response because the saved answer repeats both witness positions. The public upgrade test seeds an earlier policy 2 receipt and observes a policy 3 successor from ordinary authorized use with its predecessor still inspectable. [Replay fixture](../../crates/reliquary-entity-resolution/src/identity_trial/pilot/held_out_one_call_replies.json) · [Public receipt test](../../tests/entity_identity.rs)

The first live check after loosening the wording rule recovered the three paraphrased matches and produced a false match on `thorn` (23/24 on the comparison cases, 6/6 on regressions). The artwork descriptions shared a venue and appearance, while one witness could not identify the artist or exhibition. The final prompt asks Sol to consider whether separate referents reasonably fit both complete accounts and to preserve each source's qualifications.

| Final one-call prompt, first attempts | Correct settled outcomes | Rejected responses |
| --- | ---: | ---: |
| Prior 24 comparison cases | 24/24 | 0 |
| Six reused regression examples | 5/6 | 0 |
| Four new pre-labeled cases | 4/4 | 0 |

Each pair used one signed-in `gpt-5.6-sol` call at low effort. The final prompt kept the artworks unresolved. Its miss was the corroborated contract signatory. Sol itself returned unresolved with an empty citation list, so citation validation did not change its verdict. The new cases covered a rescue described from two perspectives, two pieces of equipment operating or disabled at different locations, two plausible market sellers, and conflicting reports about a choir. The prior 24 were already known during this change. The four new pairs were labeled before they were evaluated. [New cases and rationales](../../crates/reliquary-entity-resolution/src/identity_trial/pilot/corroboration.json)

Repeating the artwork and signatory examples reproduced both final outcomes. The six-case regression fixture presents each witness as `recorded_in` with generic roles. A separate two-call control supplied the public fixture's actual Relationship kinds and roles and retained the earlier briefing. With A as signatory and B as attendee, Sol returned unresolved. After B also gained a signature, Sol returned same and cited A's signature (Relationship 0) and B's new signature (Relationship 1), with exact excerpts on both sides. These two observations are raw model responses. The public integration test independently exercises Core validation, persistence and reopening with a scripted endpoint. [Public evidence-change fixture](../../tests/entity_identity.rs)

### Real REL inventory

The 128-Memory Insomnia calibration set contains 313 labeled Entity mentions. Its annotations identify mention spans. Scoring a cross-REL comparison also requires an expected pair judgment and independent grounded witnesses.

A read-only Core scan paged through all current Entities in the migrated 28-day and Ellis RELs, including aliases. The 28-day owner contained 524 Entities and Ellis contained 88. After folding case and whitespace, the two indexes shared zero names or aliases. Different names can still identify the same referent. More importantly, each REL had **zero current Relationships**. The overlapping 14-day REL also returned an empty Relationship page with no continuation. Under policy 3 these snapshots supplied no input for the live evaluator, which required Relationships. Only authored evidence went to the model endpoints during that trial. The two scanned files matched their authoritative SHA-256 manifests after inspection. [Pair evidence contract](../planning/cross-rel-entity-identity-spec-2026-11-23.md) · [Migration evidence](./owner-history-migration-other-pairs-2026-11-19.md)

### Reproduce

Run the default suite with `cargo test -p reliquary-entity-resolution --locked --offline --lib`. To reproduce the scripted false positive run the same command with `observe_version_only_false_affirmative -- --ignored --nocapture`.

For an intentional live run, set `RELIQUARY_IDENTITY_TRIAL_CONFIG` to the existing protected config at its registered path and run `cargo test -p reliquary-entity-resolution --locked --offline --lib configured_models_on_authored_pairs -- --ignored --nocapture`. Cargo's offline flag applies to dependency resolution. This test contacts the selected models and fails its gate on a missed staged label or a provider failure. If using an existing signed-in Codex client for the configured Sol model, set `RELIQUARY_TRIAL_CODEX` to its native executable and `RELIQUARY_TRIAL_SCRATCH` to an existing local scratch directory. Jev continues to use Switchboard. To exercise only the real selector on fixed proposals, run the same ignored test command with the filter `configured_support_on_fixed_proposals`. [Proposed implementation and release gates](../planning/cross-rel-entity-staged-evaluation-2026-11-23.md)

For the same-model control, use the same profile and signed-in client settings and run `cargo test -p reliquary-entity-resolution --locked --offline --lib same_model_on_authored_pairs -- --ignored --nocapture`. This test binds Switchboard's general decision adapter to the exact general endpoint used by the baseline and interpretation. It produces plain verdict and support choices. Each call runs in a fresh Codex session at low reasoning effort.

For the expanded same-model comparison, run the same Cargo command with `same_model_on_held_out_pairs` or `same_model_on_regression_pairs`. Each filter runs its whole set by default. To repeat specific cases, set `RELIQUARY_TRIAL_CASES` to their comma-separated IDs. The harness prints every outcome and both scores before checking completeness and agreement with the labels. Either a missing judgment or a wrong settled outcome fails the labeled trial. [Corpus runner](../../crates/reliquary-entity-resolution/src/identity_trial/pilot/corpus.rs)

To run policy 3 alone on the 24 comparison cases, six reused examples and four new cases, use the same configured pilot settings and the filter `one_call_on_synthetic_pairs`. Set `RELIQUARY_TRIAL_CASES` to select a subset. The default offline package suite replays the saved responses without contacting any model.

## Operator confirmation and undo

At Core `0556fc9d`, all nine public Entity identity tests passed, including three cases for operator review. A confirmation kept the pair separate after the worker returned `SameIdentity`. A second designated operator withdrew the latest confirmation. After reopening, inspection returned the evaluator’s answer and preserved both operators’ IDs. A further withdrawal with a stale evaluator receipt returned `Pending`. The tests also verified that a superseded confirmation could not be withdrawn and that a replacement REL reusing the same Entity ID did not inherit the decision. [Public review tests](../../tests/entity_identity/separation.rs)

The Gateway regression covered an expired preview, a retry after a successful commit, and reuse of an action ID with different inputs. Owner-session tests covered reopening the recording REL without its peer, incomplete-tail recovery, checkpoints and verified copies. Hermes operator interaction remains a separate integration. [Gateway regression](../../crates/reliquary-runtime-gateway/src/instance_identity/separation/tests.rs) · [Event recovery](../../crates/reliquary-owner-session/src/session/identity_separation/tests.rs) · [Checkpoint and copy tests](../../crates/reliquary-owner-session/src/session/checkpoint/tests_identity_separation.rs)

## Direct Memory evidence (policy 4)

The public regression starts with two Entities named Alex Chen. Their only resolved mentions contain the name. Core records `Unresolved` with zero model calls. Two independent Memories then identify Alex through the registration of the same vessel. On reopening, Core revisits the pair. One scripted Entity call cites the two source passages and Core records `SameIdentity`, with zero Relationships in either REL. Withdrawing one mention returns the pair to `Unresolved`. The earlier receipt still returns both quotes. A separate mixed-evidence case combines a mention on one side with a Relationship on the other and checks that citing just the name cannot support a match. [Public regression](../../tests/entity_identity/mentions.rs) · [Evaluator tests](../../crates/reliquary-entity-resolution/src/identity_evaluator_tests.rs)

The migrated 14-day and 28-day files share an owner ID. The trials therefore compare each ChatGPT snapshot separately with Ellis. All three RELs have zero current Relationships. Inventory on disposable copies found the following source material.

| REL | Resolved links at the head of typed migration history | Eligible directly sourced mention spans | Entities with those spans |
| --- | ---: | ---: | ---: |
| 14-day | 0 | 0 | 0 |
| 28-day | 2,523 | 1,467 | 419 |
| Ellis | 1,267 | 731 | 82 |

The 14-day file retains 169 opaque resolution records. The 28-day and Ellis files retain native typed decisions with a target Entity and a position in a specific Memory. The eligibility count requires an active source Memory, a direct source reference, and a valid span in its retained body.

On untouched evidence in the 14-day and Ellis copies, Core recorded `Unresolved / InsufficientEvidence` with zero calls. On the 28-day and Ellis copies, the configured Entity model, `gpt-5.6-sol`, received one 28-day excerpt and three Ellis excerpts. All four matched their exact content spans. Sol proposed separate Entities. Core validated one exact citation on each side. At least one failed the check for evidence that positively distinguishes its referent. The trial kept the outcome and citation count, so it cannot identify which citation or condition failed. The recorded result was `Unresolved / Ambiguity`. Fresh Gateway instances reopened both comparisons. The two cited Memory passages were recovered verbatim, and a retry reused the receipt. All three original RELs and the read-only Ellis copy kept their hashes. This real pair has no independent reference judgment. [Copy trial](../../tests/entity_identity/mention_corpus.rs)

To reproduce the live trial, set `RELIQUARY_MENTION_CORPUS_DIR` to the authoritative fixture root, `RELIQUARY_IDENTITY_TRIAL_CONFIG` to the protected Sol profile at its registered path, and `RELIQUARY_TRIAL_CODEX` to the signed-in native executable. Run `cargo test --locked --offline --test entity_identity authoritative_mention_corpus_compares_and_reopens -- --ignored --nocapture --test-threads=1`. This opt-in test sends the selected excerpts to the configured model via the signed-in client. The ordinary public test suite uses authored data.

## Automatic pair discovery

A managed REL and a read-only peer were mounted under one credential. Without a caller supplying Entity IDs, Core found two pairs from names or aliases and recorded both as unresolved with zero model calls. After a new Memory placed the same hull number beside a differently named vessel, Core found a third pair. One scripted Entity call returned a match with separate citations from the two Memories. The public API listed all three pairs by peer and paged them by ID. A third mounted REL outside the credential's grants produced no receipt. [Discovery cases](../../tests/entity_identity/discovery.rs)

An offline run on temporary copies of the original 28-day and Ellis project RELs enumerated all eligible Entity rows and found **zero** pairs sharing a qualifying key. The authenticated workspace returned zero pairs, the reopened recording copy held zero receipts and the local endpoint received zero requests. Both source files and the read-only Ellis copy retained their original hashes. This measures coverage of the current discovery rules on this corpus. It leaves open whether differently described Entities refer to the same thing.

In a separate positive control, one new Entity on the disposable 28-day copy was given the name of an existing Ellis Entity as an alias. The public list showed one pair 0.91 seconds after reopening finished. With no witnesses on the new Entity, Core recorded `Unresolved / InsufficientEvidence` locally. The receipt survived a fresh reopen. Both originals and the read-only peer still matched their initial hashes.

To repeat the copy run, set `RELIQUARY_MENTION_CORPUS_DIR` to the authoritative fixture directory and run `cargo test --locked --offline -j 1 --test entity_identity discovery::authoritative_copies_discover_without_presupplied_entity_ids -- --ignored --exact --nocapture`. The earlier configured-model trial above supplied its own pair.

## Related docs

- [Public comparison and inspection API](../public-product-api.md)
- [Comparison contract](../planning/cross-rel-entity-identity-spec-2026-11-23.md)
- [Implementation plan and acceptance cases](../planning/cross-rel-entity-identity-plan-2026-11-23.md)

## Notes

The copy trial establishes real-source lookup, citation validation and persistence. Measuring decision accuracy on retained data still requires independently labeled cross-REL pairs.
