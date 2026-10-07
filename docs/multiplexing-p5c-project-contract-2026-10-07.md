# P5-C project contract

The canonical `GatewayRuntime` now owns the trusted, pre-mount project lifecycle seam:

- `create_project_environment_rel`
- `inspect_project_adoption_source`
- `adopt_project_environment_rel`
- `validate_project_environment_cva`
- `reconcile_rel_file`

The lifecycle methods use the gateway lifecycle lock, exact mounted-file identity, and
the existing physical `MountClaim`; a path already mounted by this gateway is refused.
Creation and adoption continue to use the existing project repository preparation and
correlation code, so durable owner identity, project revision association, and storage
format are unchanged. Reconciliation is the unmounted path operation and is distinct
from mounted/quiesced reconciliation.

The APIs are trusted embedding-service controls. They are not instance/user endpoints,
do not expose mutable CVA handles, and do not construct or forward through
`ReliquaryRuntimeHost`. Host integration must perform its own path admission and
authorization before calling them.

Verification: the Phase 4 coverage guard and `git diff --check` pass. `cargo fmt --check`
and `cargo check` were started but exceeded the short third-party-cache time budget;
focused Rust tests were not started for the same reason. Existing mounted project tests
continue to cover exact-owner and grant enforcement; the new unmounted path gate fails
closed for mounted/locked files.

## Corrected fresh-create admission

A never-created REL has no typed owner header and cannot enter MountedFileIdentity::inspect. The trusted project-create path now relies on the domain Container create_new atomic no-overwrite contract; existing-source adoption/inspection/reconciliation still acquire exact physical owner mount claims. This correction was made after reviewing the initial implementation and is not yet runtime-test accepted.

## Purpose

Record the source contracts and verification for this isolated multiplexing lane.

## Overview

This record is branch-scoped. Source/API progress and executed runtime acceptance are separate gates; the parent parallel plan governs convergence.

## Related docs

- [Parallel execution plan](multiplexing-phase5-parallel-execution-plan-2026-10-06.md)
- [External consumer lane](multiplexing-phase5c-external-consumers-2026-10-06.md)

## Notes

Historical test results do not certify later integration revisions. Final release acceptance remains subject to the parent plan's consumer, deletion and release gates.

## P5-C continuation — 2026-10-07

Canonical result exports and module visibility compile under cargo check --locked. Warlock checkpoint 3806cb1 removes the old host consumer and passes 17 conversation regressions, two admission tests, the provider round-trip fixture, frontend build, docs and all 56 architecture rules. Its interrupted-generation transcript fixture still fails after reopen; the exact P3 generation record/P4 acknowledged-prefix read join remains a release gate. Final pins and the single Arcana identity are unchanged and unaccepted.

ManagedConversationState now carries an instance-local selected_leaf_node_id for historical viewing. select_managed_conversation_leaf validates exact conversation membership without moving a durable head. prepare_conversation_branch_at_leaf requires SubmitConversation and creates an explicit noncanonical branch through a dedicated ConversationBranch mutation family; ordinary metadata continues to require ManageRel. clear_managed_conversation_selection resets only this instance. P4 coverage classifies the new operations (146 APIs, 42 guard fixtures passed). Full architecture refresh timed out during Rust analysis before Pitlord.

The first full library test build exposed three inherited semantic fixture type mismatches (MemoryId versus SemanticNodeRef, GraphRelationKind versus SemanticGraphRelationKind); the fixture now compares the canonical typed values. Full runtime/CLI acceptance is still pending at this source checkpoint.

## Final verification at this checkpoint

- cargo fmt and cargo check --locked: PASS.
- P4 coverage: PASS, 146 public APIs, 42 guard fixtures.
- Structural and change-impact documentation checks: PASS.
- Full cargo test: FAILED in rustc 1.97.1, with an internal compiler error and Windows STATUS_STACK_BUFFER_OVERRUN; no library tests ran.
- Serial cargo test --lib --locked -j 1 with CARGO_INCREMENTAL=0 and RUST_MIN_STACK=33554432: FAILED while rustc linked Lore (OS missing-file error); no library tests ran.
- CLI fmt: PASS; locked check/test incomplete after the library compiler failure. Source remains ConfiguredRuntime-only.
- Architecture refresh: TIMED OUT during Rust analysis before Pitlord.

## Documentation impact

- Inspected: architecture, invariants, storage format, maintainer map, current limitations, roadmap and P5/P5-C contracts.
- Updated: P5 checkpoint, project/API contract, P5-A evidence structure and INDEX routing.
- Not affected: persisted REL/PHY formats, provider wire protocols and standalone CLI behavior.
- Compliance check: structural and change-impact checks pass; final changed record checks rerun before commit.
- Known documentation gaps: final accepted API/dependency and consumer runtime evidence remain open.

## Architecture impact

- Standards added or changed: new gateway methods classified by the existing P4 coverage policy.
- Ownership or boundary impact: instance-local historical read cursor; exact SubmitConversation branch mutation, distinct from ManageRel metadata authority; no old-host forwarding facade.
- Pitlord enforcement impact: coverage passes; full Reliquary evidence refresh timed out.
- Other verification impact: upgraded Warlock compilation and 17 conversation tests pass; three inherited semantic fixture type mismatches repaired.
- Known architectural gaps: acknowledged interrupted-prefix transcript join, stable busy/retry identity, multi-context/provider/P4 acceptance, final pins and library/CLI runtime verification.
