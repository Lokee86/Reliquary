# Switchboard extraction verification — 2026-10-27

## Purpose

Record the ownership cut and its verification against the [Switchboard extraction specification](switchboard-crate-extraction-spec-2026-10-27.md).

## Overview

The extraction starts from `124d497d` on `refactor/crate-first-restart`. Switchboard owns model identity, routing, provider transport, credential security and the raw configuration document, with no internal Reliquary dependencies. The product retains domain configuration, runtime composition, semantic services and the vector-facing endpoint contract. The unrelated Graph evidence edit and existing runtime-test artifacts were preserved.

The provider-facing `EmbeddingNormalization` is projected to Vector Store's `VectorNormalization` at the product endpoint boundary. This avoids a Switchboard dependency on Vector Store. Model configuration construction returns `ModelConfigError`, which the product config wrapper converts to its existing `ConfigError`; consumers constructing embedding route fields use the provider-facing normalization type. The supported schema-2 General reader remains at the persisted configuration boundary because configuration documentation explicitly promises it and external saved files cannot be assumed migrated. Current writes retain schema 3.

## Checkpoints

- S0: inventory identified the public facade, route matrix, documented schema-2 reader and vector normalization boundary. The Graph evidence edit remained outside the cut.
- S1: raw framing, replacement, credential codecs, encryption and key handling moved to Switchboard; product domain validation stays in `ReliquaryConfig`.
- S2: route models, authentication, codecs and the decision selector moved to Switchboard; the embedding normalization projection is explicit at the product vector endpoint boundary.
- S3: endpoint contracts, provider transports, parsers, device authorization and refresh moved; the lower tests exercise local transports with loopback access.
- S4: product runtime composes the lower types and the root facade reexports them; vector-facing behavior remains at its endpoint seam.
- S5: functional acceptance passed as recorded below. Fresh architecture evidence and documentation ownership enforcement remain unverified.

## Verification

- `cargo test -p reliquary-switchboard --locked --offline -j 1`: 44 passed with local loopback access and repository-local temporary files. This includes route precedence, localhost provider/device-auth fixtures, malformed document and failed replacement checks, model codec byte fixtures, and credential object-ID authentication.
- `cargo check --workspace --locked --offline -j 1`: passed.
- Detached CLI tests: 32 passed with local loopback access.
- `cargo test --lib --locked --offline -j 1`: 1,543 passed, 5 ignored, with local loopback access and repository-local temporary files.
- `cargo fmt --all -- --check` and detached CLI formatting check: passed.
- `ddocs check --indexes --links`: passed after adding this evidence.
- `python scripts/check_architecture.py` without refresh: source guards and 25 Pitlord rules passed against the existing older Arcana graph; this is not fresh-graph evidence. A refresh was stopped after its Rust analyzer reached about 9 GB and free physical memory fell to about 1.6 GB.
- The Pitlord documentation guard against `origin/main` failed with 344 unmapped files using the older graph (953/1,297 mapped). It cannot establish current ownership coverage from that graph. A current-change check also identified `docs/cli.md` as needing an update; the CLI documentation was updated. The fresh Arcana graph and documentation-ownership gate are unclaimed.

## Independent closure verification (2026-10-27)

The post-commit audit of `449d4afe` found one canonical Switchboard owner for models, credential encryption, raw configuration, provider transports and decision routing, with no direct Reliquary crate dependencies. The existing `tools/pitlord/semantic.json` policy now maps Switchboard's source to its own architectural area and extends the authored-source ownership scope; no new analyzer was created.

- `cargo check --workspace --all-targets --locked --offline -j 1`: passed.
- `cargo test -p reliquary-switchboard --locked --offline -j 1`: 44 passed, 0 failed.
- Detached CLI `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`: 32 passed, 0 failed.
- `cargo test --lib --locked --offline -j 1`: **1,547 passed, 0 failed, 5 ignored** on the then-current checkout (which included uncommitted Chronos correction work).
- `cargo fmt --all -- --check`: passed.
- `python scripts/check_architecture.py` without refresh: source guards, policy validation and all 25 Pitlord rules passed using the **older Arcana snapshot**; this is not a fresh-graph check.
- The Switchboard specification's four missing documentation-policy sections were added; 31 existing missing-section findings remain exclusively in other extraction documents, so full and changed-from checks still fail.
- The installed Pitlord rejects the documented `pitlord docs` command as unknown. A previous Arcana refresh was aborted near 9 GiB analyzer memory consumption; a fresh resource-intensive scan was deliberately not repeated. Fresh-graph and documentation-ownership enforcement remain unverified.
- The six-file CVAFRS07 canonical corpus gate remains a separate product-level acceptance constraint.

The retained Graph evidence change and concurrent Chronos work were not included in Switchboard's documentation/policy closure.

## Related docs

- [Model and provider runtime](subsystems/model-runtime.md)
- [Configuration](configuration.md)
- [Architecture](architecture.md)

## Notes

The original documentation-policy scan reported 35 missing-section findings; four originated in the Switchboard spec and are now corrected. The remaining 31 are in other extraction documents. These pre-existing policy findings and the separate CVAFRS07 six-file gate are not Switchboard implementation defects, but still block repository-wide formal acceptance.