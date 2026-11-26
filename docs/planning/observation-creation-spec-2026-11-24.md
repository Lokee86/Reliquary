# Observation creation

Parent index: [Planning](./INDEX.md)

The [audit handoff follow-up](memory-web-audit-handoff-spec-2026-11-26.md) defines the next revision of the Propose and Synthesize boundary.

## Purpose

Turn a claim proposed by the shared Memory Web audit into a verified Observation with exact provenance. Perception needs to check the existing REL for overlooked support, exceptions and contradictions before making the claim available to retrieval.

## Overview

**Propose → Synthesize → Receptors → Evidence back-check → Verify → Finalize.**

The shared audit produces a Perception proposal. Synthesize writes the exact candidate claim and chooses its citations. Receptors guide a search of the existing REL. Verify makes one native Jev decision request containing the exact draft, its citations and the back-check results. Finalize publishes the verified draft through the owner writer.

The release processes one authorized REL. Ordinary and relational Observations can cite eligible Memories and verified, current Observation revisions under the relational evidence rules. A published Observation has one stable typed identity and revision history and can be presented as a derived Memory. The same release includes review and dependency propagation. Ego continues to consume its own Fresh-only audit output. [Shared audit specification](memory-web-audit-spec-2026-11-24.md) · [Observation reconsideration](observation-reconsideration-spec-2026-11-24.md)

## Stage contracts

| Stage | Work and output |
| --- | --- |
| **Propose** | The shared structured audit gives Perception a bounded list of propositions, an evidence status, time scope and request-local source slots. Relational proposals also carry resolved Entity and optional role slots. Core checks field shape, source eligibility and slots before admitting a proposal. Memory evidence must be readable and settled. An Observation source must be verified and current with eligible underlying provenance. Relational proposals require current resolved participants. An empty proposal list completes creation work for the batch. |
| **Synthesize** | One general model request per admitted proposal reads its authorized evidence and produces one exact proposition, status, supported time scope and chosen citation slots, plus relational participants and roles where applicable. It may abstain. Keep one underlying claim per draft. Accept direct statements or bounded conclusions supported by cited premises. Unsupported generalizations and causal guesses stay unpublished. Core binds selected slots to original passages and validates participants, citations, time fields and limits. Use grounded Chronos interpretation where available and retain uncertainty left by the sources. Compare against bounded existing claims for duplicate detection before spending on receptors. |
| **Receptors** | One general model request generates separate descriptions of evidence that could support, challenge, qualify or otherwise change the exact synthesized claim. Embed the descriptions separately on the configured route. Bind the set to the draft content, status, time scope, participants, receptor policy and vector profile. Preserve separate receptor matches for ranking and deduplication. Publish that same set with the verified draft for later contribution routing. |
| **Evidence back-check** | Use the draft's receptors to search the authorized REL for additional passages. Use the existing owner-local retrieval, Entity and Memory Graph indexes. Include otherwise eligible Stale and Dormant Memories. Bind every result to its exact source cut with adjacent context. Produce a bounded set of distinct hits and carry their match scores and source identity locally. This stage is deterministic search and selection using the already-generated receptors. |
| **Verify** | Validate the frozen draft and passages in Core, then issue one native decision request. Ask for a decision on the whole claim and a separate judgment on each back-check passage within that request. Check the reply and apply the gate below. The verdict operates on the exact draft that Synthesize wrote. |
| **Finalize** | On final `stands_as_written`, revalidate the owner cut, routes, sources, draft and receptors. Publish the typed Observation and its accepted provenance, enrollment and related progress atomically. A terminal abstention or unresolved outcome can advance the completed batch without publishing an Observation. |

One ordinary draft therefore uses one synthesis request, one receptor-generation request, embedding operations, and one Jev request after shared discovery. The audit's existing optional discovery lookup remains part of Propose. Verify consumes the supplied evidence in a single request. Creation borrows the earlier design's evidence-led reasoning and verification, with receptor search positioned before the publication decision. [Historical evidence and verification design](https://github.com/Lokee86/Reliquary/blob/a57a823f87399e4c2028dd74987c47cba800067e/docs/decisions/0033-perception-entities-observations-and-ambiguity.md#L169-L214)

## Draft and evidence boundaries

- Keep one typed draft containing the exact wording, profile, status, time scope, participants and selected citations. Its internal identity binds the originating proposal and owner history generation, stage policy identities, relevant route identities, and exact inputs. Start with the audit's existing `observed`, `planned`, `requested`, `possible` and `expected` statuses. An observed outcome requires grounded evidence of the outcome. A plan or request supplies evidence for its own status.
- A citation binds the owner and history generation, typed Memory or Observation ID, exact revision and body identity, field, and UTF-8 range. An Observation citation resolves to the exact published claim and its eligible underlying Memory lineage. Core issues request-local slots and checks returned slots against the authorized capture. Distinct excerpts of one original record share a source lineage. Entity links, graph links, file names and receptor scores guide selection. Grounding comes from the cited passages.
- For a single draft, use up to eight admitted source slices and 24 KiB of source text, with 64 KiB serialized request and 16 KiB response caps, subject to the route's lower limits. Start with up to two receptors per category and up to eight ranked matches per receptor before merge. Admit at most three unique back-check passages to Verify, with an 8 KiB combined budget for their excerpts and adjacent context. Reserve room for the best eligible challenge and qualification matches when both are present, then fill remaining places by similarity with a stable source/revision/span tie-break. Reuse lexical or structural candidate routes where vector coverage is sparse. Search eligibility includes readable Stale and Dormant Memories independently of Freshness rank.
- Select metadata and evidence windows before hydrating bodies. Keep original citations out of the back-check list when they refer to the same exact passage. Trim literal adjacent context at safe UTF-8 boundaries. Preserve the exact quoted excerpt and separate its immediate same-source `before` and `after` text. Keep source context related to the excerpt and within the captured revision. If required evidence or a required route is unavailable, keep the work pending. An available lookup with zero hits is a valid result.
- A distinct candidate can concern the same participants in a different role, proposition, time or scope. Check exact duplicate identity using profile, normalized proposition, status, participants, and supported time scope. Synthesize can identify a known equivalent with a Core-issued existing Observation slot. Validate its identity, profile, time and participants, then route the evidence to existing-Observation contribution under that stable ID. Recheck exact duplication at publication to prevent concurrent duplicate writes. Existing manual or imported Relationships keep their current lifecycle and can route candidates to sources.

## Tested one-call Jev verification

Use the native `typesafe/jev-1.13` decision endpoint through a Perception verification consumer of Switchboard's shared native decision route. This policy binds the native model and prompt version. Synthesis and receptor generation use the general route, and receptor vectors use the configured embedding profile. Missing bindings leave the corresponding stage pending. Reuse this Verify contract for exact review drafts and successors. The contribution contract determines whether new evidence warrants review.

Send only these state fields to this contract:

| Field | Contents |
| --- | --- |
| `claim` | The exact synthesized claim, including supported time, status and scope as expressed in its wording. The wording agrees with the typed draft fields validated by Core. |
| `citations` | Chosen evidence rows with `id`, `source_id`, `text`, `before` and `after`. The source ID is an opaque per-request lineage token. Core validates the exact standing and proof of cited Observations and supplies their grounded source passages in this same row shape. Reuse the same lineage token when an original Memory is reached directly or through a derived citation. |
| `backcheck_hits` | Additional evidence in the same row shape. Dates, context and component boundaries come from the source wording. |

The single request contains `verify` and one `hit-{hit}` question per back-check row. Verify returns `stands_as_written`, `needs_revision` or `unresolved`. Each hit returns `material_challenge`, `material_qualification`, `weak_or_uncertain` or `no_material_effect`. Keep the exact prompted distinctions in the appendix, including partial cancellation versus a whole event, preliminary versus final measures, and a newly established exception versus a limit already in the claim.

- Validate the exact expected answer IDs, native choice type and legal choice for each answer. If optional probabilities are present, require known choice keys, a value for the selected choice, and finite values in the inclusive range from zero to one. Probabilities and confidence are diagnostic and carry no publication authority.
- When `verify` is `stands_as_written` and any back-check answer is `material_challenge` or `material_qualification`, the final outcome is `unresolved`. Otherwise keep the overall verdict. This is the tested rule. Hit judgments neither author a revision nor supply additional citation slots.
- A valid `needs_revision` can return to Synthesize once per proposal, with the original citations, back-check results and dispositions. Synthesize may abstain. A revised draft must change its claim or grounded evidence before reentering Verify. Reuse receptors and matching back-check work when their bound claim, relevant inputs and policies still match. Rebuild them for a materially changed claim. Hold an unresolved draft or exhausted revision budget until relevant evidence changes. Keep the same proposal identity and distinguish the new draft identity. Each draft has one Jev request on the normal path.
- Consume the validated choices and optional diagnostics from the response. Selected citations belong to Synthesize. Back-check evidence becomes positive provenance only when a synthesized draft selects it and Verify clears that exact draft. Malformed replies leave the affected stage pending without advancing progress. Apply bounded backoff to transport failures and rebind changed inputs or routes before retry. [Switchboard decision transport](../../crates/reliquary-switchboard/src/transport.rs)

## Finalize, recovery, and compatibility

Require the same authenticated Instance's current `ManageRel` grant and writable REL mount all the way through publication. Capture relevant owner data before provider calls and release writer locks while waiting. Recheck owner/generation and mount, source eligibility and exact citation and back-check revisions, relevant Entity/mention decisions, expected head or absence, and the routes and draft identity that produced the decisions. Reject stale results and recapture affected inputs. Unrelated writes can coexist.

Publish the verified claim, stable Observation ID and initial revision, exact citations and source lineage, accepted evidence and policy identity, and the matching receptor set through one owner-session commit. Enroll the new derived Memory in the existing REL Freshness score at +100 once, through the existing Freshness boundary. Creation earns no graph propagation or accepted-use event. Publish any completed Perception progress with its semantic results. Drafts are temporary work; searches and audit output can inspect source Memories under the existing grant while the draft is being processed. Only finalized Observations enter retrieval, history inspection and subsequent routing. Identical committed input reuses its result after a retry or lost acknowledgment. Restart resumes from durable progress and may redo uncommitted stages. Keep raw provider requests, replies, source copies and private rationale outside the REL.

Use the audit's versioned Observation carrier, physical-prefix validation, exact checkpoint and suffix recovery, historical proof and verified copy/maintenance rules. Keep native REL/PHY 2.1 headers, owner IDs and existing manual Relationships compatible. Pre-feature RELs open with empty Observation state. Document the minimum new reader for RELs that acquire Observation records. [Audit publication and compatibility](memory-web-audit-spec-2026-11-24.md#publication-reconsideration-and-compatibility)

## Ownership and implementation plan

| Step | Owner and dependency | Work and exit condition |
| --- | --- | --- |
| 1. Admit a proposal and synthesize its draft | Perception and Application, after shared audit and authorized source capture | Connect the Perception list to one bounded structured Synthesize step. Validate source slots, status/time, participants and dedup identity at the authorized capture. Exit when one audit batch yields a valid exact draft, clean abstention and no durable Observation yet. Reuse the audit's approved crate direction. |
| 2. Generate receptors and back-check evidence | Perception and Application, after step 1 | Generate and bind receptors to the draft. Use existing embedding, owner lookup and indexed retrieval to select exact contextual passages. Exit when a draft finds a relevant Stale or Dormant counterexample and a compatible passage about a different component without whole-REL body hydration. |
| 3. Bind and enforce the tested verification contract | Switchboard, Perception and Application, after step 2 | Add the native verification consumer. Send one versioned request per draft with exact template and structured replies. Apply only the tested disposition rule. Wire a single bounded resynthesis on needs revision. Exit when the verification seam can distinguish clear publication, revision and unresolved outcomes without a follow-up verification request for the same successful draft. |
| 4. Finalize on the normal owner worker | Gateway and owner-session, after steps 1–3 and audit publication foundations | Reuse the existing authorized owner worker and canonical Observation writer. Bind verified draft, provenance, receptors, Freshness enrollment, receipt and progress in the grouped native commit. Complete replay, checkpoints, historic proof and verified maintenance. Exit when a verified Observation is available at the Root boundary, and reopen and exact retry preserve one result. |
| 5. Close acceptance and publish the contract docs | After steps 1–4 | Run the focused seams and the applicable repository integration checks once. Update the public inspection/configuration contract, owner and maintenance docs, behavioral coverage, and the architecture map at implementation time. Record observed acceptance, including supported reader behavior. [Development workflow](../development/workflow.md) |

## Focused acceptance

| Seam | Required proof |
| --- | --- |
| Perception and native decision contract | A recording endpoint verifies exact state, frozen question texts and one request per draft, including all per-hit questions. Replay representative scripted replies for a partial cancellation that leaves the named fallback intact, an already stated limit, a preliminary count consistent with the final count, a genuine new qualification, and a genuine conflict that vetoes an overall clearance. Check no-hit, malformed and non-clearing responses. Confirm that one concrete revision can yield a new cited draft and that unresolved evidence stays unpublished. |
| Mounted workflow and guarded publication | Use one authored REL with settled active Memories, Stale or Dormant evidence omitted from the initial proposal, and an existing manual Relationship. Create an ordinary and a relational Observation, trace the six steps, and inspect only the verified wording, citations and bound receptors after publication. Race a suspended verification against a changed source or revoked grant and recover without publishing stale output. Verify one +100 enrollment and one durable outcome across exact retry and reopen. |
| Native history and compatibility | Reopen the same fixture through the ordinary and verified-checkpoint paths and include one verified copy or reclamation case. Check exact history, identities, progress and retained Relationship behavior. Admit a pre-feature 2.1 fixture with an empty Observation projection. Reuse the audit's maintenance preflight and supported transformation checks. |

Completion requires the ordinary Gateway path to enforce the six-stage contract, typed provenance, one native verification request per normal draft and deterministic publication. Keep routine verification offline with scripted endpoints. The integrated release gate also covers audit cadence, older-Memory routing, post-publication review and dependent Observations. Wider synthesis and future corpus calibration keep their existing planning scopes.

## Related docs

- [Same-REL release implementation plan](perception-implementation-plan-2026-11-24.md)
- [Shared Memory Web audit](memory-web-audit-spec-2026-11-24.md)
- [Audit implementation plan](memory-web-audit-plan-2026-11-24.md)
- [Observation reconsideration](observation-reconsideration-spec-2026-11-24.md)
- [Planning roadmap](roadmap.md)

## Notes

The unchanged component-aware Jev contract ran on 24 fresh authored drafts with one native request each. It cleared all 12 supported drafts, including six partial-change cases. It held all 12 drafts that warranted revision or had unresolved evidence, and flagged all six direct conflicts in the per-hit judgments. The overall answers held those drafts by themselves. An earlier rerun exercised the veto on an overall clearance, so keep an explicit veto case in the implementation tests. Exact disposition agreed in 22 of 24 cases. Two genuine conflicts were sent to revision rather than remaining unresolved. That evidence supports the initial publication gate on these authored cases and leaves synthesis quality, search recall and real-corpus verification to their own evaluation.

## Tested Jev question templates

Use the overall `verify` question and instantiate one `hit-{hit}` question per back-check passage ID in the same native request. The following wording comes from the fresh live trial.

```json
{
  "verify": {
    "criteria": {
      "needs_revision": "The evidence establishes a concrete material correction or narrower supported version of the same claim. The available evidence is sufficient to tell what must change, for example a documented exception, a narrower time or scope, or wording that overstates what the evidence establishes.",
      "stands_as_written": "The selected citations ground all material parts of the exact claim, with its stated scope, certainty, and valid time. The back-check reveals no undefeated contradiction or material exception within that coverage. Relevant evidence about a different population, component of an event, or period can coexist with the claim. A record of the named fallback taking place can ground that claim despite prior cancellation of the original mode.",
      "unresolved": "Essential evidence is missing or inconclusive, the referent, scope, or timing cannot be resolved, or comparably credible sources conflict without a grounded way to reconcile them. There is no clear supported correction or narrower version that resolves the underlying claim."
    },
    "instructions": "Verify the exact synthesized Observation in claim. Evaluate what the selected citations establish, then consider each back-check hit for support, a material exception, a correction, or an unresolved conflict. A stand requires the selected citations to ground the whole claim as written. Identify the exact part or mode of an event and its stated outcome. A notice canceling one part of an event applies to that part unless it says the entire event or the alternative was canceled. If a notice cancels an outdoor performance and leaves the indoor fallback undecided, it can coexist with a completion record showing the indoor fallback went ahead. For a claim only about the indoor fallback, the outdoor cancellation is compatible and does not require revision. Compare the same referent, component, scope, status, and applicable time. Distinguish the time something was true from when it was reported. Consider the documentary basis of each passage. Passages with the same source_id share their underlying record. Decide the disposition of the whole Observation. Do not rewrite it, select citations, or explain your choice.",
    "type": "choice"
  },
  "hit-{hit}": {
    "criteria": {
      "material_challenge": "The passage directly asserts a fact incompatible with a material part of the claim about the same subject, event or covered scope. This includes placing the same event at a different time, denying the exact activity the claim describes, or giving a conflicting value at the same scope and counting basis. A different mode or a differently defined count is insufficient.",
      "material_qualification": "The passage establishes a new exception, condition, or boundary that excludes or changes part of what the wording currently claims. Use this only if the exact claim would need an edit. A narrower boundary relative to the wider world is already reflected when the claim limits itself to that same scope.",
      "no_material_effect": "The passage supports the claim, confirms a limit it already states, or describes a different subject, period, mode of activity, counting basis, or record stage that can coexist with what it asserts.",
      "weak_or_uncertain": "The passage may bear on the claim, but the wording, identity, scope, timing, reported status, or connection to the claim is unclear. An indirect hint or tentative plan is insufficient."
    },
    "instructions": "Compare only back-check passage {hit}, including its before and after source context, with the exact claim. Decide what this passage itself bears on. First identify exactly what the claim says occurred and what the passage says was canceled or happened. A canceled part or mode leaves other parts open unless the passage explicitly includes them. For example, a notice canceling an outdoor performance and leaving the indoor fallback undecided is compatible with a claim that the indoor fallback went ahead. Choose no_material_effect for that notice against the indoor claim. A notice explicitly canceling the indoor fallback would bear on the indoor claim. Treat the passage's asserted event or valid time separately from the date of its record. Two different dates for the same event can conflict. An established later change can coexist with a historical snapshot, and a different subject or scope can coexist with the claim. Check the exact activity, measurement basis, and record stage on both sides. Cancellation of one mode of an activity can coexist with another mode taking place. A preliminary count of all items can coexist with a final count of eligible items. The passage must make an assertion about the same mode, measure, or stage to contradict that aspect of the claim. A different value for the same measure, or a different date for the same event, remains a challenge when the passage provides no basis to reconcile it. Do not infer a separate event or a resolution of the disagreement without evidence in this passage. Do not weigh this passage against the chosen citations or other back-check hits. Compare any stated restriction with the scope, time, or conditions the claim already states. Ask whether the claim would have to change to accommodate the restriction. A passage saying a rule applies only at one site confirms the scope of a claim already about that site. It does not add a qualification to that claim. Return one choice.",
    "type": "choice"
  }
}
```
