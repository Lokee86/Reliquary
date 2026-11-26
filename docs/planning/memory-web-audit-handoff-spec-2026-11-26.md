# Memory Web audit handoff

Parent index: [Planning](./INDEX.md)

## Purpose

The shared audit chooses what Ego and Perception should investigate. The first audit contract also wrote an Ego point and assigned status and time for each passage. It used Perception's full claim type at discovery, then Synthesize read the same sources to decide those fields again. The handoff gives each consumer a suggestion and the sources it needs for its own evidence analysis. [Ego contract](../../crates/reliquary-ego/src/lib.rs) · [Perception contract](../../crates/reliquary-perception/src/proposal.rs)

## Overview

Keep one structured discovery call for the shared REL audit, with separate Ego and Perception lists and the existing optional evidence lookup. Application supplies the authorized source passages and resolved Entity slots. The model suggests topics or Observations and selects request-local source slots. The Ego and Perception contracts use the auditor's slot table to return exact references to Application. Each list keeps its independent acceptance and coverage, including the existing rejection of invalid items.

| Consumer | Discovery item | Next stage |
| --- | --- | --- |
| Ego | Suggested topic (`subject`) and one to six Memory slots (`source_slots`). | Summary Extraction reads the selected Memories and determines the details and qualifiers for the summary. |
| Perception | Proposed Observation (`proposition`), one to eight source slots (`source_slots`), and an empty `participant_slots` list for ordinary Observations. A Relationship uses two to four distinct resolved Entity slots in that list. | Synthesize reads the selected sources and decides the exact wording, status, time scope, participant roles and final citations. It may abstain. |

## Source binding and eligibility

- Use the existing `SourceCitation` for the bound references. It identifies the owner, history generation, Memory or Observation revision, body and selected passage. The consumer can reopen the exact source through its authorized read path. Deduplicate repeated Ego references while keeping distinct selected passages, and retain the existing Perception check for distinct citations. A proposal requires at least one eligible source. [Shared slot binding](../../crates/reliquary-memory-audit/src/lib.rs)
- The REL Ego list uses Fresh Memory sources. Keep the runtime recheck of current revision, body, passage availability and Freshness before handing references forward. Retain the eligible references if one drops out and discard the topic if its list becomes empty. The same topic and Memory reference shape is usable by the planned PHY audit with its active, settled Memory eligibility policy. [Current recheck](../../crates/reliquary-application/src/perception_workflow/discovery.rs)
- Perception can select eligible settled Memories of any grade and current verified Observation Memories where the existing rules permit them. A Relationship requires grounded resolved participants and supporting Memory passages. Reuse the owner checks for those anchors before synthesis and when binding the final draft. References selected by an optional lookup follow the same eligibility and exact-source rules. [Perception input capture](../../crates/reliquary-application/src/observation_drafts.rs)

## Implementation order

1. **Change the two discovery contracts.** In `reliquary-ego`, make `EgoCandidate` a topic and exact references. Remove the old per-passage point structure and its test-only recheck helper. Keep the active recheck in Application on the selected references. In `reliquary-perception`, define a lean `ObservationProposal` distinct from the fully typed claim used by Synthesize, Verify and review. Move the existing transient status and time-scope types to Perception with their current serialized values.
2. **Carry the new forms through Application and Gateway.** Update discovery result types, the Ego batch retained by the worker and the input to `evaluate_proposal`. Bind the Perception proposal to its owner, history generation, proposed wording, selected references and resolved Entity revisions. Load the authorized passages under the current synthesis limits. Send the proposal as a starting point and let Synthesize derive the typed claim from the passages. A revision request also carries the fully typed prior draft and back-check hits through the existing revision context. Review gets that prior claim from the published Observation. Both paths use the existing synthesis runner. [Application creation](../../crates/reliquary-application/src/observation_drafts.rs) · [Application review](../../crates/reliquary-application/src/observation_drafts/review.rs)
3. **Keep publication tied to the typed draft.** Use the synthesized claim for the existing duplicate checks and route a matching draft to contribution. Continue through Receptors, Evidence back-check, the tested one-call Jev Verify contract and Finalize. Apply the same owner and evidence fences on retry, review and publication. [Observation creation](observation-creation-spec-2026-11-24.md) · [Reconsideration](observation-reconsideration-spec-2026-11-24.md)
4. **Switch the producers and callers together.** Use `memory_web_audit_v2` and `observation_synthesis_v2` as the internal model contracts. Advance their inference versions and include the audit contract version in the existing capture key. Rebind an unfinished capture using its saved source cursor and existing retry and duplicate handling. Preserve completed coverage. Keep REL and PHY 2.1 file layouts, stored Observation records and version 1 audit progress. Update the affected architecture, maintainer and behavioral contract pages when the code lands.

## Focused verification

Replace obsolete expectations in the listed tests and keep the number of test functions at or below the current count. Use the existing scripted endpoints and disposable REL fixture.

| Existing test seam | Update |
| --- | --- |
| Ego candidate contract | Replace `fresh_subject_can_have_multiple_typed_entries_from_one_source` with a topic and exact Memory reference check. Keep the Fresh-only rejection and independent Perception result. Remove `exact_evidence_change_preserves_unaffected_points` with its test-only helper. [Ego tests](../../crates/reliquary-ego/src/tests.rs) |
| Shared audit | Update the existing scripted schema name and request assertions. Reuse its separate list, batch limit, lookup and exposure checks. [Audit tests](../../crates/reliquary-memory-audit/src/tests.rs) |
| Perception structured stages | Update the existing discovery test to bind Memory and Entity slots without assigning roles. In the existing synthesis test, make the source establish a plan and assert that the synthesized claim carries the supported status and time. Keep its revision, duplicate and abstention assertions. [Structured stages](../../crates/reliquary-perception/tests/structured_stages.rs) |
| Application and mounted Gateway | Update the existing proposal fixtures. Give the scripted Relationship synthesis reply roles grounded in the supplied Memory text. Keep the mounted assertions for published citations, derived Observations, review and reopen. Run the existing Application cases for resynthesis and retained support, and the two existing bootstrap cases with the updated scripted audit. [Application tests](../../crates/reliquary-application/src/observation_drafts/tests.rs) · [Gateway tests](../../crates/reliquary-runtime-gateway/src/instance_perception/tests.rs) |

Run the affected test targets once.

```powershell
cargo test --locked --offline -j 1 -p reliquary-ego -p reliquary-memory-audit --lib
cargo test --locked --offline -j 1 -p reliquary-perception --test structured_stages
cargo test --locked --offline -j 1 -p reliquary-application --lib observation_drafts::tests::
cargo test --locked --offline -j 1 -p reliquary-runtime-gateway --lib instance_perception::tests:: -- --test-threads=1
```

Run the normal formatting and locked workspace compile check at integration. Completion requires both lists to reach their existing consumers in the new form, synthesized claims to retain the provenance and qualifiers required downstream, and existing version 1 progress to reopen through the mounted tests. [Development workflow](../development/workflow.md)

## Related docs

- [Shared audit and scheduling](memory-web-audit-spec-2026-11-24.md)
- [Observation creation](observation-creation-spec-2026-11-24.md)
- [Original Ego stages](https://github.com/Lokee86/Reliquary/blob/2b5bd0ea/docs/ego-web-synthesis-plan.md#L29-L39)

## Notes

Durable Ego handoff, the PHY worker, summary publication and context assembly follow this contract work. Update the existing hosted inference fixtures when Hermes and Mercury next take the new Core revision. [Hermes fixture](https://github.com/Lokee86/reliquary-hermes/blob/v0.3.3/tests/hosted_inference_probe.py#L131) · [Mercury fixture](https://github.com/itsflownium/mercury-hermes-rust-rewrite/blob/5b0c224f1ddabd0acebbd0456e1248c280893404/crates/gateway/tests/websocket.rs)
