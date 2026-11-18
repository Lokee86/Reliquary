# REL dependency port specification

Parent index: [Planning](./INDEX.md)

## Purpose

Restore REL-declared dependencies and ambient context inheritance in Reliquary 2.1. Today a host can select individual Memories from owners it can read. Reliquary has no durable declaration that a REL depends on other RELs, so it cannot resolve or assemble that context for the host.

## Overview

A REL declares a set of dependencies by durable `OwnerId`. Gateway resolves the authorized portion of that graph in dependency-first order. The existing context workflow uses current, stored Ego syntheses from those dependencies. A host can inspect or replace direct declarations through Root and use the same resolver from the CLI.

ADR 0029 defines homogeneous RELs and the dependency graph. ADR 0037 assigns the graph and context composition to Reliquary. ADR 0038 binds ambient context to the requesting Instance and its grants. Use the archived copies at `f431401e` under `docs/decisions/` and the [homogeneity roadmap](../rel-homogeneity-and-dependency-roadmap-2026-11-10.md).

## Target architecture and reuse

| Owner | Responsibility |
| --- | --- |
| Owner-session | Own the REL-local declaration, replay it, expose a bounded snapshot and publish through `RelWriter`. Use the existing Container append, sync and recovery boundary. |
| Gateway | Own dependency edit validation, mounted graph traversal, grants and the binding between resolved owners and a prepared context. |
| Application and Context Engine | Render copied synthesis evidence ahead of the primary context request, within the existing context budget. Keep the current conversation and compaction interfaces. |
| Root and CLI | Expose authorized management and resolution. Use the existing `WorkspaceManagement`, host admission and context entry points. |

Reuse the declaration model, canonicalization and replay patterns from `src/owner_lifecycle/metadata/`, mount and cycle validation from `runtime_host_gateway_ambient.rs`, and the behavioral fixtures in `reliquary_tests.rs` and `runtime_host_gateway_ambient_tests.rs` at `f431401e`. Use 2.1's 16-byte owner IDs and its own record layout. The old `dependency_closure_for` visits the selected REL first. The 2.1 traversal follows ADR 0029's dependencies-before-dependent order.

## Declaration and publication

- The state is a generation and a sorted set of direct dependency owner IDs. A file with no declaration starts at generation zero with an empty set. Use the original limit of 4,096 direct dependencies. Check the input count before canonicalization. Sort and deduplicate before comparing the set. Reject self-edges, the zero ID and oversize input.
- `WorkspaceManagement::rel_dependencies()` returns the source REL's declared IDs and generation with REL read authority. Those IDs are source metadata. Viewing them grants no access to the referenced owners.
- `replace_rel_dependencies(expected_generation, dependency_ids)` requires `ManageRel` for the source and its writable mount. Return the accepted declaration and whether it changed. Compare the generation before accepting a no-op. An identical set reuses the current generation. A changed set publishes one next-generation snapshot. An explicit empty set clears the declaration, including when former dependencies are absent.
- For a changed set, each proposed direct dependency must be a mounted REL the caller can read. Validate the entire graph reachable from the proposed source declaration against mounted metadata, including transitive edges through owners the caller cannot view. Reject a cycle or unresolved owner before append. Use generic errors for failures involving hidden owners. This internal validation conveys no authority or metadata about those owners.
- Serialize REL dependency updates with REL mount removal and replacement at the Gateway. Capture immutable declaration snapshots and hold at most one owner session lock at a time. Hold the topology write guard through validation and the source commit, and recheck credentials, the source mount and expected generation at the writer. In particular, concurrent edits to different RELs within one Gateway cannot both publish reciprocal edges. A container failure follows normal writer poisoning and recovery. Report success only after sync.

## Resolution and context

1. `WorkspaceManagement::resolve_rel_dependencies()` returns the effective ordered owner IDs, including the requesting REL last. Start with its explicit read grant. Capture the graph under the read side of the Gateway topology guard. Check each encountered dependency's owner grant before looking up its mount or reading its declaration. Omit an ungranted branch and every path through it. An independently reachable, granted owner may still contribute through another path. A missing granted mount or reachable authorized cycle fails the resolution without partial context.
2. Use iterative postorder traversal with ascending owner IDs as the tie-breaker for direct edges. Visit each owner once. This supports shared grandparents, multiple parents and depth bounded by the mounted graph. For example, if A depends on B and C, B sorts before C, and both depend on D, the result is D, B, C, A. A mounted sibling enters only through an authorized path.
3. The existing `prepare_context_workflow` uses that resolver for its conversation REL. For each resolved dependency that has a stored Ego Web Synthesis at its current Memory version, prepend an owner- and revision-labeled evidence message in resolved order. An owner with no current synthesis still participates in traversal. Use the owner-session projection of the stored value. The primary REL contributes its existing context, including the caller's explicit Memory and project evidence under their current grants.
4. Apply the existing source byte and message limits while collecting evidence, including labels. Use the existing token counter for the rendered messages. Add their cost to the larger of the caller's pre-inheritance token estimate and the measured base request. Reserve that cost when choosing the transcript tail under the existing compaction target. If the evidence and required primary context cannot fit, return the existing bounded-context error. Put inherited content in request messages as evidence, outside the transcript and compaction inputs, so a conversation checkpoint cannot retain it as inherited context. Zero dependencies use today's context path and budgets.
5. Bind the prepared request to the Instance, each resolved mount instance and declaration generation, and each dependency's eligible synthesis revision or absence, and the Memory cut used to determine eligibility. Recheck these before returning a request or plan and through the existing finalization guard. A changed binding requires fresh preparation. Release owner and topology locks before token counting or inference. Topology and synthesis reads never grant writes, repository access, or access to a PHY.

Use the existing product error kinds. Stale generation or prepared evidence returns `Stale`. Invalid IDs or bounds return `InvalidInput`. Cycles return `Conflict`, absent granted RELs return `Unavailable`, and grants, revocation and writer access retain their corresponding errors.

## Storage and compatibility

- Preserve the 2.1 header, legacy scope values, owner IDs and all previously accepted frames. Opening, listing or resolving an existing file adds zero bytes and requires no migration. Use each legacy scope with the same dependency rules. The scope field continues to round-trip for existing API and file-identity consumers.
- Append a top-level REL-only `RELDEP01` frame for each changed declaration. The payload contains the eight-byte magic, 16-byte source owner ID, little-endian u64 generation, little-endian u32 count, and exactly that many sorted 16-byte owner IDs. Validate ownership, canonical ordering, bounds, exact length and contiguous generations at reopen. A malformed, foreign or misplaced dependency frame fails typed admission, including in PHY. Validate one owner's file independently of mounted dependencies. Check the graph at edit or resolution time.
- Preserve the complete declaration sequence in verified copies, same-owner prefix reconciliation, prune and reclamation, and same-owner fresh-history rewrite. Treat those frames as retained records with no physical pointers. Divergent copies can retain shared-prefix declarations. A dependency frame in either divergent tail triggers the existing unsupported-state path before output. An identity-changing rewrite with dependency history also uses that refusal until rebinding owner declarations has an explicit contract.
- Port-aware 2.1 applications are the support boundary for dependency-bearing RELs. Keep pre-port frame codecs and historical cut shapes as they are. The dependency generation governs current owner configuration. The legacy `CVARELM1` byte format belongs to the historical conversion workflow.

## CLI and delivery boundary

Use `rel dependency list PATH`, `rel dependency replace PATH --expected-generation N`, and `rel dependency resolve PATH`. Replacement takes repeatable `--dependency PATH` options as the complete direct set. Each path is opened as a REL and persisted by owner ID. Replacement and resolution take repeatable `--mount-rel PATH` for additional read-only RELs such as transitive dependencies. Use the same explicit mount option on the existing `rel context` commands. Only the source of a replacement needs a writer or `ManageRel`. The CLI routes these operations through Root.

This port's delivery surface is 2.1 declaration management, resolution and request context. The [homogeneity roadmap](../rel-homogeneity-and-dependency-roadmap-2026-11-10.md) owns broader changes to scope metadata and host roles. Owner-local search and writes keep their established owner-qualified contracts.

## Completion

An existing 2.1 REL can open before and after declaring dependencies. In a fresh process, its declaration and ordered, grant-scoped context match the live Gateway result. The focused checks cover a multi-parent graph, a denied branch, cycle and stale-write rejection, prepared-context reauthorization, and exact copy and replay boundaries. The [execution plan](rel-dependency-port-plan-2026-11-18.md) defines implementation order and recording of the checks.

## Related docs

- [Execution plan](rel-dependency-port-plan-2026-11-18.md)
- [Homogeneity roadmap](../rel-homogeneity-and-dependency-roadmap-2026-11-10.md)
- [Gateway](../architecture/gateway.md)
- [Owner session](../architecture/owner-session.md)
- [Public product API](../public-product-api.md)

## Notes

Source baseline is `aa724265`. The port uses the existing REL write lease and the existing crate boundaries.
