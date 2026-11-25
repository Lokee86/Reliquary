# Perception host integration acceptance

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the shared Core revision and local acceptance for native Jev and typed Observation recall in Hermes and Mercury.

## Overview

Both hosts use Core `96c41a2287eac2a8b48a1a6443b339423a4a8ca3`. Hermes exports the tracked source into its vendor tree. Mercury pins the same Git revision in its manifests and lockfile. The checks used Windows, Rust 1.100.0, local scripted model endpoints, and disposable profiles.

## Verified behavior

| Boundary | Result |
| --- | --- |
| Shared Core | All 44 Switchboard tests and the focused Archive and Echo checks passed. The final pin passed the mounted Perception lifecycle and both exact-delivery checks. Those checks covered native Verify and Contribution, dependent review, mixed delivery, Observation-only confirmation and replay, and stale standing. |
| Hermes | The signed fixture verified a claim through native Jev and recalled it alone and with a Memory. The exact citation survived restart. A terminal copied source blocked a delayed return. The Windows packaged-host smoke opened a pre-Perception pair, reopened a REL write, and preserved the PHY hash. The native suite passed 129 tests with one existing renderer characterization ignored. |
| Mercury | The pinned Desktop and Gateway passed locked Windows developer builds. The isolated Gateway profile verified an Observation through native Jev, returned it with its citation, and withheld it after the source was archived. The same fixture passed its Memory, Archive, Echo, and profile reopen checks. |

## Local candidates

- Hermes source `2edfbdb6651eb6a6a0b18d7c271736694f3a3810` produced `reliquary-hermes-windows-x86_64-2edfbdb-test.zip` under `host/target/perception-package-2edfbdb/`. The ZIP SHA-256 is `aad54b4c532db50e28e7ea0715a999569c7aa15bb3a6c0530caddbc9dc112503`. The Windows-only ZIP contains the debug executable verified by the signed test. Each Linux and macOS executable still needs to be rebuilt and attested by the existing native workflow.
- Mercury source `60468e37e2258ec1836b0cf66338d0cd699d7d8f` uses the shared Core pin. Local Cargo resolved the unpublished pin from the isolated Core checkout. A fresh network build requires the Core commit on the canonical remote. Release distribution uses the existing notice and packaging workflow. The local Windows builds and Gateway test passed with MSVC warning LNK4098. A repository-wide format check found existing layout differences, including unchanged `crates/desktop/build.rs` at the base revision. The committed changes passed the Git whitespace check.

## Related docs

- [Host integration spec](../planning/perception-host-integration-spec-2026-11-24.md)
- [Same-REL release evidence](perception-same-rel-2026-11-24.md)
- [Configured runtime](../configured-runtime.md)
- [Hermes verification](https://github.com/Lokee86/reliquary-hermes/blob/main/docs/verification.md)

## Notes

The native route uses `typesafe/jev-1.13`. The host fixtures verify routing, evidence disclosure, and persistence. The original recorded trials establish model behavior. The separate import session retains its own checkout, executables, configuration, and owner files.

The shared Core keeps Mercury’s `ArchiveVectorPublication` type. The staged native import receipt is exposed as `NativeImportArchiveVectorPublication`. [Core API](../public-product-api.md#native-and-copied-source-import-contracts)
