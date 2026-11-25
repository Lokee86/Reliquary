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

## Shared hybrid recall follow-up

Core `8d0b432dafc023b31dade2c06c46b75c1ab266bf` replaced the separate Observation keyword search with the same ranked population and agentic selection as Memories. The disposable REL fixture reopened a verified claim with its document embedding, matched a paraphrase with zero lexical overlap, and won the shared one-result limit against a lexical Memory. The exact citation remained intact. When the claim went into review, passive hybrid, semantic, and agentic recall withheld it. The mounted worker indexed both an existing claim and a newly verified one, using document embeddings distinct from the receptor vectors. [Paraphrase regression](../../crates/reliquary-runtime-gateway/src/instance_retrieval/observation_checks/tests.rs) · [Managed worker test](../../crates/reliquary-runtime-gateway/src/instance_perception/tests.rs)

The final Core revision passed all 15 focused Gateway retrieval tests and the mounted Perception test. The Application agentic suite passed 11 tests. Mercury’s locked Gateway compile check included its native WebSocket fixture. The Hermes native test target compiled and linked on the locked test profile. Its protocol and provider checks passed 124 tests.

The source pins and Windows artifact listed above belong to the first integration. This follow-up pins the new Core in Hermes `d0601047` and Mercury `28a5e11`. Hermes applies its normal result limit to the combined return. Both hosts rely on Core for current Observation standing and retain their copied-source verification.

## Host paraphrase and review acceptance

- Hermes `662a8763ab935a7a13e57ad0669e7a0b01380e76` passed the signed paraphrase test in 55.02 seconds on a disposable REL and PHY. The zero-overlap paraphrase returned the verified Observation and exact copied-source citation across a restart. The fixture then admitted a material challenge and left Verify unresolved. The same paraphrase returned the supporting Memory while the Observation was in review. The fixture also passed review recovery and the delayed source fence.
- Mercury `406baebb909eff1a1024c1fb98c4cde66f410165` passed the native Gateway WebSocket fixture in 59.26 seconds on a disposable profile. The zero-overlap paraphrase returned the same Observation revision and exact support citation as the keyword query. A fresh authorized turn returned the same supporting Memory while a challenge-bound Verify request was held. Releasing Verify restored the exact Observation before the terminal source check.

Both runs used scripted local inference and Core `8d0b432dafc023b31dade2c06c46b75c1ab266bf`. The signed fixtures cover host delivery as well as Core selection. The separate processing profile stayed outside the test setup.

## Related docs

- [Host integration spec](../planning/perception-host-integration-spec-2026-11-24.md)
- [Same-REL release evidence](perception-same-rel-2026-11-24.md)
- [Configured runtime](../configured-runtime.md)
- [Hermes verification](https://github.com/Lokee86/reliquary-hermes/blob/main/docs/verification.md)

## Notes

The native route uses `typesafe/jev-1.13`. The host fixtures verify routing, evidence disclosure, and persistence. The original recorded trials establish model behavior. The separate import session retains its own checkout, executables, configuration, and owner files.

The shared Core keeps Mercury’s `ArchiveVectorPublication` type. The staged native import receipt is exposed as `NativeImportArchiveVectorPublication`. [Core API](../public-product-api.md#native-and-copied-source-import-contracts)
