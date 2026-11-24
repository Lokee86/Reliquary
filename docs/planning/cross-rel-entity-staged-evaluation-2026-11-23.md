# Staged evaluation of cross-REL Entity identity

Parent index: [Planning](./INDEX.md)

## Purpose

Give each model request one job. Core currently asks one Entity model to interpret quotes, reach a verdict and justify it in the same response. The proposed evaluator first interprets the sources, then asks a decision model for a provisional verdict, then asks it to select the evidence supporting that verdict. An unsupported affirmative proposal must leave the pair unresolved. The staged design was evaluated as a test-only alternative. Policy 3 retains one call in production. [Decision and verification](../acceptance/cross-rel-entity-identity-2026-11-23.md#one-call-comparison-policy-3) [Current evaluator](../../crates/reliquary-entity-resolution/src/identity_evaluator.rs) · [Trial evidence](../acceptance/cross-rel-entity-identity-2026-11-23.md#three-call-trial)

## Overview

The evaluator produces a proposed semantic judgment. Gateway retains authority over the peer read, receipt revision and publication. The trial and live evaluator share the existing evidence capture and have separate test entry points.

## Evaluation contract

The input is one explicit, directional pair of owner-qualified Entities and the bounded evidence already captured under the recording REL's management grant and the peer's read grant. Keep the existing change fingerprint, grounded source spans, lineage checks, evidence limits and deterministic zero-call outcomes. Repeating a current receipt under the same evaluation policy and bound routes reuses the receipt. The same procedure applies to people, software, places and other Entities. [Current pair contract](cross-rel-entity-identity-spec-2026-11-23.md)

| Step | Switchboard route | Input and required response |
| --- | --- | --- |
| Interpret | `entity-resolution` | Pass both sides' bounded evidence. Return one interpretation per grounded witness, tied to its side, Relationship and Memory, with an exact excerpt. Preserve qualifications such as dates and source disagreements. Core checks complete coverage, valid indices and the quoted text. |
| Decide | `entity-resolution-decision` | Supply the original evidence and the verified items, including source independence information. Ask one choice question. Accept only same Entity, distinct Entities, or unresolved. The response contains no citation requirement. |
| Select support | Same bound decision route | For an affirmative verdict, send the fixed verdict and original evidence in one batched request. Ask separately whether each indexed item materially supports that specific conclusion. Accept each declared item exactly once. Only support selections are eligible. A direct decision endpoint can supply a finite 0–1 score for each selected choice. Use those scores to rank eligible items. With plain choices use stable citation order. |
| Settle | Core | Publish one final receipt. An affirmative judgment requires material distinguishing evidence on both sides from independent sources, with validated citations. Valid empty, one-sided or dependent support settles as unresolved. Core supplies original quote references. A selection cannot replace “same” with “distinct”, trigger another verdict call, or authorize a user-confirmed distinction. |

A locally conclusive lack of independent grounding takes zero calls. A model verdict of unresolved takes two and needs no proof of absence. A normally completed affirmative proposal takes three, including one selector call regardless of the number of items. Scores express support for the fixed verdict per item. Two independent sources can both score highly. Choosing the highest score must never manufacture support out of an empty selection.

## Pending work and publication

An unavailable route, malformed reply, missing answer, unknown witness or altered excerpt leaves the attempt incomplete. A structurally valid answer declining all support is a completed judgment of insufficient support. Retry a failed selector with the frozen verdict and identical evidence, without repeating interpretation or deciding again. If the evidence, policy, bound route, predecessor, mounts or grants changed, discard that continuation and use the ordinary authorized preparation path. An older receipt keeps its history and normal freshness state throughout.

For the initial integration, hold the selector continuation in the live Gateway alongside the authorized pair's scheduling state. Store only the final receipt in the REL. A process restart discards in-flight model work and recomputes from authorized evidence on the next attempt. An initial comparison interrupted before its first receipt can be reissued explicitly. Recorded pairs resume under normal admission. This keeps peer quotations, model output and credential material out of persistent work records. Before publication, recheck both owners and current evidence using the existing publication fence. [Gateway compare](../../crates/reliquary-runtime-gateway/src/instance_identity.rs) · [Normal reconsideration](../../crates/reliquary-runtime-gateway/src/instance_identity/worker.rs)

## Implementation order and cutover gates

1. Exercise both evaluators on the same authored, labeled inputs outside the runtime. Run the protocol with scripted replies to force unsupported judgments, independent versus shared sources, malformed replies and a failed third call. Record actual route/model names, outcomes, selected source items and provider errors separately in the model-backed trial.
2. Resolve any false affirmative on ordinary shared attributes, version changes, contradicted accounts or copied sources before replacing the evaluator. Exercise both directions of error: a false match and a false separation. Review the actual quotes for correct positive outcomes. Include differently worded independent corroboration. Before promotion, enforce the configured input budget on the expanded per-stage payloads, including interpretations and the batched questions. Preserve the established evidence-limit outcome when the full input cannot fit. Four authored cases are a smoke test. A claim of improved accuracy requires additional labeled, held-out pairs evaluated by both policies.
3. In Switchboard bind the interpretation and effective decision route together. Include the chosen decision provider/model/endpoint and the interpretation route in the nonsecret receipt route identity. Detect dedicated versus adapted General and preserve in-flight route replacement fencing. At the Entity evaluation seam promote the approved stage contracts and validation. Application must distinguish final judgment from retryable continuation. Gateway owns attempt lifetime, coalescing and stage-three-only retry. Keep model calls outside owner locks and preserve inspect-only reads.
4. Bump the automatic comparison policy and replace the old evaluator in one integration change. Keep existing 2.1 receipt records readable with their original policies, revisions and citations. Reconsider recorded pairs on ordinary authorized use and append a successor on the current chain. Keep the single Root comparison interface, independent peer authorization and owner file format.
5. Before release run the public cold-reopen fixture with a read-only peer, the paused-revocation/route-replacement publication case, a failed-selector retry with invocation counts, and receipt-history inspection. Run the locked offline workspace checks. The old one-call implementation remains the production path until the pilot's false positives and the integration gates are resolved.

## Historical trial follow-up

The [same-model control](../acceptance/cross-rel-entity-identity-2026-11-23.md#same-model-control) tied at 4/4 in both runs, with more calls for the staged evaluator. Freeze the original quotes and Sol interpretations for the missed summit and software pairs. Vary only how Jev receives the verdict question, so its ability to distinguish the three outcomes can be measured separately from interpretation. For support selection, keep the provisional verdict fixed and change one source from a genuinely distinguishing fact to an incidental attribute. Check whether that specific item's rank and selection change. Repeat the paired test on fresh examples before adopting a request shape or a minimum score. Preserve valid empty support as unresolved and malformed or unavailable support as pending.

The [broader Sol comparison](../acceptance/cross-rel-entity-identity-2026-11-23.md#broader-same-model-comparison) now supplies 24 fresh labeled pairs and six reused regression examples. Keep these outcomes as a regression baseline if prompts or validators change. The first measured gap to isolate is a correct match expressed by differently worded sources, including an explicitly witnessed rename with no shared third participant. Any change to corroboration still needs to leave the ambiguous cases unresolved. A new estimate after tuning requires fresh labeled pairs.

## Related docs

- [Original identity contract](cross-rel-entity-identity-spec-2026-11-23.md)
- [Trial observations and reproduction](../acceptance/cross-rel-entity-identity-2026-11-23.md#three-call-trial)

## Notes

The prototype and its fixtures compile only for Entity-resolution tests. They do not register a runtime worker, modify a REL, or switch production routing. The protected configuration and source corpus are outside the fixture. See the [trial implementation](../../crates/reliquary-entity-resolution/src/identity_trial.rs), [protocol tests](../../crates/reliquary-entity-resolution/src/identity_trial/tests.rs) and [configured A/B test](../../crates/reliquary-entity-resolution/src/identity_trial/pilot.rs).
