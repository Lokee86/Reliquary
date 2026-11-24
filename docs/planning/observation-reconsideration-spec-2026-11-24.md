# Observation reconsideration

Parent index: [Planning](./INDEX.md)

## Purpose

Keep published Observations useful as new evidence arrives. Perception needs to tell when a new passage warrants review, what retrieval may use during review, and how a revision affects higher Observations that cite it.

## Overview

The shared Memory Web audit discovers initial candidates. Creation synthesizes, back-checks, verifies and publishes them with exact evidence. Reconsideration starts from an existing Observation and gathers evidence about that claim. One batched Jev request judges whether any of the new passages warrant review. Only a trigger or an invalidated citation starts verification of the existing claim. A reopened claim retains its identity and history. [Shared Memory Web audit](memory-web-audit-spec-2026-11-24.md) · [Observation creation](observation-creation-spec-2026-11-24.md)

The same-REL release includes first publication, reconsideration and Observation-to-Observation evidence. Review starts after a claim's first publication. Ordinary and relational Observations use one lifecycle. An Observation participates as a derived Memory in retrieval and as a proposition in later audits, with its own typed revision and provenance carried through both paths. An Entity or a manual Relationship record can route work to evidence. The underlying grounded passages establish the proposition. [Historical Observation decision](https://github.com/Lokee86/Reliquary/blob/a57a823f87399e4c2028dd74987c47cba800067e/docs/decisions/0033-perception-entities-observations-and-ambiguity.md#L72-L104) · [Relational Observation decision](https://github.com/Lokee86/Reliquary/blob/a57a823f87399e4c2028dd74987c47cba800067e/docs/decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md#L215-L257)

## Triggers and candidate selection

| Input | Action |
| --- | --- |
| A cited Memory revision, bound participant, or required evidence becomes invalid | Make the head pending as soon as the owner exposes the change. Queue targeted verification directly. A corrected or explicitly superseding Memory can be offered to the verifier with its own exact citation. |
| A verified Observation used as evidence becomes pending | Apply pending standing to its dependents immediately. Defer their model work while the upstream result is unsettled. |
| Newly settled evidence or a sampled older passage reaches a challenge, qualification, other reconsideration receptor, or applicable exact structural route | Gather at most five unique passages for the target revision per request. Commit pending as soon as one accepted material challenge or qualification survives the time and scope checks. |
| A hit reaches only a support receptor, or Jev returns a weak, unclear, or irrelevant result | Leave current standing alone. Support-only receptor hits need no trigger call by themselves. An independently initiated verification can consider these passages as context. Weak hits do not accumulate into an automatic trigger in this slice. |

Reuse the audit's source-readiness rules and bounded owner-local progress. Route through exact dependency lookup and existing Entity, participant, and Memory Graph indexes before receptor similarity. Multiple routes for a passage share one candidate. Use the receptor set committed with the verified Observation. Build a successor set in one separate structured Perception request when review materially changes the proposition, time scope or participants. Keep distinct support, challenge, qualification and other reconsideration descriptions and embed them separately using the configured embedding route. Tag receptors by the exact head and route/profile. Use a bounded result budget and ignore stale receptor entries. Exact routing can run while receptors are unavailable. Receptor matches and Entity overlap confer eligibility to compare. The verifier establishes support from cited passages. [Receptor decision](https://github.com/Lokee86/Reliquary/blob/a57a823f87399e4c2028dd74987c47cba800067e/docs/decisions/0033-perception-entities-observations-and-ambiguity.md#L199-L214)

Normal Perception processing queues work when sources settle or change. Each shared audit also submits its selected older passages through the same bounded Memory-to-receptor lookup for existing Observation heads. Use the sampled Memory's compatible vector and the existing exact Entity and dependency routes. A match can queue contribution independently of a discovery result. [Audit cadence and sampling](memory-web-audit-spec-2026-11-24.md#automatic-audit-cadence-and-sampling)

Deduplicate by owner history generation, exact source revision/body and field/span, exact Observation ID and revision, and contribution policy version. Before Jev dispatch, skip an exact passage already cited or judged for that head. Multiple routes and samples share the same pending comparison. Record a completed key only with an accepted contribution result, atomically with any resulting pending marker. Recover queued work and completed comparisons from committed progress after reopen. A changed passage, Observation revision or contribution policy can make the pair eligible again. A repeated sample, route rebind or regenerated equivalent receptors reuse existing comparisons.

Service pending targets and changed dependencies before new receptor matches through the owner-worker lease. Keep work bounded and allow progress on other targets. Use the contribution limits of 24 KiB of passage text and 64 KiB serialized, subject to the route's lower limits. Carry additional eligible hits into later bounded requests. Count independent evidence by underlying lineage. A wall-time deadline, Freshness grade change or route replacement alone leaves existing standing unchanged. Rediscovering an old passage confers no Freshness award.

## One-call Jev contribution contract

Use Switchboard's native decision endpoint with the tested `typesafe/jev-1.13` and a versioned Perception contribution policy. Add an Observation contribution consumer that uses the existing shared native decision route. A missing native binding leaves eligible contribution work waiting. Review uses the tested native Verify contract from creation to decide whether an exact statement can stand.

Send one request per target revision and admitted batch. The request state contains `existing_observation` and `evidence_hits`, keyed by Core-issued passage IDs. Use the stored claim wording and exact raw passages, including temporal language present in the text. Core retains the owner, target revision, citation coordinates and participant identities in its capture. The model receives no author-supplied date or scope fields. The three choices for each hit are independent questions within that one request. The [tested question templates](#tested-question-templates) give the exact first policy.

- Treat `material_challenge` and `material_qualification` as potential triggers. `weak_or_uncertain` and `no_material_effect` leave standing alone.
- Suppress a qualification when the time choice is `later_change_only`. Preserve a challenge, a first-effective date that disproves an earlier claim, a later report about the earlier period, and a change that ends an ongoing claim, subject to the other gate. A time choice never creates a trigger.
- Suppress either potential trigger when the scope choice is `explicitly_separate_scope`. Keep subsets, exceptions and unclear scope eligible for the original verdict. Scope alone never creates a trigger. Use choices directly, without a score cutoff or a follow-up decision request.
- Validate the answer IDs, choice types and allowed values against the request. Missing, extra or invalid answers leave the batch unclassified and its progress uncommitted. Treat provider failures as retryable with bounded backoff. Hold malformed responses for changed input or binding, or explicit retry. Preserve an already-pending review until it can be verified.

A surviving contribution trigger commits the pending marker, triggering citation references and completed batch identity together. Queue the head for its next eligible review, independently of the shared audit counter. A known source invalidation makes standing pending as soon as the relevant owner change is visible and queues the same work directly. The contribution decision serves as a trigger. A stored or verified Chronos interpretation can help establish a published validity interval. [Chronos and archival temporal evidence](temporal-search-spec-2026-11-23.md)

## Review and successor verification

1. Capture the exact head at the authorized owner cut, the usable original citations, triggering or invalidated references, and any supported replacement passages from bounded lookup. A corrected source or a settled upstream Observation successor can supply a new exact citation. Give accepted material evidence priority over optional context in the verifier's bounded window. If required material evidence still lies outside that window, retain pending work.
2. When the accepted citations remain eligible, use the current wording and exact citations as the first Verify draft. When those citations require replacement, use the existing Synthesize stage to prepare one grounded draft from the evidence that remains available. A draft carries the precise claim, time scope, participants, exact citations and back-check inputs. Prepare or reuse receptors under the creation rules.
3. Send one native Jev Verify request for the frozen draft. Use the exact tested creation template, whole-claim and per-back-check choices, and veto for a material challenge or qualification. A valid `needs_revision` can use the creation plan's single bounded resynthesis. A revised draft gets its own identity and one Verify request. A reaffirmation with identical eligible evidence retains its exact revision and receptors.
4. Publish a successor only after Verify clears it and owner-session rechecks the same grant, mount, head, sources and route. When reviewed evidence leaves the claim refuted, unsupported or unresolved after the bounded attempt, withdraw its current standing and record the reason. Keep it pending if a required source or route is temporarily unavailable or a response fails validation. Preserve the prior revision. [Tested Verify contract](observation-creation-spec-2026-11-24.md#tested-one-call-jev-verification)

## Standing and retrieval

| Effective standing | User-facing Observation lookup | Current retrieval and reuse |
| --- | --- | --- |
| Current | Return the verified head, exact revision and provenance. Display any established validity interval. | Eligible for normal current context within its evidence and time scope. It can be cited by a new Observation after source readiness checks. |
| Pending reconsideration | Return the last verified wording with an explicit **Pending review** state and a reason, such as new evidence, changed support, or an upstream review. Direct inspection can show available cited and triggering passages under the current grant. | Exclude before ranking current Observation candidates, preparing new agent context, and admitting derived evidence. Keep eligible source Memories available on their own. |
| Withdrawn or superseded | Show the retained revision and its status in history. A qualified successor carries its verified time and scope. Unavailable source bodies retain their provenance references. | Only a verified successor can enter current context. Retrieval of a retained revision for inspection or accepted-manifest recovery remains historical. |

After verification, keep the same revision if the exact claim and accepted provenance remain valid. Durably record the completed review and clear pending. Append a successor on the same Observation ID if the claim, time scope, participants or accepted citations change. Publish the verified narrower or corrected wording when evidence supports it. Withdraw current standing when evidence refutes the claim or usable support cannot be established. Record whether review found refutation, lost support, or unresolved evidence. A transport or validation error leaves the previous head pending. All completed outcomes preserve earlier revisions. Keep semantic validity separate from provider and evidence availability.

Keep existing source-Memory APIs compatible. Add typed Observation hits and standing to the authorized Observation lookup and to context paths that admit derived Observations. A derived hit uses the canonical Observation revision and citation identity. Use that revision as the sole authoritative representation of the claim. Apply the same eligibility check at the Gateway retrieval cut and just before a new handoff. If standing changed while preparing delivery, reject or recompute that delivery under existing stale-cut handling. Honor already-accepted historical manifests. Search, inspection and review attempts confer no accepted-use credit. The Freshness awards below are tied to originating activity and verified changes. [Retrieval cut](../../crates/reliquary-runtime-gateway/src/instance_passive_retrieval.rs) · [Handoff revalidation](../../crates/reliquary-runtime-gateway/src/instance_retrieval_delivery.rs)

## Freshness from receptor hits and verified changes

An eligible receptor match from new accepted REL activity uses the existing semantic Freshness curve. Start with the same inclusive cosine threshold of 0.75 and a +1 to +10 local award across support, qualification, challenge and other reconsideration receptors. Reuse the numerical policy and treat receptor-specific threshold calibration as separate from the focused bookkeeping checks. Take the strongest eligible match once per originating activity and stable Observation ID, even if several passages, receptors or semantic match routes find it. Support-only hits can earn the semantic award without a Jev request. Freshness affects retrieval priority for eligible claims. The trigger and standing rules above still govern review and use. [Semantic boost and candidate reduction](../../crates/reliquary-freshness/src/semantic_policy.rs) · [Existing local reinforcement policy](https://github.com/Lokee86/Reliquary/blob/a57a823f87399e4c2028dd74987c47cba800067e/docs/decisions/0039-unified-deterministic-memory-freshness.md#L33-L39)

| Verified review outcome | Total award for the credited activity and Observation |
| --- | ---: |
| Material qualification that narrows the proposition's verified scope, conditions or valid time | +25 |
| Material correction that overturns the earlier conclusion and publishes a verified successor | +50 |
| Reaffirmation, wording-only change, citation-only change, or withdrawal without a verified replacement | Keep the original semantic award |

Apply only the difference at successful successor publication. An original +6 earns +19 more for a qualification or +44 more for a correction. Use the same REL clock and score ceiling of +100. The stable Observation ID carries the score to the verified successor. Credit one eligible receptor activity that the verified result cites as responsible for the change. When several qualify, choose the strongest existing semantic match and break ties by stable source identity. Commit the increase and its receipt with the successor. Keep the originating activity key and the amount already awarded. If another eligible match raises the base award, apply only the positive difference. Retries, copied passages, regenerated receptors and reopened work reuse the recorded award. Apply the award locally to that Observation. Dependent reviews continue under the existing provenance rules. A strong Jev verdict queues review and leaves the original semantic award in place until verification changes the claim.

## Dependent Observations

- Record an exact supporting Observation revision with its owner/generation, stable ID, revision, body identity and span, alongside the ultimate source lineage used for verification. Validate the authorized current head and readable source at capture and publication. Keep cited revisions fixed. Deduplicate shared ultimate sources across derivation paths. Disallow self-citation and any cycle through Observation IDs, including through prior revisions. Keep relational Observations eligible as evidence for ordinary higher Observations and preserve the existing restriction on relational-to-relational synthesis.
- Maintain reverse links from cited Memory, Observation and participant identities to directly dependent heads. Derive the links and standing projection from committed records. The read path can check the pinned upstream chain for the candidates it is about to expose, using memoized bounded checks within the read. An unfinished check leaves that candidate out of current context until validated. The worker can resume unfinished checks across polls and reuse verified results for the same dependency cut. This closes the interval between a root becoming pending and propagation reaching every dependent. Limit reads and writes to the affected records and queue inference on the worker.
- When upstream review reaffirms the identical revision, release the inherited pending state without inference on the dependent if its other evidence is still current and it has no independent trigger. When an upstream revision changes or is withdrawn, schedule direct dependents once for that settled change, then continue through their descendants as their own reviews settle. Reverify against the successor where available and any independent support. Append a revision with newly verified citations or withdraw current standing when support cannot be established. Replacing a parent never silently replaces a child's citation. Preserve the earlier derivation in history.

For example, B relies on A's claim that an office can approve purchases. A is challenged and both leave current context while A is reviewed. If A is reaffirmed with the same proof, B can return without a model call. If A is narrowed to purchases under a limit, B must be verified again before it can return. Other evidence may still support B's original wording.

## Ownership, persistence and compatibility

| Owner | Responsibility |
| --- | --- |
| Perception semantic crate from the audit plan | Receptor and contribution policy, Jev request and reply validation, trigger filtering and shared Verify for exact drafts. |
| Application and Switchboard | Application composes authorized capture, contribution and the shared Synthesize and Verify steps. Switchboard selects the configured native route and owns transport and credentials. |
| Owner-session and Perception Store | One canonical Observation revision history for both profiles, approved evidence, minimal review and progress records, and rebuildable dependency and standing projections. Publish Observation reinforcement through the existing Freshness scoring and durability boundary. Owner-session reads exact owner cuts and commits grouped state through the native writer. |
| Gateway and Root | Gateway owns normal owner processing, grants, leases, bounded retries, route and mount fences, and retrieval eligibility at handoff. Root exposes authorized current and historical reads. |

Persist the target revision, exact changed or triggering citation references, status or successor, reason, policy and route identity, and captured input fingerprint. Coalesce equivalent pending reasons. Commit a completed review and its progress together. Before committing any model result, recheck the same Instance's grant, history generation and mount, relevant sources and dependencies, expected head, captured work and route binding. Discard obsolete output and resume on the new inputs. An unrelated write can coexist. Identical committed work reuses its receipt. Keep raw prompts, responses, private model rationale and secrets out of the owner record. [Owner publication and retry precedent](../../crates/reliquary-owner-session/src/session/identity_receipt.rs) · [Gateway lease and mount checks](../../crates/reliquary-runtime-gateway/src/owner_workers.rs)

Extend the audit's versioned Observation carrier and private checkpoint projection. Use physical-prefix validation for ordinary replay, suffix replay and historical proof. Files predating Observation publication start with an empty Observation projection and keep their REL 2.1 header, owner IDs and existing Relationship history. Reuse the audit's verified copy, reclamation and unsupported-transformation boundaries. Resume pending work from committed facts after reopen. A missing or incompatible checkpoint falls back to verified replay. [Audit publication and compatibility](memory-web-audit-spec-2026-11-24.md#publication-reconsideration-and-compatibility)

## Implementation plan

| Step | Depends on | Work and exit condition |
| --- | --- | --- |
| 1. Extend the publication foundation | Shared audit's authorized source capture, verification and Observation history | Add exact typed derived citations, durable review transitions and reverse links to the canonical writer and projection. Add cycle checks, replay/checkpoint handling and pending recovery. Exit when a mounted owner can retain exact A → B → C lineage and reopen an in-progress review. |
| 2. Wire contribution and triggered verification | Step 1, Switchboard native route and audit source readiness | Connect exact/receptor routing for new and sampled evidence, durable comparison deduplication and the tested three-choice payload through one owner worker. Persist the trigger atomically and use the tested Verify contract for reaffirmations and successors. Reuse the semantic bonus for receptor matches and publish any upgrade in the verified successor's commit. Resume queued work safely after interruption. Exit when a strong hit reopens the exact target, time/scope exclusions produce no review, and a known invalidation bypasses Jev. |
| 3. Enforce standing through reads and propagation | Steps 1–2 | Connect typed lookup/history, pre-rank current eligibility and final handoff revalidation. Follow direct dependent work as reviewed heads settle. Exit when inherited pending is visible, a reaffirmation releases it without extra calls, and a changed or withdrawn parent cannot silently support a dependent. |
| 4. Close the product boundary | Steps 1–3 | Verify the mounted workflow and retained history. Update the public contract, configured-runtime docs and affected ownership, maintenance, behavioral-contract and coverage maps alongside implementation. Run the affected tests and repository-required integration checks once. Record observed results and supported reader behavior. [Development workflow](../development/workflow.md) |

## Focused acceptance

| Seam | One focused proof |
| --- | --- |
| Jev and verifier boundary | Use a recording native decision endpoint with raw passages. Assert one request for five hits, exact frozen questions and no supplied time/scope fields. Script a later-only change, an explicit scope mismatch, and a valid first-effective challenge in one batch. Prove only eligible decisions can set pending. Cover a strong subset or exception, an unclear routing role, a weak hit and a malformed reply in this seam. Assert Verify runs only for a trigger or deterministic invalidation. Both the unchanged draft and a rewritten successor use the exact creation question template. A material back-check hit vetoes an otherwise cleared successor. Route the same sampled passage through two receptors and resample after reopen to prove a single accepted comparison. Check that pending and queued verification are independent of the shared audit threshold. |
| Freshness award and retry | Use one authored source and a match worth +6. Match it through two receptors and assert one +6 award, including a support-only case without Jev. Check that a strong Jev verdict, a reaffirmation and a withdrawal without replacement leave the original amount in place. With separate source activity for each outcome, verify +19 for a material qualification and +44 for a correction, subject to the +100 ceiling. Retry a committed successor after reopen and assert no repeat credit. |
| Mounted owner, retrieval and dependency chain | Use one authored REL with A → B → C and an unrelated Observation. Read while A is pending and verify all dependent content is history-visible but absent from new ranked context or handoff. Reaffirm the unchanged A without downstream inference. Next revise or withdraw A, keep B if separately supported, and require fresh proof for C before it returns. Include a directly corrected source, retained citations, shared-source deduplication and rejection of a cycle. Pause one attempt across a newer revision or revoked grant and prove it cannot publish. |
| Reopen and compatibility | From the same fixture, interrupt one review across commit, reopen through exact checkpoint and an Observation-bearing suffix, and retry without a duplicate successor. Check one verified copy and the earlier historical cut. Open a pre-feature REL with a manual Relationship and verify its header, owner ID and Relationship are retained. Use existing owner maintenance and inspection seams. |

Completion means the ordinary Gateway path passes the focused seams and accepts a verified prior REL alongside Observation-bearing history under the combined release plan. Keep routine tests offline with scripted decisions. The live trial evidence establishes the initial contract and does not require a provider call in CI.

## Related docs

- [Shared audit and first Observations](memory-web-audit-spec-2026-11-24.md)
- [Observation creation and verification](observation-creation-spec-2026-11-24.md)
- [Same-REL release implementation plan](perception-implementation-plan-2026-11-24.md)
- [Gateway architecture](../architecture/gateway.md)
- [Planning roadmap](roadmap.md)

## Notes

The final live trial used only the raw passages in the original six and expanded 24 authored Observation batches. One fresh request per Observation preserved all 24 triggering hits out of 150 and correctly selected all 17 reviews with zero unnecessary reviews. Four false hits were filtered by time and two by scope. This is evidence for trigger decisions on those cases. The separate Verify trial establishes the exact draft gate on authored inputs. The mounted acceptance case exercises that same gate with retained proof and successor drafts. Cross-owner or PHY propagation, automatic wall-time staleness, weak-lead escalation and clarification prompts are follow-on work.

## Tested question templates

Substitute the request-local hit ID for `{hit}`. Send all three choices for every hit in the same native decision request.

```json
{
  "{hit}": {
    "criteria": {
      "material_challenge": "The passage directly asserts a fact incompatible with the observation as stated, for the same referent, subject, scope and overlapping period. It is grounded evidence on its own that should trigger reconsideration.",
      "material_qualification": "The passage establishes a material exception, boundary, or change of scope or valid time for the same claim. The earlier claim can still have been true, but its wording or current application needs reconsideration now.",
      "no_material_effect": "The passage corroborates the claim, repeats what it already says, is incidental, or clearly concerns a different referent, scope, or period without altering this claim. No reconsideration.",
      "weak_or_uncertain": "The passage plausibly bears on the claim but a tentative proposal, indirect signal, incomplete support, or unresolved identity, scope, or timing leaves it inconclusive on its own. Keep as a lead without triggering."
    },
    "instructions": "Classify evidence hit {hit} against the existing observation in the state. Judge this hit on its own. Does what it actually asserts justify reconsidering this exact observation now? Treat a proposed or hypothetical event as such. For a claim bounded to a particular date or period, a later change alone leaves that claim unchanged; select no_material_effect. A later record can still challenge the earlier claim if it establishes what was true then, for example by saying that a rule first took effect after the claimed date. If the observation asserts an ongoing state, an established later change can materially qualify it by ending the period when it held. Use the time the state applies, rather than the date the evidence was recorded. Make one independent choice for {hit}.",
    "type": "choice"
  },
  "{hit}-time": {
    "criteria": {
      "change_to_ongoing_claim": "The observation asserts a state that continues without a stated historical end, and the passage establishes a change that ends or narrows that state.",
      "earlier_period_evidence": "The passage makes an assertion about the time covered by the observation, including a later-published correction or a report describing what was true then. An assertion may support, qualify, or contradict the observation.",
      "first_effective_after": "The observation claims a state or rule already applied during a particular period, but the passage establishes that the same state or rule first took effect after that period. A first activation or an authoritative initial effective date can establish that it did not apply earlier. A new change to a state already in effect does not qualify.",
      "later_change_only": "The observation is explicitly about a closed date or period. The passage describes a change that takes effect strictly after that period. The change can coexist with the observation being true during its period. Select this only if the text establishes the ordering and does not also say or imply that the earlier asserted state never applied.",
      "unclear_or_other": "The dates, their meaning, or their ordering are unclear, the event is merely proposed, or none of the other choices is established by what the passage says."
    },
    "instructions": "Read only evidence hit {hit} and the existing observation. Classify what the passage says about when the asserted fact applies, independently of its strength as evidence. Use the time that the state applies, not when a report was written or published. Separate a later-only replacement from a first effective date that establishes the claimed state was absent earlier. If the text is ambiguous or combines a later change with evidence about the earlier period, do not choose later_change_only.",
    "type": "choice"
  },
  "{hit}-scope": {
    "criteria": {
      "explicitly_separate_scope": "The passage explicitly limits its assertion to a different entity, group, environment, region, contract, or other boundary outside the observation's coverage. The passage gives no basis for applying that assertion to the observation's target, including by inclusion or a shared rule. Similar names, a shared provider, or a shared parent do not establish overlapping coverage.",
      "overlapping_scope": "The passage asserts something about the same target and covered scope, or about a subset or exception inside that scope. It may concern a wider population if it explicitly includes the target. A different value for the fact under comparison can be a disagreement within the same scope.",
      "unclear_scope": "The passage does not establish coverage well enough to tell whether it applies to this target. Choose this if the relation between entities or their coverage is ambiguous."
    },
    "instructions": "Compare only evidence hit {hit} and the existing observation. Identify who or what each assertion actually covers. Decide whether the passage has overlapping coverage, explicitly separate coverage, or unclear coverage. Ignore dates and the strength of the evidence for this choice. Preserve subsets, exceptions, and conflicting values within the observation's coverage. When a passage links a separate group or rule to the observation's target, consider that link. Use explicitly_separate_scope only when the text establishes the separation.",
    "type": "choice"
  }
}
```
