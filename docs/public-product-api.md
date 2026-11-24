# Public product API

Parent index: [Documentation](./INDEX.md)

## Purpose

This guide describes the supported Rust consumer surface exported by the `reliquary-memory` package and the security boundary around it.

## Overview

Root exports opaque owner handles, product workflows, immutable read values and trusted-host interfaces. It does not export mutable Container or store handles. The executable CLI is a separate crate and depends on Root, rather than reaching into implementation crates.

The current owner formats are REL 2.1 and PHY 2.1. `Rel` and `Phylactery` expose identity, path and `OwnerInfo` for current owners. `OwnerId` is a 16-byte identity displayed as 32 hexadecimal characters.

`Rel::create(path)` requires a new path and allocates a fresh owner ID. The new header uses `Unspecified`. Opening or verifying an existing 2.1 REL reports its stored `Unspecified`, `Organization`, `Project`, or `Connection` value and original owner ID. Hosted authority comes from the authenticated principal's owner grants.

`Gateway::mount_owner_path` admits the selected current owner once and returns its ID. Supply the previously reviewed or registered ID when it is known. Core checks it and the requested file kind on the same leased handle before registering the mount. A detached owner snapshot and a subsequent trusted mount remain independent.

`Rel::read(path, operation)` lends the callback an opaque `RelRead` for metadata, conversation listings, and Memory history. Core holds a shared lease through the callback and can use a verified checkpoint to serve historical payloads on demand. `Rel::open` provides an owned, detached snapshot. `Rel::verify` performs a complete audit.

A trusted host can release a read-only PHY mount through `HostAdmissionControl::detach_read_only_phylactery(owner)` after draining its requests. The operation leaves other mounts available. Host credential revocation and owner selection remain explicit lifecycle steps.

## Getting started

Prerequisites: depend on the workspace's `reliquary-memory` package and create a path for a new owner. In a trusted host, create and mount the owner, install a credential binding with only the required grants, then authenticate an `Instance`:

```rust
use reliquary_memory::{
    Gateway, HostCredentialBinding, OwnerGrant, ProductError, Rel, TrustedHostAdmission,
};

fn trusted_host_example(token: String) -> Result<(), ProductError> {
    let rel = Rel::create("project.rel")?;
    let gateway = Gateway::new();
    let owner = gateway.mount_rel(&rel, true)?;
    let admission = TrustedHostAdmission::new([
        HostCredentialBinding::new("operator", token.clone(), [OwnerGrant::ManageRel(owner)]),
    ]);
    let (requests, _revocation) = admission.install(&gateway)?;
    let instance = requests.authenticate(&token)?;
    let management = instance.workspace_management(owner)?;
    let _health = management.health()?;
    Ok(())
}
```

The host passes a credential loaded from its protected configuration as `token`; the example takes it as a function argument so the snippet remains compilable without inventing a host secret-store API. Do not pass the token as a CLI argument, serialize it into requests, or log it. Grants are bound to owner IDs and REL/PHY kinds. `ManageRel` is the management grant; `OperateRel` and `ViewRel` are narrower operation scopes. PHY supports view and operate grants, not REL management. Read-only mounts cannot be used for writes. Revocation fences authorized operations, including publication boundaries.

## API families

| Family | Root entry points | Contract |
| --- | --- | --- |
| Owners | `Rel`, `Phylactery`, `OwnerInfo`, `OwnerId`, `OwnerKind`, `OwnerScope` | Create/open current files, obtain stable owner identity and inspect verified metadata. Wrong-format input fails admission. |
| Admission and lifecycle | `Gateway`, `TrustedHostAdmission`, `HostCredentialBinding`, `OwnerGrant`, `Instance` | Mount owner files, authenticate a host principal, issue scoped capabilities, revoke credentials, and quiesce/reopen through product lifecycle operations. |
| Import and processing | `NativeImportPlan`, `ProcessOptions`, `OwnerProcessingJob`, `Instance::stage_native_import`, `Instance::start_processing`, `Instance::process_and_drain` | Stage source data and run named processing through configured routes. Durable progress and source identity are queryable; replay and resume honor recorded workflow evidence. |
| Inspection and management | `InspectionQuery`, `InspectionCursor`, `WorkspaceManagement`, `MemorySummary`, `MemoryProvenance` | Bounded, cut-bound owner reads and supported user-origin mutations; stale cuts and unauthorized owners are explicit errors. |
| Cross-REL Entity identity | `WorkspaceManagement::compare_entity_identity`, `inspect_entity_identity` | Compare one explicit owner-qualified pair and inspect its receipt. Core automatically revisits recorded pairs when their evidence changes. Publication requires `ManageRel` on the recording REL and read access to the peer. Inspection requires both read grants. |
| Retrieval | `Instance::has_active_vectors`, `search_archive_vectors`, `search_archive`, `search_archive_hybrid`, `search`, `begin_retrieval`, `memory_query`, `refine_query`, `traverse_graph`, and prepared delivery/recovery methods | Authorized read-only vector presence for host metadata binding; passive search, decision-driven retrieval, inspection and confirmed host use remain distinct. A query or displayed result does not assert use. |
| Host work | `ConversationQueue`, `ContextView`, `AuthorizedContextCheckpoint`, `ExternalHostSource`, `TrustedProjectSource`, `TrustedHostAdapter` | Host-facing queue, context, source and exact handoff protocols, with durable evidence and explicit recovery. |
| Maintenance | `plan_rewrite`, `execute_rewrite`, prune/reclaim/repair/reconcile operations | Plan and verify create-only copies; they do not replace an existing destination. Quiesce writable owners before copy maintenance. |
| Configuration | `ReliquaryConfig`, `ConfiguredRuntime`, `ModelRoute`, `CredentialId`, `SecretString` | Open or prepare `reliquary.cfg`, validate offline route bindings, protect credentials through Switchboard. |

This table is an orientation map, not a complete list of every public value. Rustdoc for the built version is the exact method signature reference; CLI callers use the [CLI reference](reference/cli.md).

## Vector and hybrid Archive search

`Instance::search_archive_vectors(owner, profile_id, query, limit)` searches the accepted Archive generation for a compatibility profile in a mounted REL. It requires REL view access. Pass the embedding's `CompatibilityProfileId` and a query with the same dimensions, finite values, and nonzero norm. The limit accepts 1–128 hits. An absent active generation returns `Unavailable`.

`ArchiveVectorSearch` returns the profile, generation, and source Archive version. Each hit has a stable fragment ID, its row ordinal in the generation, and a cosine score. Hits are sorted by descending score and ascending ordinal for ties. Scoring reads the accepted packed vectors and makes no provider calls or durable writes.

`Instance::search_archive(owner, text, options, runtime)` runs hybrid search using a configured query embedding compatible with an active Archive generation. `Instance::search_archive_hybrid(owner, profile, text, embedding, limit)` accepts a qualified embedding from the caller. Both require REL view access and apply the existing hybrid weights to fragment text and exact cosine scores before applying the result limit. Results include the generation and source cut, the number of fragments with readable text, and each hit's conversation and node range. Fragments with text beyond the evidence bounds or unavailable content can still match their accepted vectors. A matching persisted lexical index is reused after reopen. A mounted generation with no valid record builds the shared index on its first query with lexical terms.

`Instance::publish_archive_lexical_indexes(owner)` requires `OperateRel` and a writable mount. It prepares and publishes one sealed index per active Archive generation. The result reports profile, generation, and added REL bytes per index. Each index commits independently, and retrying a committed generation reports zero added bytes.

`SearchMode::Semantic` for passive Memory search uses global exact ranking within the active embedding profile. The result reports `semantic_routing = "global_exact"`.

## Temporal search

Pass a checked `TemporalFilter` with the `Source` or `Mentioned` basis. Use `TemporalFilter::new` for signed Unix nanoseconds or `TemporalFilter::from_rfc3339` for offset-qualified bounds. The end is exclusive. Resolve relative query wording in the caller's calendar zone and pass the resolved bounds with the topical query.

- `Instance::search_temporal` supports lexical, hybrid and global exact-semantic Memory search. It filters accepted Memory source timestamps or grounded Chronos evidence before ranking and limiting, then returns the ordinary search result, applied window and matching evidence for each hit. Lexical search needs REL view access and runs locally.
- `Instance::memory_query_temporal` starts an agentic query with a date window. Refinement, traversal and prepared delivery inherit that window. Start a new query to change or clear it. Inspection and previews are read-only, and approved consumption follows the existing confirmed-use protocol.
- `Instance::search_archive_temporal` uses a compatible query embedding. `Instance::search_archive_hybrid_temporal` accepts a qualified embedding from the caller. Both project dated evidence from turns on the accepted fragment path. Mixed fragments rank by eligible-turn lexical evidence, and their remaining turns are context.

Source time refers to the accepted source timestamp. Mentioned time refers to the phrase and precision resolved against the original body and source time. [Temporal search specification](planning/temporal-search-spec-2026-11-23.md)

## Checked user-origin Memory mutations

These mutations require a mounted REL `Instance` with `ManageRel`. The caller first obtains an `InspectionCut`; a cut binds the owner and history generation plus the current Archive, Memory, Perception, Relationship, Graph, and Community versions. A new request must match the current cut at publication. Request identity is derived from the operation, authenticated principal, and caller request ID; its receipt is recorded in the specific owner history. Exact retries replay where the operation supports replay. Reusing an ID with changed evidence or a stale cut is rejected according to that operation's contract.

### Create

`WorkspaceManagement::create_memory_at_cut(expected_cut, request)` accepts a `CreateMemoryRequest` with `request_id`, `title`, `content`, `category`, `memory_type`, and `created_at_ns`. Its `CreateMemoryReceipt` contains the resulting `MemorySummary`, the post-commit cut, and `created` (false when the exact prior effect was replayed). The stable request is accepted within the same owner history generation; the request payload cannot be changed under that ID.

### Correct and retire

`correct_memory_at_cut(expected_cut, request)` takes a `CorrectMemoryRequest` containing the stable request ID, target Memory ID, expected revision and body ID, replacement title/content, and creation timestamp. The operation publishes an immutable successor and records the original's supersession/lifecycle effect in one owner commit. `CorrectMemoryReceipt` identifies the original ID and revision, the replacement summary, resulting cut, and whether it was created or replayed. Stale revision/body evidence, a changed cut, or conflicting reuse of a request ID fails before publishing a different effect.

`retire_memory_at_cut(expected_cut, request_id, request)` accepts a `MemoryRetirementRequest` that binds the owner, original Memory ID/revision/body ID, successor ID/revision/body ID. It verifies the archived supersession and reference-closure conditions before writing a terminal tombstone. `RetireMemoryReceipt` returns the tombstoned Memory ID, tombstone revision, Memory version, resulting cut, and whether this call retired it. Missing, terminal, stale, or inconsistent evidence is rejected; retirement does not erase prior history.

### Associate or unlink

`change_memory_association(expected_cut, request)` accepts a `MemoryAssociationRequest` with principal-bound `request_id`, source and target IDs and revisions, action (`Link` or `Unlink`), and optional expected edge ID/revision. The source and target must be distinct and current at the accepted cut. The supported user-origin association is a directed Topical edge; an unlink must identify the exact edge revision. A successful `MemoryAssociationReceipt` contains the resulting edge, cut, and `replayed` flag. Unlink is recorded as an inactive user edge, so Dream processing cannot silently restore the user's removed association. Stale endpoint/cut evidence, mismatched edge identity, and changed request reuse are conflicts or stale errors rather than implicit updates.

## Native and copied-source import contracts

`NativeImportPlan::from_path` prepares a bounded ChatGPT export or normalized manifest, optionally selected by a half-open RFC3339 window. `NativeImportPlan::from_hermes_copy` accepts a trusted host's already privacy-filtered text snapshot, including original profile/session scope, optional known branch/revision, ordered source message IDs, roles, timestamps, text, and provenance-only author metadata. Copy bytes and message sizes are bounded. The resulting receipt preserves the source digest and identity; a local Archive branch describes only the accepted copied view. Unknown source branch or parent identity is not asserted, and author metadata grants no PHY access.

For incremental copied-source import, `from_hermes_copy_extension(snapshot, predecessor_sha256)` accepts only a new contiguous suffix. Publication checks the authenticated predecessor receipt and exact current branch head under the owner writer boundary. The extension identity binds both the prior digest and suffix; changed prefixes, branch/head changes, duplicate IDs, or stale anchors are refused. Exact retry does not allocate duplicate Archive turns. A terminal notice uses `Instance::tombstone_hermes_copy(owner, scope, source_sha256)` after the import epoch settles. It is bound to the same principal/source scope and exact accepted copy digest, covers the copied conversation including extensions, hides its source bodies from inspection, and is idempotent. A pending open epoch must be settled before terminalization.

## Bounded inspection and Community projections

`WorkspaceManagement::inspect(query, cursor, limit)` returns an `InspectionPage` with an exact `InspectionCut`, items, an optional continuation, and `truncated`. The `InspectionCursor` is opaque and bound to the file, query, and exact cut; reusing it after owner changes or with a different query fails stale. Pages are limited to 128 items and 256 KiB. Callers must continue with the returned cursor and cut rather than inventing offsets for cursor-based queries.

- `InspectionQuery::MemorySummaries(filters)` filters by text, category, memory type, lifecycle, and current Community. Query text is limited to 4,096 bytes, category/type filters to 128 bytes, titles to 256 bytes, excerpts to 512 bytes, and the scan window to 512 Memory IDs per page. It returns cut-bound continuation; terminal Memory entries do not expose their title or body. A page reports truncation when the supported scan window is reached.
- `Instance::inspect_memory_summary(owner, kind, cut, id)` reads the selected Memory's current summary by ID. It uses the same lifecycle, canonical status, and Community projection as the paginated list and requires the owner read grant and exact inspection cut.
- `InspectionQuery::MemoryGraph(query)` returns a bounded Memory-to-Memory neighborhood. Requests allow 0–128 distinct seed IDs (zero scans at most 512 initial candidates), depth 1–3, at most 500 nodes and 1,000 edges, and may bind an expected cut/community. The walk examines at most 4,096 relations and is also bounded by serialized page bytes; the result marks truncation rather than implying the graph is exhaustive.
- Exact detail, history, provenance, Archive turns, graph neighbors/edges, entities, relationships, Communities, and Community members use their corresponding `InspectionQuery` variants. Community membership reads require the accepted current Community generation and return bounded pages.

`rename_community_at_cut(expected_cut, request)` accepts the stable principal-scoped request ID, Community ID, expected Community generation, and validated name. The `CommunityRenameResult` returns Community metadata (including generation, graph version, member count, name origin/current status), resulting cut, `changed`, and `replayed`. A changed name advances Community metadata generation while preserving Graph version and membership; it does not run Dream or generate a new partition. Exact replay returns the original accepted effect. Pruning and storage reclamation preserve the durable user-name receipt.

`WorkspaceManagement::preview_community_recompute()` calculates a fixed-seed Leiden partition for the active graph Memories using REL read access. `apply_community_recompute()` requires `ManageRel` and a writable mount. It captures fresh input, then checks the same owner and Memory, Graph, and Community versions before publication. Stale input returns `ProductErrorKind::Stale`. A successful apply returns the durable generation and whether it was newly written. An identical current partition reuses its generation. Both methods return coverage, the Community size histogram, modularity, and a digest over the sorted member groups.

## Cross-REL Entity identity

`compare_entity_identity(local, peer_owner, peer)` compares one explicit pair and records the receipt in the `WorkspaceManagement` owner REL. It requires `ManageRel` on that REL and read access to the peer, which may be mounted read-only. Entity IDs are owner-qualified, so equal ID bytes across RELs remain distinct endpoints. Core follows each Entity's resolved mentions into its current, directly sourced Memories. This includes migrated resolutions that retain an exact mention position and Entity ID. It also considers grounded Relationships when present. A shared name alone leaves the pair unresolved without a model call. When both sides have independent context, Core makes one call to the configured Entity endpoint. Independently worded accounts can identify the same Entity. Each cited excerpt must match its own source. Repeating a comparison with the same evidence, policy and bound route returns its existing receipt. `SameIdentity`, `RelatedDistinct` and `Unresolved` are revisable automatic judgments for any Entity kind. `RelatedDistinct` means the cited evidence distinguishes the two referents.

`inspect_entity_identity(local, peer_owner, peer, revision)` requires read access to both RELs. It returns `None` before the pair has a receipt, otherwise the selected receipt with `Current`, `Pending`, `Historical`, or `Unavailable` state and cited quotes when both RELs still retain the verified spans. A superseded receipt can still show its original quotes after a correction. Match each `IdentityCitation` as `Mention { side, mention }` or `Relationship { side, relationship, memory }`. Positions index the corresponding evidence on that side of the receipt. `cited_quotes` follows the citation order. Earlier Relationship-only receipts remain readable.

Core revisits previously recorded pairs during an authenticated session with `ManageRel` on the recording REL and read access to the peer. Opening a new session with those grants picks up changes made while the workspace was closed. The Gateway uses its configured Entity endpoint for judgments that need inference. If the endpoint is unavailable, a change that requires it stays pending. Core can record insufficient evidence locally. Only the recording REL receives a receipt. Each Entity keeps its original ID and graph edges.

### Confirm or undo separation

A trusted host calls `TrustedHostAdmission::install_with_entity_review` at setup with the nonsecret principal IDs it designates as operators. Keep the returned `EntityReviewAuthority` in the user interaction handler. Bind it to the operator's authenticated Instance. Both actions require management access to the recording REL and independent read access to the peer. Hermes currently exposes automatic comparison only.

1. To confirm separation, start from a current comparison receipt. Call `EntityReviewSession::prepare(pair, Confirm)` and show the owner-qualified pair, the recording REL, the evaluator judgment and any disagreement. For undo, pass `Undo { target_revision }` and show the exact confirmation and its operator. Retain the prepared action and its `preview().action_id` on the server.
2. On deliberate approval in that session, call `commit(&prepared)`. An uncommitted preview expires after five minutes. Reopen the review after a relevant change, remount or expiry. Retrying a committed action in the same session returns its original event, even after expiry. After reconnecting, the same operator can use `recover(pair, action_id)` with the required grants.
3. Read `inspect_entity_identity` for the selected evaluator receipt and its freshness. The `effective` field always gives the pair's current answer, even when inspecting an earlier receipt. An applicable confirmation gives `ConfirmedSeparate`. After undo, the answer comes from the current evaluator receipt, is pending reconsideration if its evidence or route is stale, or is unavailable if an endpoint is unavailable. Use `inspect_entity_separation` with a revision to inspect earlier operators and decisions. Both reads require access to both owners.

Later edits to the reviewed Entities preserve the confirmation when both histories retain the reviewed revisions. A replacement with an unverifiable history requires a fresh operator review. Only the latest separation event can be withdrawn, and it must be a confirmation. A designated operator may withdraw it even if another operator made it, an endpoint has retired, or continuity can no longer be verified. Undo appends to the recording REL and leaves every superseded confirmation historical. [Confirmation and undo contract](planning/cross-rel-entity-separation-confirmation-spec-2026-11-23.md)

## REL dependencies

`WorkspaceManagement::rel_dependencies()` reads the local direct declaration and its generation. `replace_rel_dependencies(expected_generation, dependency_ids)` replaces it with `ManageRel` and a writable source. Direct targets must be mounted RELs the caller can read. The proposed reachable graph must resolve and be acyclic. An unchanged declaration reuses its generation. An empty replacement clears it. Existing files open at generation zero with no dependencies.

`resolve_rel_dependencies()` returns the ordered, granted closure, ending with the source REL. A denied branch contributes no owners through that path. A granted dependency that is unavailable prevents a partial resolution. `Instance::prepare_context_workflow` renders current stored Ego syntheses from the resolved dependencies ahead of the primary request as budgeted evidence. Caller token estimates cover the base request. Requests, plans and finalization recheck the resolved mounts, declarations and synthesis cuts. Compaction uses its original transcript sources.

## Maintenance and reconciliation boundaries

Rewrite, prune, repair, reclaim, copy, and reconciliation operations create a new destination and never replace an existing source or destination. Quiesce writable owners before preparing copy maintenance, and independently reopen/verify the result. Fresh-history rewrite fails closed when authenticated user Memory correction, retirement, association, or Community naming authority/request history cannot be preserved.

Public REL reconciliation requires both inputs to have the same owner identity. Identical and strict-prefix histories use a verified exact-copy path; supported divergent REL histories use semantic replay into a distinct destination and report replay counts. Divergent tails containing unsupported authenticated user-operation evidence are rejected rather than silently dropped. The internal Application verified-copy helpers also support same-owner REL/PHY prefix copies. There is no public Root/CLI PHY reconciliation API, cross-owner reconciliation, arbitrary semantic merge, or in-place rewrite.

The public facade is a hard cut from the old `Cva` name; there is no compatibility alias. Normal runtime opens only current 2.1 REL/PHY files and has no 2.0/FRS07 reader or converter. Arbitrary Dream/Insomnia maintenance, a deployed network daemon, unrestricted provider administration, and public mutable store/Container access are outside this API.

## Errors and compatibility

Fallible product methods return `ProductError` or a more specific configuration error. `ProductErrorKind` distinguishes input, authorization, revocation, conflict/stale state, wrong format, unavailable service, verification, I/O and unsupported operations. Callers should branch on the kind when recovery differs and avoid parsing error text as a stable machine protocol.

The serialized `ProductErrorKind` values are `wrong_format`, `admission`, `unauthorized`, `revoked`, `unavailable`, `unsupported`, `stale`, `conflict`, `writer_unavailable`, `publication_collision`, `io`, `verification`, and `invalid_input` (defined in [`src/error.rs`](../src/error.rs)). Refresh current cut/revision evidence after stale/conflict failures; inspect the existing writer after writer-unavailable; choose a new destination after publication collision. Authorization, format and verification failures require correcting the underlying authority/input or preserving the file for investigation, rather than blind publication retries.

Runtime accepts the current REL 2.1 / PHY 2.1 format. It does not read the retired 2.0/FRS07 format or convert it. Public APIs intentionally use `Rel`, not the historical `Cva` facade. Copy maintenance creates a new verified destination and fails if that destination already exists.

## Protecting evidence

Relevant executable contracts include `tests/public_api.rs` (owner identity, kind isolation, grants, revocation, copy reopen and history); `tests/native_import.rs` (authenticated import replay after reopen); `tests/oc06_inspection.rs` (`hp02_memory_summaries_filter_and_page_without_returning_bodies`, `hp02_memory_graph_expands_active_neighbors_at_one_cut`, and other bounded/cut-bound projections); and `tests/oc06_routed_provenance.rs` (grant requirements and terminal source). The separate Root-only consumer tests cover checked create replay in `tests/external-consumer/tests/checked_memory_creation.rs`, correction/retirement in `tests/external-consumer/tests/user_memory_correction.rs`, link/unlink replay in `tests/external-consumer/tests/memory_association.rs`, and Hermes extension/terminal scope in `tests/external-consumer/tests/operational_processing/hermes_copy.rs`. `tests/community_management.rs` protects cut-bound naming and replay. `tests/entity_identity.rs::{identity_namesakes_revisit_correction_restart_and_grants,identity_paused_evaluation_rechecks_peer_and_revocation,identity_readonly_peer_and_equivalent_concurrent_requests_publish_once}` covers pair receipts, freshness, grants, race rejection, read-only peers, and equivalent concurrent calls. The [operator review tests](../tests/entity_identity/separation.rs) exercise confirmation against a conflicting evaluator judgment, authorized undo after a replacement, and restoration of the effective answer. CLI parity and operational behavior are covered in `cli/tests/cli.rs`, `cli/src/operational_tests.rs`, and `cli/tests/g4_corpus.rs`.

## Related docs

- [Operator workflow](operator-workflow.md)
- [Configured runtime](configured-runtime.md)
- [CLI reference](reference/cli.md)
- [System architecture](architecture/system.md)
- [Owner-session architecture](architecture/owner-session.md)
- [Gateway architecture](architecture/gateway.md)

## Notes

The source of truth is Root's `src/lib.rs` export list and the flat modules `src/owner.rs`, `src/host.rs`, `src/host_admission.rs`, `src/host_context.rs`, `src/host_entity_review.rs`, `src/host_external_source.rs`, `src/host_instance.rs`, `src/host_project_source.rs`, `src/management.rs`, `src/processing.rs`, `src/retrieval.rs`, `src/native_import.rs`, `src/maintenance.rs`, `src/maintenance_owner.rs`, `src/maintenance_operations.rs`, and `src/configuration.rs`. Internal crate visibility is not public consumer support.
