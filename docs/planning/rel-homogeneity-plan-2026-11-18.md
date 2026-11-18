# Scope-free REL creation execution plan

Parent index: [Planning](./INDEX.md)

## Purpose

Implement the [homogeneity specification](rel-homogeneity-spec-2026-11-18.md) in the Core and Hermes repositories and verify one matched integration. Start with Core `36c6383a` and Hermes `2723c82`.

## Overview

Work in dependency order. Confirm the changed behavior at the existing creation and setup seams, then run the applicable repository gates once. Keep the synthetic owner examples shared across the compatibility checks.

## Work packages

| Work package | Depends on | Deliverable and exit check |
| --- | --- | --- |
| 1. Capture the baseline | Current commits | Record the creation consumers, Hermes setup and startup checks, and divergence between the vendored fork and Core. Capture tiny pre-cut Organization, Project and Connection REL 2.1 files and record their IDs, scope and SHA-256. Keep generated owner files and hashes in the local test area. Use the same synthetic bytes for the final Core and Hermes compatibility checks. |
| 2. Cut Core creation | 1 | Change [Root creation](../../src/owner.rs), the [owner writer](../../crates/reliquary-owner-session/src/session/rel_writer.rs) and [CLI creation](../../cli/src/commands.rs) together. Migrate existing workspace, CLI and external-consumer callers plus the [G4 acceptance driver](../../scripts/run_g4_acceptance.ps1). Use the existing lower-level scoped creation for legacy format fixtures where needed. Update API, CLI, operator and applicable architecture and contract docs. Verify neutral creation, legacy reopen and old option refusal. Run the Core integration gate once and commit the verified Core unit for the host vendor pin. |
| 3. Integrate Hermes with Core | 2 | Refresh the reviewed source-only vendor snapshot and locks from the accepted Core SHA. Reconcile the graph and packaging deltas from the provenance manifest. Change Rust setup creation, read-only preflight and registry admission, and update Python setup's creation receipt check in the same Hermes unit. Update the canonical local-service, protocol, and verification docs. Compile the host against Root and verify both create roles through setup. |
| 4. Verify and land the pair | 3 | Run the focused matrix below with the freshly built host and matching Python adapter. Run the Hermes integration gate once. Record actual results, source pin, binary identity and fixture measurements in dated evidence. Commit the Hermes unit with the pin and evidence and record both local revisions. |

## Focused verification

**Core and CLI.** Extend the existing [owner creation and isolation tests](../../tests/g4_public_product.rs) and [CLI integration tests](../../cli/tests/cli.rs). Check `Rel::create(path)`, `rel create PATH` and fresh reopen through verified metadata. Check `Unspecified`, stable ID, create-only behavior, and refusal of `--scope` before file creation. Verify the pre-cut witnesses' identities, reported scopes and hashes. Keep the existing exact identity and copy/prune tests on their normal paths. Recompile the separate external-consumer workspace so it exercises the new public signature.

**Hermes.** Extend `host/tests/setup_validation.rs` and `host/src/registry_tests.rs`. For durable automated legacy cases without a baseline executable, use the tiny frozen synthetic owner bytes as test-only source fixtures. Copy them to fresh temp paths for the checks. Use the canonical constructor for ordinary new fixtures. Test the legacy scope values in both roles with independent temporary profiles so duplicate-owner rules still apply. Reuse the existing tests for wrong kinds, duplicate owners, path and publication failures, grants, and persisted routing. Update the former wrong-scope case to assert acceptance. Verify General and dedicated `setup-create` both report neutral scope, and exercise the Python setup receipt path with the new response.

**Matched smoke.** Use one isolated setup-and-reopen run with the new native binary and Python adapter. Select typed legacy owners, start the registry, and verify an accepted session resolves to the configured REL after restart. Compare the old file hashes and owner IDs. Create a neutral REL through dedicated setup and validate it in the General role in a fresh profile. Confirm that the refreshed vendor keeps the existing exact-cut graph projection and hosted inference contract through the native host tests.

## Integration and evidence

- Run the applicable formatting, locked offline Core workspace, CLI and external-consumer checks once. Use the normal [development workflow](../development/workflow.md), including the package architecture and committed documentation checks for affected surfaces. Compile the former creation callers and search the active source and scripts for retired uses.
- Run Hermes's locked offline `host/Cargo.toml` suite and its prescribed Python checks with the audited Hermes checkout and worktree-local temporary state. Check the committed Hermes documentation against its starting revision. Use one native test build for the smoke and record any environment-limited check.
- Verify the final source-only vendor inventory, exact Core pin, expected graph and hosted-inference entry points, and lockfile changes. Record the scope transition in both repos' operator references and the matched-binary requirement in Hermes's upgrade instructions.

## Related docs

- [Homogeneity specification](rel-homogeneity-spec-2026-11-18.md)
- [Dependency port acceptance](../acceptance/rel-dependency-port-2026-11-18.md)
- [Current planning roadmap](roadmap.md)
- [Development workflow](../development/workflow.md)

## Notes

Use disposable profiles and owners. Keep the verification centered on creation, role admission, file identity and the existing vendor integration contract.
