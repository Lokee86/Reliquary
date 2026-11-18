# Scope-free REL creation and host roles

Parent index: [Planning](./INDEX.md)

## Purpose

Finish REL homogeneity in 2.1. A new REL should need only a destination path. An existing valid REL should be selectable as General memory or a dedicated project REL in Hermes according to the authorized host configuration.

## Overview

The [dependency port](../acceptance/rel-dependency-port-2026-11-18.md) restored context inheritance by owner ID. The remaining class behavior is in creation and host admission. The Root constructor still accepts a scope, the CLI defaults it to `Project`, and Hermes requires a `Project` header for dedicated memory. Hermes Python setup also expects dedicated creation to return that scope.

ADR 0029 defines uniform REL semantics. ADRs 0037 and 0038 keep owner identity, mount authority and context in Reliquary. The accepted decisions are recoverable at `f431401e` under `docs/decisions/`. This delivery covers creation and role admission. ADR 0029's optional free-form display label is metadata work with its own contract.

## Core creation and file compatibility

- The public constructor is `Rel::create(path)`. The CLI grammar is `reliquary rel create PATH`. Both go through the same owner-session creation path. Allocate a new owner ID using the current UUID representation and write the existing neutral `Unspecified` scope byte in the REL 2.1 header.
- Change the current owner-session creation entry point to `RelWriter::create_current_path(path, owner_id)`. Keep the physical scope-aware initialization primitive for legacy format fixtures. Identity-preserving maintenance continues to use the source's container identity. Production new-owner callers enter through the neutral path. Retire the public creation scope argument, CLI `--scope` parser/default and creation-only conversions. An old flag fails with the normal CLI input error before touching the destination.
- Keep the four existing scope values readable. `OwnerInfo.scope`, inspection and CLI JSON continue to report the stored value with the existing spelling. Preserve the 2.1 header codec, its identity comparisons and scope bytes in copy and prune proofs. Ordinary open, verify, mount and setup leave existing file contents and owner IDs byte-for-byte. Current create-only destination checks and writer leases govern publication.
- The neutral header value grants no capabilities. The owner file kind, authenticated grants and explicit repository association continue to govern access and project features. REL dependency declarations and existing semantic history retain their current replay rules.

## Hermes setup, startup and routing

- In `host/src/setup_create.rs`, General and dedicated project roles use the same `Rel::create(path)` inside the existing staged publication path. Preserve request validation, explicit project selection, destination protections, verification and returned owner ID. PHY creation uses its existing path.
- In `host/src/setup_validation.rs` and `host/src/registry.rs`, admit a verified REL of any recognized 2.1 scope for either memory role. Remove the shared `validate_rel_scope` helper and the `project_rel_scope` diagnostic. Preserve wrong-kind, missing/corrupt-file, duplicate-owner, writer-conflict and persisted-topology checks.
- In `src/reliquary_hermes/setup.py`, verify a neutral scope in successful REL creation receipts for both roles. Keep legacy values valid in general owner reports. The setup request and report shapes, including the reported `scope` field, are preserved.
- General and dedicated roles come from `owners.json` and trusted Hermes project attribution. Persist and recover the same accepted session routes and owner IDs. Keep owner grants, profile and active-PHY isolation, repository bindings, and topology-change/restart behavior in their existing owners. Context inheritance continues to use declared REL dependencies.

## Migration and integration boundary

Cut the documented Rust and CLI creation inputs to their scope-free forms and migrate callers directly. The old Rust call loses its second argument. The documented replacement for a command with `--scope` is `rel create PATH`. Existing REL contents and the public read-side metadata and setup wire shapes are the external compatibility boundaries.

The migration window runs from the constructor change until Core, its in-repo consumers, and the matching Hermes native/Python pair pass integration. Tests or callers still using the old constructor can be temporarily red inside that window. The Python setup and native host must be built and used from the same completed Hermes revision. The native binary is built separately today, so the upgrade instructions identify the matching binary before operators use setup creation.

Refresh Hermes's source-only `vendor/reliquary` snapshot to the accepted scope-free Core commit. Current baselines are Core `36c6383a`, Hermes `2723c82`, and the Hermes vendored fork at `dca06ffa` plus its documented changes. Verify the vendored fork's graph projection and partial Community decoding against the target. The four hosted-inference and vector-status commits named in `vendor/reliquary/SOURCE-PROVENANCE.md` are already in Core's history. Preserve their behavior, the approved local Lodestone subset and its manifest redirects, the patched `quinn-proto` source and license files, and the source-only packaging layout. Record the exact source revision and remaining vendor changes in provenance, and keep the affected Cargo locks resolvable offline.

## Completion

1. Root and CLI create and reopen a neutral REL with its new durable ID. The retired flag is rejected without creating a file. Existing typed 2.1 files verify and preserve their original bytes, IDs and reported scope.
2. One matched Hermes build creates neutral RELs in both roles and admits legacy-scoped and neutral RELs through setup and startup. The configured session routes remain stable across reattach. Authorization, duplicate and wrong-kind checks still apply.
3. The reviewed vendor snapshot builds through the host's public Root dependency, including the dependency port and the retained graph and hosted-inference behavior. Current docs describe the creation input cut, paired upgrade and file compatibility. The integration checks and measured evidence are defined in the [execution plan](rel-homogeneity-plan-2026-11-18.md).

## Related docs

- [Execution plan](rel-homogeneity-plan-2026-11-18.md)
- [Original REL homogeneity roadmap](../rel-homogeneity-and-dependency-roadmap-2026-11-10.md)
- [Root API](../public-product-api.md)
- [CLI reference](../reference/cli.md)
- [Storage architecture](../architecture/storage.md)

## Notes

Acceptance uses disposable synthetic owners. File or binary changes in a running profile are outside the acceptance setup.
