# Retrieval and confirmed-use acceptance

This record evaluates S05 and V04 after V07 acceptance at `1240c2ac`. V02 and S06 are already accepted. The canonical capability tracker remains the authority for completion status.

## Required behavior and verification map

| Requirement | Implementation and executable evidence |
| --- | --- |
| Hybrid lexical/exact-semantic ranking and bounded results | Retrieval's existing hybrid merge, overlap, lexical, semantic and limit tests; Application's owner-snapshot rank test covers the full population before truncation, stable identity ties and exact revision/body binding. |
| Community routing | Existing routed/global policy tests, stale-graph refusal and missing/stale-community fallback; permutation regression verifies member and vector insertion order do not change results. |
| Query candidate union and Freshness ordering | Full lexical/exact-semantic union precedes selected-ID exclusion and the ten-candidate cut. Missing Freshness is zero, not an eligibility filter. Application tests cover late high-scoring candidates and lexical-only/semantic-only candidates. |
| Turn-scoped selection | New queries clear searched IDs and preserve confirmed selected IDs. Raw matches are retained for `already_provided`, which is suppressed whenever new content is approved. |
| Refinement | Strict complete-query rewrite; all declined searched IDs are reconsidered in bounded batches without another retrieval or embedding call. Invalid replies leave the supplied turn unchanged. |
| Graph exploration | Independent concurrent incoming/outgoing one-hop pools, ten candidates per anchor, current active relations only, no recursive traversal, global content deduplication and independent ordered anchor reports. |
| Strict decisions | Versioned exact response keys, owner-qualified offered IDs only, duplicate and foreign approvals rejected; selected route failures propagate without runtime fallback. |
| Non-reinforcing internal reads | Owner snapshots, ranking, model examination, graph inspection and hydration have no accepted-use writes. Owner tests compare repeat snapshots and receipt absence. |
| Authorized external delivery | The requesting host owns an opaque local `MemoryRetrievalTurn`; Gateway has no controller registry. Prepared results bind its controller UUID, Instance, principal, owner and external turn. The registered trusted host confirms exact content consumption. Grants are checked around owner reads, decisions and host delivery. |
| Durable accepted use | Only confirmed delivery reaches the existing owner-local Freshness receipt writer. Stable identities preserve the original source IDs and captured cut across retry and reopen. |

## Publication and recovery details

Selected IDs change only on actual confirmed content consumption. They remain selected if receipt publication fails. Host-confirmed exact manifests allow a new authenticated Instance and local turn handle to recover the original source revisions and receipt after reopening the owner. Replay preserves a newer local query/search frontier and unions confirmed selections. Pending or unknown host status holds the operation; terminal rejection releases the pending proposal without credit. A durable receipt suppresses another host call. A read-only owner can prepare retrieval but cannot hand content to the external receiver without a writer.

The integration tests found that external Memory publication did not create its initial Freshness birth. Publication now writes that unadmitted birth before its completion receipt. Interrupted publication resumes the same birth; explicit completed-publication retries can repair an absent birth without changing existing `RELXMEM3` records. The activity cut is derived from the intent's authenticated physical Archive prefix. Opening and reading remain mutation-free. A pre-Dream external Memory can therefore receive confirmed use through the existing receipt writer.

## Scope

Agentic retrieval is REL-scoped. PHY content is not added to this controller. Semantic evaluators consume immutable owner-prepared values; storage remains in its existing owner packages. External host adapters must implement their negotiated idempotent handoff and exact-manifest recovery contract. Local mock-host verification does not qualify a deployed external host or establish recall quality on the retained historical corpus.

## Verification result

S05 and V04 are accepted with their prerequisites completed. The final locked offline workspace suite passed **476 tests across 47 suites**, with zero failures and local loopback enabled for mock HTTP tests. Locked metadata, workspace compilation, formatting and whitespace checks passed. The architecture guard passed with **23 production packages and 76 approved production edges**; the two added store dependencies are confined to the Gateway test fixture (13 test-only edges in total).

The Gateway tests cover exact host content, owner and controller isolation, unavailable routing, pending and forged acknowledgements, terminal rejection, read-only mounts, grant revocation after inference and after actual consumption, repeated delivery, two distinct deliveries in one turn, receipt replay without losing later selections, and full owner/Gateway reopen with a missing receipt. Owner tests cover non-reinforcing reads, tombstone/archive filtering, retired edges, historical captured-cut acceptance, durable receipt replay and pre-Dream external publication recovery. G2 remains unaccepted; V05 and V06 retain their separate outstanding requirements.
