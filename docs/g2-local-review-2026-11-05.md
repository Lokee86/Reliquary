# G2 local acceptance review

Reviewed rebuild checkpoint `444bb4f1` against the capability tracker in the design worktree. V01 through V07 have recorded acceptance. G2 remains open.

## Historical Freshness gap

The [V06 checkpoint](v05-v06-continuation-2026-11-05.md) calls out historical attribution and propagation as G2 evidence. The rebuild's `RelWriter::settle_native_import` validates completed import work and hashes the owner-local Freshness scores and anchors at the import cut. The native import integration test exercises that settlement's durability, restart and retry.

The reference import reconstructs Freshness from historical source, Memory and Dream evidence. The rebuilt Freshness crate has separately tested historical planners, but the import path does not call them or durably publish their effects. As a result, the current settlement test cannot prove historical attribution or propagation parity.

The remaining integration needs owner-verified inputs at a frozen import cut, historical planner results admitted as durable owner effects, and a comparison case with older eligible Memory and an excluded or later-born Memory. The case must verify resulting scores after reopen and prove that retry adds no credit. Keep the existing owner-session write boundary.

Local compilation, locked offline metadata, formatting, whitespace and the architecture guard passed. The production graph has 23 packages and 76 edges.

The locked offline workspace suite passed 504 tests across 23 nonempty suites when run with Windows hard-link and loopback permissions. The initial restricted run stopped on two verified-copy tests when hard-link publication was denied.

## Archive timeline checkpoint

The Application settlement entry point validates the historical timeline from owner-accepted Archive nodes and activity positions before requesting the existing snapshot. It preserves accepted order when imported timestamps precede previously accepted live turns. The native import test exercises that case and confirms settlement retry and reopen. All 504 local locked offline workspace tests passed. Historical attribution, propagation and durable publication remain open.
