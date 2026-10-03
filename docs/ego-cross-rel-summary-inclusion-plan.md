# Ego cross-REL summary inclusion plan

Parent index: [Documentation index](INDEX.md)

## Purpose

Record the future-only policy for including relevant summaries from other RELs in Ego's active context. This is **summary selection across authorized scopes**, not a second cross-REL synthesis pipeline or a combined project/organization summary.

## Overview

Ego should discover and include relevant existing summaries from authorized RELs for the active PHY. It may follow permitted ambient dependencies or separately authorized reverse/dependent scope queries, without importing the source Memory Web, generating a combined synopsis by default, or changing dependency ownership semantics.

## Status and existing foundation

The intended Memory-Web Summaries are user-relative: their source Memories and conversation evidence remain REL-owned, while summary perspective and target persistence are keyed to the active PHY and source REL. The exact PHY-owned per-REL persistence structure and final context assembly are still planned. The runtime graph host already supplies REL dependency closure; Ego does not own or recreate that topology.

Cross-REL summary discovery, authorized reverse traversal, selection, budget integration, and deduplication described here are not yet implemented.

## Use cases

- **Project-facing:** While working in a project REL, Ego may include relevant summaries from authorized organizational, team, program, or other dependency RELs. A project's context can therefore include applicable objectives, conventions, decisions, and standards without importing entire source Memory Webs.
- **Organization/team-facing:** While working in a higher-level REL, Ego may include relevant summaries from authorized dependent project or team RELs. This reverses the direction of context lookup to surface project status, discoveries, blockers, and other information relevant to the current task.
- **Cross-cutting work:** Multiple dependencies and explicitly authorized related scopes may contribute distinct summaries. Organizational labels remain optional REL metadata, never access-control or traversal semantics.

The objective is to move *relevant existing summaries into context*, not to describe the combined state of all reachable RELs by default.

## Ownership and visibility

- Every underlying Memory remains owned by its original REL or PHY. Summary inclusion neither moves Memories nor publishes them into the destination REL.
- A user-relative Memory-Web Summary is associated with the **active PHY** and the **source REL**. Ego may select that PHY's summaries for authorized source RELs. It must not traverse another user's PHY or treat another user's user-relative summary as a shared organization/team synopsis.
- If a true team-wide or organization-wide shared synopsis is later required, it is a distinct artifact with explicit shared ownership, generation, and disclosure policy; it is not implicitly synthesized from another user's PHY summary.
- Opening a REL does not grant access to every related REL. The host must resolve permitted scope paths and the applicable authorization before Ego sees candidate summaries.
- Source owner, summary identity/revision, source evidence references, and visibility constraints survive inclusion in the destination context. They do not become destination-owned evidence.
- Cross-REL discovery may use authorized metadata and (when available) Perception relationships as relevance signals, but Ego must work without Perception. Neither derived graph structure nor metadata grants access.

## Traversal rules

REL dependencies still mean **dependent -> ambient dependency** under [ADR 0029](decisions/0029-active-rel-hierarchy-and-deferred-connection-scope.md). The host's authorized dependency closure is the default downward-to-ambient context source; Ego consumes it rather than implementing a competing graph walker.

**Reverse lookup is a separate explicitly authorized query**, not a reversal of dependency inheritance. For a higher-level REL, the host may expose permitted dependents and other explicitly authorized related scopes as *candidates*. Authorization must cover the actual source REL and summary; merely being a parent/dependency of a REL is not sufficient permission to read it. Reverse lookup must never silently include every descendant or sibling, and a related REL does not become an ambient dependency by being selected for one task.

The candidate set is finite, deduplicated by durable source REL identity, and cycle-safe. REL type labels such as `Organization`, `Team`, or `Project` never alter traversal, storage, or visibility rules.

## Selection and context assembly

1. Start from the current REL, active PHY, active task, and the host-supplied set of authorized candidate RELs (ambient dependency closure plus any explicitly permitted reverse/related queries).
2. Discover existing per-REL Memory-Web Summaries for the **active PHY**. Use summary metadata, content, source identity, and task relevance to choose candidates; do not regenerate summaries simply because they cross a REL boundary.
3. Prefer the active REL's applicable summary and add only relevant external summaries. Preserve source attribution and allow targeted retrieval of original evidence where source permissions permit.
4. Deduplicate overlapping material and avoid recursive expansion. An included organization summary must not automatically reimport the same project through reverse traversal or trigger an unbounded chain of summary inclusions.
5. Fit selected summaries and the other higher-priority Ego layers into the existing injection budget. A selected cross-REL summary consumes the **Memory-Web Summary/context-assembly allocation**, not an additional unlimited allowance. Use available persisted summary tiers where suitable; omit lower-relevance material rather than padding context.

The inclusion pass selects and composes existing summary material. It does not invoke Dream/Perception, create cross-owner Graph/Community authority, or write a synthesized combined summary as routine output. If the user explicitly requests a portfolio or multi-project synthesis, that is a task-specific operation over permitted sources, not default Ego orientation.

## Freshness and failure behavior

Each summary keeps its originating REL/PHY source version and owner-specific refresh policy. Cross-REL inclusion **does not** refresh, invalidate, or advance another REL's activity clock. It can use freshness metadata when ranking current candidates; underlying summaries refresh through their existing owner-local queues.

If a source summary is missing, stale, inaccessible, or has incompatible visibility, fail closed on access and degrade gracefully in context assembly. A stale summary must not be represented as newly verified state. Where allowed and useful, targeted source retrieval may supply detail independently of summary inclusion.

## Separate context lanes

- **Per-REL Memory-Web synthesis:** produces user-relative summaries from one REL's evidence for the active PHY.
- **Cross-REL inclusion (this plan):** selects existing summaries across explicitly authorized REL scope paths in either direction.
- **Cross-chat:** separately selects existing conversation-compaction records from the *current REL only*, by recent activity and remaining budget.
- **Current-session context:** remains outside Ego's cross-chat budget.

These boundaries prevent cross-REL inclusion from becoming another synthesis or cross-chat mechanism.

## Implementation and validation

1. Finish the PHY-owned, REL-keyed Memory-Web Summary identity/persistence contract and provide a read interface that returns source provenance and freshness without copying source evidence.
2. Consume the graph host's existing authorized ambient closure; define a separate authorization-checked dependent/related candidate query for reverse lookup rather than modifying dependency meaning.
3. Extend Ego area/context selection to choose, deduplicate, attribute, and budget existing summaries across candidate RELs; preserve the Perception-optional fallback.
4. Test active-project -> team/organization inclusion, authorized organization -> project inclusion, multiple dependency paths, denied reverse/sibling access, non-active PHY exclusion, cycles/duplicates, stale/missing summaries, and unchanged current-REL Cross-chat behavior.

Traversal breadth, semantic relevance thresholds, tier selection, overlap removal, and per-layer budget splits require corpus calibration. No exact values are established here.

## Related docs

- [Ego Memory-Web summary plan](ego-web-synthesis-plan.md)
- [Ego Cross-chat context plan](ego-cross-chat-context-plan.md)
- [ADR 0029: Dependency-based context inheritance](decisions/0029-active-rel-hierarchy-and-deferred-connection-scope.md)
- [ADR 0034: Active-PHY privacy boundary](decisions/0034-cross-owner-relationship-graph-and-active-phy-privacy.md)
- [Roadmap](roadmap.md)

## Notes

When this ships, move implemented context-selection behavior into current architecture/API documentation and retain only unresolved design and calibration here.
