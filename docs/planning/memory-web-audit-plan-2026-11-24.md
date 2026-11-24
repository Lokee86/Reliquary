# Shared Memory Web audit implementation plan

Parent index: [Planning](./INDEX.md)

## Purpose

Implement the [shared auditor and Observation contract](memory-web-audit-spec-2026-11-24.md). Use one authored REL cohort for both extraction contracts and one compatibility fixture for retained Relationships. Verify the workflow with scripted endpoints and actual owner files.

## Overview

Build the shared batch runner and authorized read path first. Connect both discovery contracts as two lists in one structured call. Add Perception verification and publication after discovery. Connect the normal Gateway processing path once the audit trigger and older-Memory cadence are decided. Verify each owning interface before the integrated run.

## Implementation sequence

| Step | Depends on | Work and exit condition |
| --- | --- | --- |
| 1. Shared audit and source references | Baseline | Add the internal audit crate. Implement deterministic batch packing, exposure accounting, request-local citations, size limits and one optional evidence round. Exercise the combined request and validate the two typed outputs independently. Update the Cargo workspace, lock and [approved DAG](../../architecture/target-crate-dag-2026-11-03.json) for the final crate directions. Exit when one batch request returns both lists, rejects ineligible citations and preserves valid work across a failure in the other list. |
| 2. Open one authorized working set | 1 | Reuse [owner-session keyset pagination](../../crates/reliquary-owner-session/src/session/management_inspection.rs), the [Memory store seek](../../crates/reliquary-memory-core/src/store.rs), [exact source checks](../../crates/reliquary-owner-session/src/session/perception_lifecycle/support.rs) and existing lexical postings for bounded internal lookups. Page metadata first, then hydrate selected exact bodies. Follow the structured evidence request pattern in [Insomnia](../../crates/reliquary-insomnia/src/model_pipeline.rs). Supply the shared batch with Core-assigned eligibility, including Fresh-only evidence for Ego in both mandatory input and follow-up search. Exit when both lists use authorized slots and coverage completes for each list under the batch budget. |
| 3. Select Perception work and Relationship candidates | 2 | Use settled Memory revisions for routine work and bounded keyset traversal for bootstrap and cold rotation. Derive current grades from authoritative Freshness state with same-turn events included. Use a narrow derived current projection where needed. Keep initialization out of the ordinary open path. Validate against [Freshness replay](../../crates/reliquary-freshness-store/src/store.rs) and maintain it for enrollment, imports, activity and same-turn updates. Reuse [mention inputs](../../crates/reliquary-owner-session/src/session/entity_mention_inputs.rs), [direct evidence](../../crates/reliquary-owner-session/src/session/identity_evidence.rs) and [participant lookup](../../crates/reliquary-perception-store/src/relationship_store.rs). Factor out the needed lookup pieces without running the whole Entity resolver. Exit when both cold bands run under recent-work saturation and deterministic candidate lookup handles migrated typed mentions and current overrides. |
| 4. Publish and reopen Observations | 3 | Add the typed ordinary/relational Observation history and batch progress in Perception Store. Reuse the [typed owner publication and retry pattern](../../crates/reliquary-owner-session/src/session/identity_receipt.rs), [physical-prefix validation](../../crates/reliquary-owner-session/src/session/replay_prefix.rs), and [checkpoint](../../crates/reliquary-owner-session/src/session/checkpoint.rs). Add historical proof, reclamation/copy support and preflight for unsupported transformations. Expose paged current/historical reads through the existing Root inspection grants. Exit when a mixed manual Relationship and Observation REL reopens from disk with stable IDs, exact evidence and resumed progress. |
| 5. Connect normal processing and reconsideration | 2–4 | Compose selection, combined discovery, separate Perception verification and guarded commit in Application and Gateway. Use [owner worker leases](../../crates/reliquary-runtime-gateway/src/owner_workers.rs) and the existing [owner access fences](../../crates/reliquary-runtime-gateway/src/instance_owner.rs). Use explicit run and sampling requests through the processing seam in focused tests. Agree on the production trigger and cadence before wiring automatic invocation. Run at most one Perception batch per owner at a time. Keep pending checks and retry scoped to the relevant source and route. Use source revision and participant changes to target previously published claims, and revisit affected heads after a restart. Exit when an ordinary processing session discovers, persists and later reconsiders an Observation, with Ego using the same audit code through its own extraction stage. |

Move each responsibility directly to its final owner. The integration window starts when the new crate edges and native projection land and ends when the ordinary processing path, inspection, replay and maintenance are all connected. Run the affected test seam as each step lands. Existing 2.1 Relationship decoders and APIs stay as the persisted/user-facing compatibility boundary. New synthesized claims enter only through the Observation writer. Ship after the full target builds and passes the integration checks.

## Focused verification

| Seam | Cases and required results |
| --- | --- |
| Audit crate and extraction contracts | One synthetic source set fits in one batch. A larger budget-constrained set, including an oversized Memory, reaches every selected mandatory slice across batches. Optional search can cite an authorized source outside a batch and cannot count it as mandatory exposure. A repeated batch, an empty result, a failed endpoint, and a forged or malformed citation exercise completion and retry. Assert that one structured discovery call emits subjects and propositions with explicit status, time scope and eligible citations. Offer a mixed-grade batch, reject a cold Ego citation and keep the valid Perception candidate. Accept one Fresh source with multiple distinct supported status/time entries. Check Perception verification independently of extraction. |
| Mounted Core workflow | Use one authored REL with recent, Stale and Dormant Memories, two related software Entities, and a same-name or co-occurring distractor. Use explicit audit and sampling invocations, a controlled accepted-turn counter and scripted completions. Observe bounded routine and sample work under load, both bands rotating across restart, and same-turn grading. Seed one retained typed mention and a newer override. Assert an actual evidence-backed Relationship, an ordinary Observation, no unsupported claim, and no accepted-use or Freshness reward from the audit. Change a cited source, observe pending or withdrawal after reevaluation, and preserve the preceding revision. Pause a model call once, revoke access or replace the mount, and prove stale output cannot publish. An identical retry publishes only one revision. |
| Native reopen and copies | Reuse [mounted inspection](../../tests/oc06_inspection.rs), [checkpoint cases](../../crates/reliquary-owner-session/src/session/checkpoint/tests.rs), and [verified maintenance](../../tests/g4_maintenance.rs). Open a pre-feature 2.1 file and an Observation-bearing copy with an existing manual Relationship. Check ordinary reopen, exact checkpoint and an Observation-bearing suffix, then one verified copy/reclamation case. Assert the old Relationship ID/revision is unchanged, the Observation provenance and progress survive, interruption exposes only the complete prefix, and an unsupported transformation refuses before writing a destination. |

These checks use small authored files and a recording endpoint. The resource assertion is bounded work per poll with no full-REL body load for an ordinary batch, no Freshness history replay per candidate, and no audit work on a passive open. Synthetic scale cases exercise the cursor and batch caps at the unit or owner-session seam. Corpus quality calibration and wall-clock performance benchmarks are separate work.

## Completion

- Verify both contracts use one structured discovery call per ordinary audit batch and the layer directions match the updated DAG. Review the actual Gateway-to-owner read and publication paths for direct store bypasses and duplicate writers.
- Run the three focused seams first. At integration run Rust formatting, the locked offline workspace check and test baseline once, plus the architecture and applicable documentation checks. Use the existing repository workflow and CI for required checks outside the feature's focused test cases. [Commands](../development/workflow.md)
- Update the affected architecture owner pages, public inspection documentation, supported-format and maintenance limits, coverage and behavioral contracts alongside implementation. Record observed acceptance results and any remaining reader/version limitations.

## Related docs

- [Shared audit and Observation specification](memory-web-audit-spec-2026-11-24.md)
- [Roadmap](roadmap.md)

## Notes

One fixture can carry the routine, low-Freshness, co-occurrence and correction cases. Keep provider quality measurements and the existing cold-attach investigation separate from the feature acceptance. [Open performance follow-up](roadmap.md#owner-open-performance)
