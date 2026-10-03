# Phase 0B G2 — principal attribution and personal publication

Parent index: [Documentation index](INDEX.md)
Related: [Multiplexing implementation plan](multiplexing-implementation-plan.md), [Readiness review](multiplexing-readiness-review.md), [Phase 0 baseline](multiplexing-phase0-baseline.md), [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)

## Purpose

Resolve the Phase 0B G2 ownership question: who authored or initiated source material, which principal a personal synthesis may belong to, and how partial writes across a REL and several PHYs recover without retargeting.

## Overview

A shared REL can contain turns from multiple principals. A personal classification alone does not identify a destination PHY. The proposed contract resolves destinations only from authenticated source attribution and exact provenance, then freezes the full synthesis and route plan in a gateway-owned per-REL operational journal before any PHY write.

Status: design draft for integration into the multiplexing Phase 0B package. It resolves the G2 design contract at reference-model level. It does **not** claim production behavior, production API compatibility, or implementation readiness by itself.

## Source evidence and current boundary

The current host is single-PHY, so the issue is a migration hazard rather than a demonstrated reachable cross-user leak today. Source inspection establishes these constraints:

| Evidence | Current behavior and implication |
| --- | --- |
| `src/runtime_inference/runtime_host_interaction.rs::begin_message` | Uses the host's sole attached PHY principal. Instance attribution must replace that ambient source. |
| `src/archive/interaction_stream.rs::begin_message_with_principal` | The existing implementation may take `principal_id` from the parent node for an Agent turn. Parent ancestry is not a reliable identity for the instance that initiated a later generation in a shared conversation. |
| `src/archive/interaction_stream_record.rs::InteractionStreamRecord`; `src/archive/interaction_stream_codec.rs` (`CVAISTR1`); `src/archive/interaction_stream_persistence.rs::stream_record` | Durable streaming checkpoints have no `principal_id`; the transcript projection recovers it only from matching live `SessionState.in_flight`. It is unavailable after restart. |
| `src/runtime_inference/insomnia/ownership.rs` | Insomnia classifies only `User` versus `Project`. “User” says a fact is personal, but does not name which principal owns it. The prompt chooses Project for ambiguity. |
| `src/runtime_inference/insomnia/processor/application.rs::prepare_application` | For a User draft, `MemorySourceRef.principal_id` is derived from the turn whose ID equals the draft's single `source_node_id`. Authority and grounding source references can differ. No unanimous multi-source principal check exists here. |
| `src/runtime_inference/runtime_host_insomnia.rs::process_claim` | Every prepared User draft is published through the currently attached/active PHY. Worker code has no per-candidate principal-to-owner resolution. |
| `src/runtime_inference/insomnia/processor.rs::process_claimed_insomnia_episode_routed` and `processor/application.rs::publish_user_application` | PHY writes occur before the REL completion receipt. `mutation_id` and semantic/source-ref validation make a same-destination retry idempotent; a PHY failure can leave partial earlier PHY writes. Current host locking supplies one target, not a multi-principal recovery plan. |
| `src/runtime_inference/insomnia/processor/application.rs::commit_application`; `src/runtime_inference/insomnia/completion.rs` | The REL completion records owner-qualified external `MemoryRef`s only after PHY publication. A restart-safe multi-PHY route needs a durable REL intent before the first external write. |
| `src/runtime_inference/insomnia/ownership_tests.rs::routed_user_memory_uses_owner_qualified_receipt_and_no_rel_provenance` and `routed_retry_reuses_phy_memory_after_wording_drift` | Existing tests establish owner-qualified references, identifier-only cross-owner provenance, and retry behavior when the target PHY is stable. They do not test mixed principals, route ambiguity, partial writes across PHYs, or restart recovery. |
| `docs/architecture.md` and `docs/storage-format.md` | REL owns Episodes and Insomnia completion/operational state; PHY owns personal Memories. A PHY `MemorySourceRef` may identify a source REL and principal without copying REL provenance records into the PHY. |
| `docs/invariants.md` | One mutable domain has one owner; stable IDs cross databases; runtime work state is not semantic authority. Cross-owner publication therefore needs deterministic identities and a REL-side receipt/intent, not an implicit active-PHY pointer. |

The current retry key is derived from Episode ID plus candidate key. Current same-target retries check semantic equivalence and source-ref equality before accepting an existing Memory. Those are useful building blocks, but they cannot make an unknown or changing destination safe. The proposed per-REL gateway sidecar freezes the complete resolved Episode plan before any cross-owner side effect. If that sidecar is unavailable, the Archive checkpoint remains readable but operation attribution is `Unknown`; there is no personal-publication fallback.

## Terms and attribution fields

Persist separate facts rather than overloading `principal_id`:

- **`actor_principal`**: the authenticated principal that authored a User turn. It comes from Gateway-verified instance context, never from a request field or currently selected PHY. For an Agent turn, the actor is the service/agent; it has no user `actor_principal`.
- **`generation_initiator`**: the verified principal who authorized a specific generation operation and whose instance context was captured for it. It is immutable for the operation. It is not inherited from the parent node. Shared work lacking one initiating principal must explicitly record a service authorization class plus its authorized principal set; it cannot silently choose one.
- **`generation_id`**: stable operation identity linking an Agent turn/checkpoint to the generation that produced it.
- **`source_principal`**: a resolved provenance fact for a source node. For a User node it is `actor_principal`; for an Agent node it is the associated generation's `generation_initiator`; for legacy or system material without sufficient durable metadata it is unknown.
- **`destination_phy_owner_id`**: resolved only from a trusted Gateway mapping for a specific source principal. The Insomnia classifier cannot invent it, and worker scheduling/ambient active PHY cannot supply it.

The legacy Archive `principal_id` field keeps its existing compatibility meaning and bytes. Durable generation attribution is stored in the concrete gateway owner-operations journal below, not a new Archive/REL record. It is never backfilled by assuming an Agent answer belongs to its parent node's principal. Legacy records without a journal attribution entry remain readable; if they cannot prove a personal destination, treat them as `Unknown` and do not publish a user-owned Memory.

## Contract

### Capture and durable operation attribution

1. On User submission, Gateway authenticates the attached instance and stamps `actor_principal`; callers cannot override it.
2. On generation admission, Gateway appends and syncs an `Attribution` frame before dispatch or checkpoint acknowledgment. It binds the stable `generation_id` to REL UUID, conversation/session/message IDs, source actor class, `generation_initiator`, and authorization grant generation. Every checkpoint resolves through this immutable mapping.
3. A late joiner/restart resolves attribution from the gateway owner-operations journal. If the journal is missing, corrupt, or lacks an entry, attribution is `Unknown`; the Archive checkpoint remains readable but no personal Memory may be routed from it.
4. Before dispatch and each final side effect, the gateway validates the operation's captured grant generation. Revocation closes new leases/intents, drains only short already-admitted owner-stage leases, and holds intents that fail revalidation. A held intent remains bound to its exact principal and PHY; retry requires explicit authorization/repair and cannot retarget.
5. Existing `principal_id`, `CVAISTR1`, and other REL records remain byte-compatible and retain their existing meaning. No Archive record version or REL storage-format migration is introduced for G2 attribution.

### Episode provenance to destination

For each extracted candidate, build a provenance set from every node reference that materially supports the proposition: primary source, content-authority source, and grounding source (including any additional evidence refs added by the extractor contract). Validate each reference against the REL Archive and the Episode. Resolve each to a `source_principal` using the durable turn/generation rule above. A generation initiator by itself is never evidence that a personal proposition is true or identifies its subject: a personal candidate must include at least one principal-authored User source node whose content supports the proposition. Agent-only, service-only, or unattributed provenance is rejected. Agent nodes may corroborate only when their operation attribution is durable and consistent with the principal-authored sources; conflicting initiator/source principals fail closed.

- A **personal/User** candidate is publishable only when all personal-bearing provenance resolves to exactly one principal, that principal is authorized for personal publication, and the principal maps to exactly one mounted/available PHY owner whose durable principal identity matches. The source principal is the destination principal. No current selection, worker-local slot, request-supplied owner ID, or “first mounted PHY” can influence it.
- If provenance is missing, legacy-unknown, contradictory, or identifies two principals, the candidate is **quarantined/rejected** with a durable reason. It is not sent to any PHY. It must not be converted to a REL/project Memory as a fallback: that can expose one user's private fact to other REL readers. An explicitly nonpersonal shared/project proposition may be REL-owned only if its evidence and classification independently support that scope.
- A Project/REL-owned candidate is published only to the REL and has no PHY destination. An owner classifier's `User` label is not authorization and is not sufficient routing data.
- If a personal statement is sourced from an Agent turn, use the generation initiator only when the durable operation record exists and the candidate's provenance establishes that the personal assertion is attributable to that generation. A generated mention of another person's facts cannot be stored in the initiator's PHY merely because the initiator requested the generation.
- Shared-PHY instances remain one PHY execution and one write owner. Distinct principal IDs must never alias the same PHY identity. A principal-to-PHY mapping change is rejected while route intents are pending; it is not a retry-time retarget.

### Durable cross-owner route intent, receipt, and retry

Cross-file atomicity is not available. Make publication recoverable and deterministic with a narrow gateway-owned journal scoped to each REL rather than claiming a distributed transaction:

1. **Prepare:** after provenance and authorization validation, derive stable mutation IDs and the exact complete Episode plan: every REL and personal draft payload, classification/rejection result, source/provenance fingerprint, destination principal and PHY owner, and expected semantic receipt ID. Persist this in `<state-dir>/owner-operations/<rel-uuid>.roj`, alongside the G1 conversation queue journal but outside Archive and REL formats. The file starts with magic `RQOP0001`, version 1, and the exact durable REL UUID. Append-only frames contain a monotonically increasing sequence, bounded payload length, and SHA-256 checksum. Frame kinds are `Attribution`, `EpisodePlan`, `EpisodeReceiptObserved`, and `Held`. Sync `EpisodePlan` before any PHY write. This narrow owner journal is operational recovery state, not a Memory or Episode and does not advance REL activity.
2. **Publish:** write each planned Memory to its exact PHY using the stable mutation ID and principal-qualified source reference. An authorization lease/fence is validated at intent creation and every owner-stage write honors its captured grant generation. Revocation closes new leases/intents and drains only short already-admitted owner-stage leases. Existing matching content is success. A same-ID record with different semantics or source ref is a conflict; stop and retain the intent for repair. A failed/unavailable target remains pending. Never publish to a different PHY to “make progress.”
3. **Receipt:** after verifying every planned PHY write, atomically commit the REL's ordinary project Memory batch and Insomnia completion containing owner-qualified `MemoryRef`s for all personal results. This REL completion is the semantic receipt. Then append/sync `EpisodeReceiptObserved` only after reading back and verifying that completion. The operational marker is not a receipt and is not in the REL transaction; cross-file atomicity is not claimed.
4. **Recovery:** on restart/retry, load the complete frozen plan before extraction, classification, wording, or routing models can run. Never redraft or recompute destinations for an existing plan. Reconcile every target by exact owner ID and mutation ID; verify existing records; repeat missing writes idempotently; then commit/read back the REL receipt. If a target is offline or its grant revalidation fails, append/sync `Held` and expose pending status. A held plan remains bound to its exact principal and PHY; resumption requires explicit authorization/repair and cannot retarget. If the state directory or owner journal is missing, operation attribution is `Unknown`, personal publication is disabled, and any partial attempt lacking a recoverable plan is held for explicit repair. Never infer a replacement plan from a fresh model run.
5. **Terminal rules:** do not acknowledge Insomnia completion or schedule downstream processing until the REL receipt is durable and verified. Replaying a completed Episode returns that existing REL receipt without rerunning models. Duplicate workers are fenced by the Insomnia claim token plus stable owner-journal plan identity. Keep plan/receipt-observed frames until verification; compact only through G1's atomic replacement rules. A truncated final frame follows G1's tail repair rule; interior sequence/checksum corruption fails closed.

A sidecar plan that has external writes but no REL completion is a recoverable partial, not success. The REL completion must never reference a write that was not verified. The PHY stores identifier-only provenance as today; no REL-private body or Episode is copied into it.

### Compatibility contract

- Preserve typed REL/PHY files, owner UUIDs, existing Memory/MemorySourceRef semantics, source turn identity, Archive ancestry, Episode identity, and completed Insomnia receipt semantics.
- Existing `CVAINSC*` completion payloads and `CVAISTR1` stream records remain byte-compatible and unchanged. Store new attribution/recovery frames only in the gateway owner-operations sidecar; do not change prior REL bytes or meanings. Back up/move the configured state directory with the REL runtime if continuity is required; state-directory loss leaves attribution Unknown and partial intents held for explicit repair.
- Legacy turns with `principal_id` can still be read, but parent-principal propagation alone is not accepted as proof of generation initiation. Legacy source data lacking sufficient proof can continue REL-local processing where safe; it cannot create a personal PHY Memory.
- Current single-principal routed Insomnia remains behaviorally equivalent: one unambiguous principal routes to that principal's authorized PHY and emits the same external owner-qualified receipt.
- Owner-missing, ambiguous, and retry failures are explicit status/rejection outcomes. No fallback to whichever PHY happens to be attached.

## Adversarial oracle

`multiplexing_phase0b_personal_contract.rs` is a std-only executable reference model, not production code. It checks:

1. Agent turn attribution uses the immutable generation initiator, separately from the actor field; the model does not inherit the source principal from parent ancestry.
2. In one shared Episode, clear personal facts for A and B route to their respective PHYs while the current/selected PHY is set to the opposite principal; REL-only facts remain REL-only.
3. A personal candidate with A+B provenance, missing/legacy attribution, or mismatched principal-to-PHY identity is quarantined and creates no PHY record.
4. A crash after one of multiple planned PHY writes in one mixed-principal Episode leaves the complete durable route intent set and no completion receipt; restart/retry finishes at the same destinations with one Memory per stable mutation ID and one receipt.
5. Changing the active PHY or remapping a principal while an intent is pending cannot retarget the write; conflicts stop fail-closed.
6. Shared instances attached to one PHY do not create duplicated owner writes; same mutation retries verify semantics and source reference.

Passing this fixture supports only the consistency of the stated reference model. G2 passes only after the actual Archive format, Insomnia receipt protocol, host workers, authorization/revocation, and integration tests implement and verify the same outcomes.

## Related docs

- [Multiplexing implementation plan](multiplexing-implementation-plan.md)
- [Multiplexing readiness review](multiplexing-readiness-review.md)
- [Multiplexing Phase 0 baseline](multiplexing-phase0-baseline.md)
- [ADR 0038](decisions/0038-gateway-instance-runtime-multiplexing.md)
- [Current architecture](architecture.md)
- [Storage format](storage-format.md)

## G2 exit criteria

- Persisted user actor and generation initiator are separately represented; live checkpoint, completion, late join and restart round-trip preserve both.
- Two-principal shared-Episode test asserts principal-separated Memory IDs, provenance, and receipts; no cross-principal PHY result is observable.
- Ambiguous/legacy attribution creates no personal write and no REL fallback leakage.
- Crash injection covers intent durability, each PHY write, REL completion, and post-completion cleanup; all replays are idempotent and never retarget.
- Same-destination drift and cross-destination conflict tests are explicit; revoked/unavailable targets remain held and visible.
- Existing single-principal routing and on-disk compatibility tests pass.
- G2 source inventory and compatibility decision are checked against the actual current branch before Phase 1 implementation; this draft does not prove the other Phase 0B gates or authorize the hard cut.

## Notes

This is a design proposal backed by current-source audit and a standalone executable reference model. The fixture validates only the proposal's state-transition logic. It is not a production behavior proof, does not resolve G1/G3-G6, and does not authorize a Phase 1 hard cut.
