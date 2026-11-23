# User confirmation of separate cross-REL Entities

Parent index: [Planning](./INDEX.md)

## Purpose

Proposed follow-up. A user reviewing two selected Entities in different RELs can record “These are separate Entities” and later undo that decision.

## Overview

Automatic comparisons are revisable judgments based on evidence. An explicit confirmation records who made the decision and exactly which pair they reviewed. [Automatic comparison contract](cross-rel-entity-identity-spec-2026-11-23.md)

## User-visible behavior

- Start with an authorized comparison of A's Entity and B's Entity. The host shows both owner-qualified endpoints, the current evaluator result and the recording REL. A user can confirm separation even when the evaluator is unresolved or suggests a match. Show a suggested match as a disagreement before the user commits. A stores the confirmation for this directional pair. Both Entities retain their original IDs and graph edges.
- Authorized inspection shows the evaluated result and its freshness alongside the confirmation and its status. While a confirmation applies, consumers of this exact pair use “confirmed separate.” Inspection also shows any conflicting automatic judgment. Evidence, policy and route changes can revise the evaluator judgment. The confirmation continues to apply to those same owner-qualified Entities through ordinary edits, including a new alias, description or software version. Retirement makes the pair unavailable. A replacement REL must establish continuity with the reviewed Entities before honoring the confirmation. If continuity cannot be verified, show the recorded decision as historical and ask for a fresh review.
- Undo withdraws a specific confirmation by appending a new record. The effective answer then comes from the latest current evaluator result, or is pending reconsideration if the evidence is stale. Undo can withdraw a stale confirmation while both RELs are accessible, even if an endpoint has since retired. A designated operator with the required grants can undo another operator's confirmation. Inspection preserves the original actor and the actor who withdrew it.

## Host authority and publication

Today, Core's trusted host binds credentials to a principal and owner grants. The same authenticated Instance that can compare with `ManageRel(A)` and read access to B can also run comparison workers. Neither its grant nor its principal string establishes that a person made a decision. A Memory, copied conversation, evaluator response, or model-supplied flag cannot authorize confirmation or undo. [Host Instance checks](../../src/host_instance.rs) · [Comparison and worker entry points](../../src/management/entity_identity.rs)

The trusted host exposes confirmation and undo through a dedicated operator interaction. Give the host a separate opaque approval capability installed through trusted host setup and withheld from model tools, ordinary request handlers, and workers. The host binds it to an authenticated operator session with `ManageRel(A)` and independent read access to B. After showing the pair and intended action, it invokes the capability only in response to a deliberate user action in that session. Keep the capability and the server-side approval request outside caller-controlled tool arguments. The host supplies an authenticated, stable, nonsecret operator ID for audit. Core verifies the host capability and owner grants. The host is responsible for proving the user action. Core acceptance can use a trusted test host. Hermes stays on the automatic path until its host independently demonstrates this interaction and keeps the capability outside its model and worker surfaces.

Prepare an opaque, short-lived action bound to the operator session, recording owner, directional pair, both mount identities and expected confirmation head. For confirm, bind the current comparison receipt revision and fingerprint displayed to the operator. For undo, show and bind the exact confirmation to withdraw. Recheck both grants under the credential publication fence before any disclosure or append. Use Gateway's pinned topology and ordered pair guards to revalidate mounts and expected state. A relevant change, remount, stale preview, or revocation before commit requires a fresh authorized review. Validate the comparison basis without calling the evaluator at commit. Unrelated writes do not invalidate the preview. [Gateway pair guards](../../crates/reliquary-runtime-gateway/src/instance_identity/locked_pair.rs) · [Comparable cut-bound management operations](../public-product-api.md)

## Durable state and retry

Store a typed confirmation event in A's existing REL publication envelope. Keep the evaluator receipt chain independent. Each event records the directional pair, action, monotonic revision and predecessor, issued action ID, authenticated operator ID, and the reviewed comparison revision for confirm or target confirmation revision for undo. A confirm also records both endpoint revisions and a canonical digest of each endpoint snapshot, covering the content used to identify that Entity. No prose, quotes, prompts or credentials enter the event. At most one confirmation head can apply. A newer confirmation supersedes an older one and undo never revives a superseded event. On remount, verify the same owner history and the reviewed Entity revisions in the endpoints' retained history. This preserves the confirmation through later revisions and prevents it from transferring to unrelated histories that reuse an ID. Mount identities constrain the pending action only.

Scope idempotency to the host-issued action ID and operator in A. Check authorization before handling a retry. An exact retry returns the original durable event, identified as historical if the head has advanced. A changed action with the same ID or a distinct action with an obsolete head fails. Concurrent mutations leave a single successor. Acknowledgement follows durable append. Recovery after a lost response can read the event by action ID from the authorized host path. An expired uncommitted action requires a fresh review.

Apply both current read grants to hosted inspection of confirmations, their history, actor IDs and retry results. A missing grant takes precedence over pair state, including stale and undone status. Keep these records out of single-owner hosted exports and diagnostics. As with comparison receipts, access to A's raw file exposes its retained references and audit metadata. [Receipt storage and privacy](cross-rel-entity-identity-spec-2026-11-23.md#receipt-and-reconsideration)

## Implementation sequence

1. Add the typed event and confirmation projection to REL owner-session. Reuse the publication envelope and check the owner, local provenance, referenced revision, chain and action identity at replay. Validate peer references structurally so A reopens without B. Existing 2.1 histories start with no confirmations. Include the projection in checkpoint, historical proof and verified maintenance paths. Reject fresh-history rewrites or divergent reconciliation that cannot retain it before creating a destination.
2. Route prepare, confirm and undo through the host-only Root capability and Gateway guards. Extend authorized pair inspection with distinct evaluator and confirmation fields and effective status. Keep evaluator and worker writes on their existing receipt path. Reuse the existing ordered pair fixture for tests. Update the public API, architecture and behavioral contract documentation once implemented.
3. Verify the trusted host operator interaction with the intended production host before enabling there.

## Focused acceptance

| Case | Required result |
| --- | --- |
| Explicit decision and undo | A designated operator confirms a pair after reviewing its result. Worker reevaluation can disagree but cannot override the confirmation. After undo, inspection uses the current evaluator result or reports pending reconsideration, and both audit events remain after reopening A. |
| Authority and privacy | Compare and worker calls, model tool arguments and written Memories cannot confirm or undo, even with both owner grants. Missing peer read, missing local management, revocation during preview, or a read-only A denies publication. A read-only B suffices. Unauthorized inspection and retry disclose no pair or operator metadata. B's bytes stay unchanged. |
| Revisions and race | A changed receipt, replaced mount or intervening confirmation rejects a pending action. A concurrent retry gets one durable event. Reused action IDs with different inputs and undo against a superseded head fail. An unrelated write permits commit. A committed confirmation survives an ordinary Entity revision and a remount with verified continuity. Retirement or a replacement lacking the reviewed history makes it unavailable for a current decision. Authoritative undo still works for a stale confirmation with both owners accessible. |
| Reopen and maintenance | Old 2.1 files open with an empty confirmation history. Cold replay, checkpoint and suffix replay, and verified copies preserve the chain without B. A partial tail recovers the complete prefix. Unsupported rewrite and divergent reconciliation fail before creating a destination. |

Run the focused cases through the existing [Root pair fixture](../../tests/entity_identity.rs) and [owner-session receipt tests](../../crates/reliquary-owner-session/src/session/identity_receipt/tests.rs), plus the locked offline integration and documentation checks.

## Related docs

- [Development workflow](../development/workflow.md)
- [Cross-REL Entity identity implementation plan](cross-rel-entity-identity-plan-2026-11-23.md)

## Notes

Candidate discovery, a “same Entity” confirmation, and reciprocal or transitive assertions are outside this proposal.
