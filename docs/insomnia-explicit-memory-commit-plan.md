# Insomnia explicit Memory commit plan

Parent roadmap: [Roadmap](roadmap.md)  
Authority baseline: [ADR 0012](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)

## Purpose

Specify the stronger explicit-retention contract without creating a second Memory authority.

## Overview

Upgrade the existing `create_memory` path into a first-class explicit retention operation.

An invocation must:

1. establish an immediate Episode boundary through the initiating turn;
2. send the resulting Episode to Insomnia at immediate-live priority;
3. process it through the normal Insomnia extraction/classification/canonicalization path; and
4. complete successfully only when at least one accepted canonical Memory retains provenance to the initiating turn.

This is a stronger processing contract, not a second Memory-writing path.

## Existing substrate

ADR 0012 already defines the important ownership boundaries:

- Archive owns deterministic Episode source truth;
- `create_memory` finalizes the current uncovered tail without closing the conversation;
- immediate-live Insomnia work outranks ordinary live and imported/backfill work;
- Insomnia is the sole authoritative generator of working Memory; and
- the live model cannot provide a Memory payload or write the Memories owner directly.

The upgrade should extend those semantics rather than introduce a sibling persistence mechanism.

## Invocation contract

The shared operation should carry, at minimum:

- conversation identity;
- the initiating Archive turn/node identity;
- the current ancestry leaf needed to finalize the uncovered tail;
- caller/origin metadata sufficient for diagnostics, such as explicit user request or model/tool invocation; and
- optional scope/routing hints only where existing owner-routing policy already supports them.

The operation should not accept proposed Memory text.

The public tool name may remain `create_memory`; a later naming cleanup may choose a more explicit product-facing label such as "remember" or "commit memory". Naming must not change the underlying authority model.

## Episode boundary semantics

Invocation is a hard boundary request.

- Finalize the current uncovered Episode tail through the initiating turn.
- Do not close the conversation.
- The next eligible response cycle belongs to the following Episode.
- Any earlier Episode emitted by the ordinary size policy retains its ordinary priority.
- The explicitly finalized tail is queued at immediate-live priority.
- Repeated invocation for an already-finalized initiating turn must be idempotent.

The initiating turn must be captured before processing begins so retries cannot drift to a later conversation leaf.

## Elevated Insomnia contract

The queued work item must carry an explicit retention requirement equivalent to:

```text
minimum_accepted_memories = 1
required_provenance_node = initiating_turn_id
```

The requirement changes acceptance, not semantic authorship.

Insomnia remains free to:

- extract one or more Memories from the Episode;
- phrase the retained Memory more broadly or canonically than the literal initiating turn;
- merge with, supersede, or materially revise an existing Memory when normal Memory semantics require it; and
- emit additional ordinary Memories from the Episode.

A successful run must leave at least one accepted canonical Memory whose provenance includes the initiating turn.

A duplicate/supersession path satisfies the contract only if the surviving canonical Memory is durably updated or linked so that provenance to the initiating turn is retained. "Candidate was generated and then discarded" is not success.

## Provenance

The initiating turn is the mandatory provenance anchor for the explicit-retention guarantee.

Existing ADR 0012 provenance roles remain intact:

- authoritative user turn;
- optional assistant authority source explicitly adopted by the user; and
- optional grounding source used only to resolve a referent.

The elevated contract must not convert grounding into authority or force unsupported details into Memory.

When a model invokes the tool in response to explicit user retention language, the user's initiating turn should remain the required provenance anchor. The tool-call turn is operational metadata, not a substitute semantic source.

For future autonomous model invocation, the caller must supply the exact conversation turn whose proposition is being committed; vague "something earlier was important" invocation is not sufficient.

## Salience rule

Explicit invocation is itself an additional salience signal.

Normal Insomnia may validly conclude that an Episode yields no durable Memory. Explicit retention removes that zero-Memory outcome for the anchored request: at least one grounded Memory must survive publication.

This does not permit arbitrary fabrication. If the initiating turn is referential, Insomnia may use the existing bounded Archive evidence expansion needed to resolve the referent, while keeping the initiating turn in provenance.

## Completion and failure semantics

The explicit-retention job is not complete merely because a model call returned.

Completion requires the normal atomic Insomnia publication transaction plus verification that the retained canonical result satisfies:

- accepted Memory count >= 1; and
- initiating-turn provenance is present on at least one surviving canonical Memory.

Malformed structured output, zero candidates, all-candidate rejection, provenance loss during dedupe/supersession, or publication failure leaves the job incomplete and eligible for normal retry/fallback handling.

Retries must remain idempotent by Episode/work identity and must not create duplicate semantic state.

No partial Memory publication should become visible outside the existing atomic completion semantics.

## User/model invocation policy

One underlying operation should serve both surfaces.

User-originated invocation is appropriate for explicit retention intent such as "remember this", "save that", or equivalent host UI action.

Model invocation should be available through the shared runtime tool adapter when the model is acting on explicit retention intent or another deliberate host/system checkpoint.

Do not make ordinary model-perceived importance sufficient for automatic invocation by default. That would create excessive Episode fragmentation and bypass Insomnia's normal probabilistic salience policy.

Any later autonomous policy requires separate measurement and an explicit trigger contract.

## Result surface

The runtime should expose enough result state for the caller/host to distinguish:

- Episode finalized and queued;
- processing completed with one or more satisfying canonical Memories; and
- processing failed/retryable.

The caller does not need direct mutation authority over the resulting Memories.

Whether the live tool call blocks until Insomnia completion or returns a durable job/result handle is a runtime integration decision. In either case, the semantic success condition above is mandatory.

## Implementation sequence

1. Extend immediate-live Insomnia work metadata with the required initiating provenance node and minimum accepted-Memory count.
2. Thread that contract through `request_create_memory`, queue persistence/reclaim, extraction, canonicalization, and completion verification.
3. Make canonical merge/supersession retain the required initiating provenance when it satisfies the explicit request.
4. Add completion validation so zero-memory/provenance-missing outcomes remain retryable failures.
5. Expose the narrow operation through the shared live runtime/tool adapter without accepting Memory payloads.
6. Add user-intent/model-tool routing around the same operation.
7. Add diagnostics/result reporting for queued, satisfied, retryable-failure, and terminal-failure states.
8. Validate crash/reopen/idempotency behavior and the ordinary non-explicit Insomnia path.

## Acceptance tests

At minimum:

- explicit request closes only the uncovered tail through the initiating turn;
- conversation remains open and subsequent turns begin the next Episode;
- work is queued at immediate-live priority;
- successful completion publishes at least one canonical Memory;
- at least one surviving canonical Memory cites the initiating turn;
- referential retention may use bounded grounding without losing the initiating provenance anchor;
- dedupe into an existing Memory still preserves the initiating provenance;
- zero-candidate and all-rejected outputs do not falsely mark the Episode complete;
- malformed/failed attempts retry without duplicate Memory publication;
- crash before atomic completion exposes no partial success;
- repeated invocation for the same finalized request is idempotent;
- ordinary Episodes remain allowed to yield zero Memories; and
- the runtime/tool surface cannot inject caller-authored Memory bodies.

## Related docs

- [Roadmap](roadmap.md)
- [ADR 0012: deterministic Episodes and Insomnia Memory authority](decisions/0012-deterministic-episodes-and-insomnia-memory-authority.md)
- [Multiplexing implementation plan](multiplexing-implementation-plan.md) — shared runtime integration must preserve explicit principal routing.

## Notes

This plan is future-only. The existing `create_memory` operation and Insomnia authority are the implementation baseline; the stronger acceptance/completion guarantee is not claimed as shipped.

## Non-goals

This upgrade does not:

- create a direct user/model Memory-write API;
- replace deterministic Episode formation;
- make all user turns mandatory Memories;
- make every model-identified important statement an automatic checkpoint;
- weaken normal Insomnia provenance, routing, dedupe, supersession, or atomic-publication rules; or
- introduce a second semantic authority beside Insomnia.
