# Public interaction, retrieval and CLI contract — 2026-11-07

Parent: [operational completion specification](public-api-cli-operational-completion-spec-2026-11-07.md). Evidence: [gap audit](public-api-cli-gap-audit-2026-11-07.md).

**Status: specified target, not current CLI documentation or an acceptance claim.** OC05, OC06, OC07 and OC08 implement this contract. Retrieval and queries are required product behavior, not optional extras after import. All command examples below are interfaces to implement; do not report them as already available.

## Purpose

Make Reliquary usable for ordinary questions about its stored knowledge: find relevant Memories, refine a search, follow relationships, inspect exact source evidence, and let a host consume selected content safely. Expose the same operations to a local operator without requiring Hermes, a daemon or direct access to internal crates.

## Overview

Three distinct operations must remain distinct:

1. **Inspection/search:** read stored state and return evidence. Lexical search and deterministic graph/source inspection need no inference. Hybrid/semantic search uses the configured compatible embedding route. None awards Freshness.
2. **Agentic retrieval:** the existing decision-driven `memory_query`, `refine_query`, `traverse_graph` behavior with an owner/principal/turn-bound controller. A model approves only offered, authorized candidate identities. Evaluating or previewing those results is still not external consumption.
3. **Confirmed host use:** the registered host actually consumes an exact prepared manifest, after which accepted-use/Freshness is recorded once. Failed, omitted, cancelled, stale, denied, pending or unknown delivery gets no fabricated credit.

A natural-language query returns grounded Memory evidence. This work does not add answer generation, a new reasoning agent, generic database access, or a chat UI. Hosts may use the returned evidence in their own response generation; that does not relocate the retrieval policy into each host.

## Public API shape and authorization

Use product-owned opaque handles over existing Runtime/Application mechanisms. A `RetrievalSession` is bound to the authenticated Instance, principal, owner, external turn identifier and immutable mode (`Inspect` or `Deliver`). Public calls expose `search`, `begin_retrieval`, `memory_query`, `refine_query`, `traverse_graph`, bounded preview/inspection, cancellation, and the existing deliver/recover operations through nameable Root types.

Root is the only Reliquary dependency of hosts and CLI. They must not build `RelRetrievalSnapshot`, inject invented Memory scores, read store fields, construct internal work claims, serialize writer handles or use private modules to implement the public host trait. The normal text-query path resolves query embeddings and decision endpoints inside the library using the configured runtime; requiring callers to calculate vectors themselves is not operational closure.

Read/search requires the appropriate View permission or its existing stronger equivalent. Confirmed-use publication requires the applicable Operate permission and writable owner. Explicit user-origin management requires Manage permission. Revalidate after provider calls and before hydration/handoff/publication. Scoped IDs and file paths are never credentials. Denied results must not leak private owner existence, counts, titles, graph neighbors or source snippets.

Existing accepted agentic retrieval is **REL-scoped**. PHY creation/routing and authorized exact inspection must work, but agentic PHY inclusion and cross-owner merged retrieval are not smuggled into this completion task. Reject unsupported scopes explicitly. Preserve already-accepted dependency authorization wherever an operation actually supports it; a local CLI is not permission to traverse unrelated owners.

## Search modes

| Mode | Library behavior | Failure / reporting |
| --- | --- | --- |
| Lexical | Existing owner-derived lexical ranking and active/canonical filtering; deterministic identity tie ordering; no provider invocation. | Missing owner/query, invalid limits or denied scope are typed errors. No hidden embedding/network call. |
| Hybrid | Existing lexical plus semantic behavior using the active compatible profile and configured query embedding; keep established weights/limits and retained policy. | Missing/incompatible profile, vector coverage or provider error is explicit. No silent lexical-only success. An explicitly requested supported fallback reports the actual mode and reason. |
| Semantic routing | Existing community-routed semantic lookup, including its accepted stale/missing community fallback behavior, with exact owner/cut/profile bindings. | Report routing/fallback actually used. Do not equate semantic-only routing with the default hybrid engine or silently change ranking. |
| Agentic query | Existing query candidate policy plus bounded decision selection using the configured decision route. | Report selection failure separately from no matches. Invalid/foreign/invented approvals fail closed and leave the prior controller state intact. |

Passive hybrid ranking and agentic candidate selection are distinct implementations in the accepted record. OC05 must preserve/test the governing semantics of each, not claim every entry point already shares one algorithm. Configuration defaults are derived from accepted policy, not hardcoded anew in CLI. No marketing recall/accuracy claim follows from successful execution.

Results contain owner ID/kind, Memory ID, exact revision/body ID, title/content within bounds, source/provenance references, lifecycle/availability, captured owner/graph cut, effective query/mode, profile identity when relevant, ranking facts actually available, truncation and fallback/omission status. Do not fabricate missing scores, summaries or confidence values. Incompatible or absent data is not an empty successful match set.

## Stateful query, refinement and graph traversal

Preserve the [accepted retrieval contract](retrieval-confirmed-use-acceptance-2026-11-05.md): query/search state is local to one external turn; initial query offers up to ten candidates; refinement rewrites the whole query and reconsiders declined searched candidates in bounded batches without a new global retrieval/embedding call; traversal considers current incoming/outgoing one-hop relations with ten candidates per anchor, independently reported and globally deduplicated.

A new query clears the searched set while preserving genuinely confirmed selected IDs within the same delivery turn. A new external turn resets both. Refinement/traversal without an active query is an error. Unknown/foreign/tombstoned/unreadable anchors are rejected rather than used to cross a privacy boundary. Report unseen/declined counts and direction/kind evidence for each anchor; do not recursively walk the graph under a one-hop command.

On provider failure, cancellation before result acceptance or invalid response, preserve the previous controller state. Concurrent actions on one handle must either serialize or reject a stale generation; never interleave search frontiers. Owner remount, profile change, credential revocation and stale source/body cuts invalidate affected prepared work explicitly.

### Inspection mode: no pretend delivery

The CLI and ordinary diagnostics default to `Inspect`. They need actual content plus continuing query state without a `TrustedHostAdapter` that falsely confirms consumption. Add a narrow Runtime inspection transition reusing the same Application evaluation; do not clone the retrieval engine into Root or CLI.

A successful preview updates only its local query/evaluated-candidate state. It must not persist accepted use or populate the delivery controller's confirmed `SelectedIDs`. Optional local `displayed_ids` are diagnostic only, never durable consumption proof. A preview can therefore repeat an approved Memory; display repetition as such rather than falsely reporting `already_provided` as confirmed host use. Existing pending-delivery state must not be created for an inspection result. Inspection and delivery session handles are not interchangeable.

Inspection queries, previews, graph expansion, provenance reads, CLI printing, stdout piping, JSON export and dashboard display all remain non-reinforcing. Ending the inspection process drops its ephemeral controller. It does not update the owner, accept a hidden use receipt or implicitly resume a previous process's selected IDs.

### Delivery mode: preserve exact confirmation

Expose the existing `PreparedMemoryDelivery`/outcome and exact host delivery contracts through Root-owned opaque wrappers, including every type required to implement `TrustedHostAdapter`. Authorized preview reveals only the prepared immutable values; it grants no confirmation authority. A token alone, a caller-supplied digest or a Boolean `used=true` cannot manufacture a receipt.

Keep the established stable-use identity and manifest binding: Instance/principal/owner/turn, exact Memory revisions/bodies, graph/community cut, payload bytes and digest. Before sending, verify writable admission, current grants and source validity. A durable prior receipt suppresses another host call. A host-confirmed exact manifest can recover a missing local receipt after reopen; later local query/search state must not be discarded during recovery.

A pending/unknown host outcome remains Held and cannot be blindly redelivered. Terminal rejection and explicit pre-handoff cancellation release pending state without credit. If content was consumed but receipt publication failed, preserve/reconcile that fact and award at most once; do not pretend it never happened or start a different operation ID. Failed or omitted context gets no credit.

The CLI inspection commands do not provide a generic `--confirm-used` flag. Real delivery/recovery remains a host capability with executable conformance tests, not an operator assertion that a Memory was probably used.

## Inspection and supported management

Expose bounded Memory list/get/provenance/history, source conversation/branch/transcript, graph neighbors/edge details, Entities/relationships and community metadata through authorized immutable projections. Source drill-down resolves the exact supporting revision/span/time where retained; a pruned/stale/unavailable source is reported explicitly, never replaced with current text.

Deterministic graph inspection is separate from decision-driven `traverse_graph`. Graph inspection lists a bounded one-hop neighborhood and actual edge origin/kind/direction without inference. Historical exact reads use the accepted historical cut API; the spec does not promise historical ranked search where only current retrieval exists.

Use existing management defaults/caps, including 128 items and 256 KiB per existing management page. Introduce stricter bounds for new payloads where needed and bind cursors to owner/filter/generation/cut. A stale cursor requires resynchronization; never concatenate pages from incompatible states. Return honest absent, unavailable/pruned, terminal, denied, corrupt and incomplete states according to the existing error/privacy contract.

Expose already-supported user-origin Memory creation, semantic retirement and typed relationship operations with expected revision and stable request identity. Add only the narrow workflow needed for a retained operation; no arbitrary raw edge write, undelete, lifecycle override or unrestricted store editor. Required operations without an accepted primitive stay explicit dependencies, not successful no-ops. Existing Identity/Personality/Anchor records may be inspected/managed through their supported store contracts; new synthesis is out of scope.

Mounted read/write owners must use the mounted read projection under the appropriate short lock. Do not reopen the same file as an independent reader while a writer lease makes that illegal on Windows. Provider calls occur outside owner mutation locks; revalidate before commit.

## CLI contract

The command grammar below is a target contract. Existing maintenance commands remain unchanged. Common `--config <FILE>` selects route/principal/grant configuration; secrets are never passed on the command line. Use `--json` consistently for machine output; diagnostics go to stderr. Unknown flags, duplicate options and unsupported owner modes fail nonzero before mutation.

| Command family | Required behavior |
| --- | --- |
| `rel create/info/verify/history/rewrite/prune/repair/reclaim/reconcile` | Retain current supported behavior and negative cases. |
| `rel import <REL> --source <FILE> --format <chatgpt|normalized> [--start <RFC3339> --end <RFC3339>] [--phy <PHY>]` | Normalize/plan before publication; stage source-bound import; optional explicit `--process` delegates to the same processing operation. Report selected coverage/omissions and resumable import ID. |
| `rel process <REL> [--import <ID>] --until <idle|ready> [--max-episodes <N>] [--phy <PHY>]` | Drive declared configured stages, expose progress and typed stopped/held/failed/incomplete states. `idle` is not `ready`. Never mark an unfinished import complete. |
| `rel work <status|resume|stop> <REL> ...` | Inspect or control a supported owned run; respect leases/checkpoints and idempotent recovery. Offline stop cannot pretend to control another process without an existing control channel; report that limitation. |
| `rel search <REL> --query <TEXT> --mode <lexical|hybrid|semantic> [--limit <N>]` | Stateless bounded read/search with actual mode/profile/fallback and provenance; no accepted-use mutation. |
| `rel query <REL> --query <TEXT>` | One-shot agentic **inspection** query using the configured decision route; emit actual selected Memory evidence, not a synthesized answer or consumption receipt. |
| `rel interact <REL> [--commands <JSONL>]` | One local process with an opaque inspection session. Support query, refine, traverse, show, source, neighbors, status, new-turn and quit using the same Root calls. No network daemon. |
| `rel memory <list|get|provenance|history|add|retire> <REL> ...` | Bounded exact reads and supported typed user-origin mutations. History preserves existing syntax/aliases only where explicitly documented; retirement is not physical deletion. |
| `rel archive <conversations|branches|show> <REL> ...` | Exact source inspection and bounded transcript pages; no model invocation. Preserve old history-summary commands. |
| `rel graph <neighbors|edge> <REL> ...`; `rel entity/relationship/community ...` | Bounded deterministic inspection; expose retained mutations only through named verified operations, never generic storage access. |
| `rel vectors <status|build> <REL> ...`; `rel insomnia/dream/entity-resolution/freshness ...` | Named controls over the same processor stage APIs; no second coordinator or private worker shortcut. Missing stage capability is explicit. |
| `rel conversation/context/source ...` | Existing source feed/status, queue/stream controls, context inspection/checkpoints and external-source interactions with required host bindings. A missing trusted host execution capability is an error, not simulated generation. |
| `phy create/info/verify/memory/history ...` | Minimal explicit PHY creation and authorized supported inspection needed by paired operation. This deliberately extends the earlier REL-only CLI policy; it does not add PHY Archive, REL activity clocks or cross-owner reconciliation. |
| `config check <FILE> [--online]` | Redacted offline validation by default; explicitly requested provider readiness probes only. Do not invent a replacement credential store or auto-discover unrelated account secrets. |

A root-only shell invocation illustrates the intended interaction:

```text
reliquary rel interact project.rel --config reliquary.cfg
query "What collision geometry does the server use?"
refine "Only the user's final requirement, not assistant suggestions"
traverse <MEMORY_ID>
show <MEMORY_ID>
source <MEMORY_ID>
new-turn
quit
```

The line syntax is parsed into the same validated commands as JSONL input. In JSONL mode each record includes a request ID and command; each result/error echoes that ID. Newline-delimited replies contain one complete bounded JSON object; no model chatter or progress text is mixed into stdout. Stdin/file input is data, never shell code.

One-shot `query` deliberately starts/ends one inspection turn. A subsequent separate invocation cannot pretend to refine its in-memory state. Use `interact` or `--commands` for multi-step sessions. Do not add unsigned controller-state files or a service merely to make separate commands appear stateful. Process restart starts a fresh inspection turn; only genuine host-delivery recovery uses the durable exact-manifest recovery contract.

## Interaction acceptance matrix

| Case | Required executable evidence through Root and CLI where applicable |
| --- | --- |
| I01 — search | Full-population lexical/hybrid/semantic behavior, ties, filters and caps; configured embedding/profile used correctly; absent/incompatible route does not silently pass as another mode. |
| I02 — query and turn identity | Valid query through configured decision route; invalid/foreign/duplicate approvals rejected; separate principals/owners/turns isolated; one-shot and shell explicitly differ in lifetime. |
| I03 — refinement | Full-query rewrite; searched frontier preserved; declined candidates reconsidered in bounded batches with no extra global retrieval/embedding; failure leaves prior state unchanged. |
| I04 — graph interaction | Active incoming/outgoing one-hop neighborhoods, per-anchor caps/counts, deduplication and directions; foreign/retired/unreadable anchors denied; deterministic neighbors works without inference. |
| I05 — preview and cancellation | Inspect mode continues query/refine/traverse without a fake host or stuck pending delivery; no accepted-use receipt, selected-as-consumed state or owner byte change from any display/read; cancellation handled. |
| I06 — actual use | Exact successful host handoff reinforces once; retry/reopen recovery does not resend or re-award; missing/wrong/unknown/stale receipts never credit; consumed-but-unrecorded recovery tested. |
| I07 — source and history | Returned Memory revisions/bodies match owner state; exact user source/provenance resolves; unavailable/deleted/pruned payload is explicit, not substituted; no privacy leaks. |
| I08 — public surface and grants | All host trait/value types available via Root only; View/Operate/Manage and readonly/writable behavior enforced; revocation between inference and handoff prevents unauthorized action. |
| I09 — CLI parity/lifecycle | Public API and separate-process CLI agree on fixture results/errors; JSONL order/request IDs, errors, reset, interruption and fresh process behavior; no unsigned state replay or internal dependency. |
| I10 — real native corpus | Existing configured endpoints process the preserved two-day selection; meaningful grounded query/refine/inspection and eligible traversal verified after independent reopen. Archive-only or zero accepted Memories is not whole-workflow success. |

Deterministic fixture tests define exact expected selection and recovery behavior. The separate live run demonstrates operational integration and records actual model behavior, not universal recall quality. Root and CLI tests must independently create/process their fixtures; internal prepopulated state is supplementary evidence only.

## Related docs

- [Operational specification and OC01–OC09](public-api-cli-operational-completion-spec-2026-11-07.md)
- [Gap audit](public-api-cli-gap-audit-2026-11-07.md)
- [Accepted retrieval contract](retrieval-confirmed-use-acceptance-2026-11-05.md)
- [Existing public API](public-product-api.md)

## Notes

This interaction work is necessary even when Reliquary is embedded in another host. The CLI is the first concrete consumer and verification surface, not the owner of retrieval logic. The scope covers the user's retrieval/query requirement while deliberately excluding a new agent framework, dashboard or service architecture.