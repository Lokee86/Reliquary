# REL dependency port execution plan

Parent index: [Planning](./INDEX.md)

## Purpose

Implement the [REL dependency port specification](rel-dependency-port-spec-2026-11-18.md) in native Reliquary 2.1. Use baseline `aa724265` and the original implementation at `f431401e`.

## Overview

Use one implementation lane in dependency order. Test each seam when its contract is available and run the applicable repository checks at integration. Use small REL fixtures and authored syntheses through the existing Ego store test setup.

## Work packages

| Work package | Depends on | Deliverable and exit check |
| --- | --- | --- |
| 1. Owner storage and compatibility | Baseline | Adapt the old declaration store to the [owner-session](../../crates/reliquary-owner-session/src/session/rel_session.rs) and [writer](../../crates/reliquary-owner-session/src/session/rel_writer.rs). Add typed replay and the REL-only validation guard. Route the frame through verified copy, prefix reconciliation, prune, reclamation and rewrite as specified. Verify an old typed file and one written declaration across reopen, including generation and exact owner identity. |
| 2. Gateway and Root management | 1 | Add shared topology coordination at [mount and quiesce](../../crates/reliquary-runtime-gateway/src/gateway_mounts.rs), one validator and ordered resolver, and the three [Root management](../../src/management.rs) methods. Check expected generation and grants at publication, reject cycles and missing reachable owners, and reuse the generation on a no-op. Verify the authorized order and reciprocal-write conflict with the graph fixture below. |
| 3. Ambient request context | 2 | Read current owner-local Ego synthesis with the dependency snapshot. Bind order, mounts and synthesis to [Gateway preparation](../../crates/reliquary-runtime-gateway/src/instance_context/prepare.rs) and its [existing recheck](../../crates/reliquary-runtime-gateway/src/instance_context/finalize.rs). Apply request-only evidence and budgeting in [Application](../../crates/reliquary-application/src/context_workflow.rs). Verify the actual context request, stale recheck and compaction behavior on that same graph fixture. |
| 4. Operator surface and integration | 3 | Wire the dependency commands and explicit read-only mounts through the [CLI local host](../../cli/src/local_host.rs) and the [context commands](../../cli/src/context_source.rs). Update API, CLI, operator and affected architecture, contract and coverage owners. Run the fresh-process smoke and the normal integration checks. Record the evidence and commit the completed unit. |

## Focused verification

**Owner replay and maintenance.** Use an existing owner-session fixture or pre-port 2.1 fixture. Read it at generation zero and compare its bytes after the handle closes. Exercise canonical replacement and clearing, an invalid frame on reopen, and exact retained dependency history. Extend the existing [maintenance acceptance](../../tests/g4_maintenance.rs) seam to prove a retained copy reopens with that history and that both an identity-changing rewrite and a divergent dependency tail refuse before publication. Reuse the existing copy and recovery tests for ordinary frame behavior.

**Gateway and context.** Extend the [existing context test seam](../../crates/reliquary-runtime-gateway/src/instance_context/tests.rs) with a diamond, a deeper ancestor and an unrelated mounted REL. Check dependency-first order, shared ancestor deduplication and an ungranted branch. Cover a missing granted mount, a stale expected generation, read-only write denial and one reciprocal-edit race. Seed a small synthesis for selected nodes, leave an intermediate without one, and inspect the real returned request. Reuse the fixture to verify a topology or synthesis change makes a prepared request stale and that inherited text stays out of the compaction plan. Exercise one case that reaches the existing context budget.

**CLI smoke.** On tiny disposable 2.1 RELs with mixed legacy scope values, list the pre-port declaration, replace it, then use separate CLI processes to list, resolve and verify. Supply dependencies by path in the CLI and compare the reopened owner IDs, generation and order. Check the source's header and owner ID and compare the read-only dependency files' hashes. The Gateway fixture supplies the full context assertion.

## Integration and evidence

- Run formatting for workspace and CLI, `cargo check --workspace --locked --offline -j 1`, the locked offline workspace tests, and the CLI suite once. Run the package architecture and documentation checks appropriate to the changed ownership and public surfaces. Use the existing [development workflow](../development/workflow.md) commands.
- Check the final diff against the spec by reading the normal public path from CLI and Root to Gateway and owner-session, the copy and replay routes, and context preparation and recheck. Record actual outcomes and any environment limits in the local acceptance record. Commit the implementation, canonical docs and evidence together.

## Related docs

- [REL dependency port specification](rel-dependency-port-spec-2026-11-18.md)
- [Current planning roadmap](roadmap.md)
- [Development workflow](../development/workflow.md)

## Notes

The verification set uses the existing local test infrastructure and requires neither a live provider nor an active owner file.
