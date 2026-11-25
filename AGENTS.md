# Contributor instructions

## Purpose

This file gives repository contributors the current product boundaries and the routine change workflow. The [maintainer map](docs/architecture/maintainer-map.md) routes changes to owners, documentation and verification surfaces.

## Overview

The root package, `reliquary-memory`, is the supported consumer boundary. Keep product APIs opaque and authorization scoped. Container and durable stores remain behind owner-session, Application and Gateway responsibilities described in [architecture documentation](docs/architecture/system.md). Switchboard owns the binary configuration format, credential protection and provider transport.

## Change workflow

Documentation is part of the implementation. Update the canonical owner and affected maintainer, coverage and behavioral-contract maps in the same change.

1. Read the canonical owner page and current behavior tests before changing a responsibility. Start at [docs/INDEX.md](docs/INDEX.md); use [architecture/INDEX.md](docs/architecture/INDEX.md) for boundaries and [reference/INDEX.md](docs/reference/INDEX.md) for exact interfaces.
2. Keep code, tests and documentation aligned. Update the canonical page and applicable coverage/contract mappings for changed public behavior, persistence, CLI grammar, recovery or security.
3. Prefer a focused change at the owning layer. Do not expose store handles or containers through Root, route around Gateway grants, or make CLI code implement product policy.
4. Run the relevant targeted tests and repository checks documented in [development workflow](docs/development/workflow.md). The package dependency guard is `python scripts/check_architecture.py`. Use the repository's required offline Cargo commands when the change warrants them.
5. Review the diff for unrelated changes and commit the completed unit with its verification evidence.

## Product invariants

- Runtime admission supports current REL 2.1 and PHY 2.1. Old-format conversion is an offline, separately scoped operation.
- REL and PHY owner files are independent owners. Access to one does not grant access to another.
- Only authenticated Instances with owner-specific grants perform hosted operations. Revocation is checked by the operation paths; filesystem paths and identifiers are not credentials.
- Writes publish through the owning session and its durable commit boundary. Do not describe a completed in-memory step as durable until the product operation reports success.
- Credential values and master keys must not appear in logs, CLI arguments, JSON reports, examples, or committed configuration.
- Native import resume verifies the accepted embedding compatibility profile and uses the current inference routes. Other workflows follow their recorded bindings.
- Retrieval, browsing and CLI display do not prove external consumption or earn accepted-use credit.
- Copy maintenance is create-only: destination paths must not be replaced.

## Verification and safety

Use `python scripts/check_architecture.py` after package boundary changes. Use `cargo fmt --all --check`, `cargo check --workspace --locked --offline -j 1`, and `cargo test --workspace --locked --offline -j 1 -- --test-threads=1` where applicable. CLI changes also use `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`. Follow the repo-specific documentation and architecture gates in the development workflow.

Do not run a real provider probe, mutate a source corpus, or change credential/configuration secrets as routine test setup. Never place temporary files or caches in `C:\tmp`; keep them in the active worktree and use `.pytest\_tmp` for pytest or `.go-cache` for a custom Go build cache.

## Related docs

- [Development workflow](docs/development/workflow.md)
- [Maintainer map](docs/architecture/maintainer-map.md)
- [System architecture](docs/architecture/system.md)
- [CLI reference](docs/reference/cli.md)

## Notes

Historical rebuild instructions and acceptance records remain under `docs/` as dated evidence. The engineering-standards remediation specification is a governance plan, not a statement that implementation or compliance gates have been accepted.
