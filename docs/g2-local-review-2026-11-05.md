# G2 local acceptance review

Reviewed the completed rebuild against the capability tracker in the design worktree. V01 through V07 have recorded acceptance. G2 is accepted.

## Historical Freshness gap

The [V06 checkpoint](v05-v06-continuation-2026-11-05.md) calls out historical attribution and propagation as G2 evidence. The rebuild's `RelWriter::settle_native_import` validates completed import work and hashes the owner-local Freshness scores and anchors at the import cut. The native import integration test exercises that settlement's durability, restart and retry.

The reference import reconstructs Freshness from historical source, Memory and Dream evidence. The rebuilt Freshness crate has separately tested historical planners, but the import path does not call them or durably publish their effects. As a result, the current settlement test cannot prove historical attribution or propagation parity.

The remaining integration needs owner-verified inputs at a frozen import cut, historical planner results admitted as durable owner effects, and a comparison case with older eligible Memory and an excluded or later-born Memory. The case must verify resulting scores after reopen and prove that retry adds no credit. Keep the existing owner-session write boundary.

Local compilation, locked offline metadata, formatting, whitespace and the architecture guard passed. The production graph has 23 packages and 76 edges.

The locked offline workspace suite passed 504 tests across 23 nonempty suites when run with Windows hard-link and loopback permissions. The initial restricted run stopped on two verified-copy tests when hard-link publication was denied.

## Archive timeline checkpoint

The Application settlement entry point validates the historical timeline from owner-accepted Archive nodes and activity positions before requesting the existing snapshot. It preserves accepted order when imported timestamps precede previously accepted live turns. The native import test exercises that case and confirms settlement retry and reopen. All 504 local locked offline workspace tests passed. Historical attribution, propagation and durable publication remain open.

## Historical provenance checkpoint

The rebuild now has a pure historical import scorer that combines propagated events with retained live contributions, and the owner snapshot includes accepted events on its final activity turn. The provenance planner now classifies Memory with any unresolved source, content, grounding, or Episode-path evidence as unsupported. The regression covers wholly missing and partially resolved provenance. It verifies a single unsupported result and preserves the missing-evidence diagnostic.

## Final historical Freshness acceptance

Native import now derives a bounded, immutable scoring input from owner-accepted Archive, Import, Insomnia, Memory, Dream, Graph, Community and Freshness projections. Application converts that input into the existing pure historical propagation and chronological scoring types. Owner-session rechecks the evidence digest immediately before publication and Freshness Store durably publishes the recomputed projection with its import identity, base cut, settlement cut and exact journal boundary.

The durable snapshot replays before later Freshness journal activity, so same-turn accepted use written after settlement is preserved. Exact publication and settlement retries are byte-stable and add no credit. Open import fencing remains in force while the narrowly scoped imported Episode completion and first-cycle Dream operations run.

The end-to-end native fixture creates an aged pre-import eligible Memory, a lifecycle-excluded Memory and one imported Memory. It publishes the imported Memory through scoped Insomnia completion, accepts its owner-backed first-cycle Dream relation, settles the historical projection, verifies the older Memory gains credit, verifies the excluded Memory stays outside credit, retries without changing the result, and reopens the owner with the same settled score.

Native Episode attribution also requires every turn in the Episode path to belong to the import receipt. A partial path match is rejected before claim or settlement without changing the owner file.

G2 acceptance covers the capability tracker's supported ingest, Dream, Entity, retrieval, Freshness, vector, context, import and history behavior through the recorded V01–V07 evidence and the final workspace verification below.

## Final verification

The locked offline workspace suite passes with local loopback and Windows hard-link support. Locked metadata and compilation, formatting, whitespace and the architecture guard pass. The production graph remains 23 packages and 76 approved dependency edges.
