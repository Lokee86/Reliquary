# F03-V02 historical source witness

## Purpose

The feature-gated f03_source_witness example reads the six disposable copies bound by the accepted F03-V01 receipt. It uses historical reader base c6010fd30dd52acb81c667f7aed99b831307e49f and writes private evidence outside the source root. The historical reader is isolated validation tooling.

## Overview

Before a run, verify the six copy pins and the restricted output parent's ownership, access rules and absence of reparse components. Supply the exact V01 custody receipt bytes. The exporter opens the six copies read-only through guards that bind the physical files and deny writers or removal on Windows. It checks the pinned hashes, sizes, exact tree membership, physical identities, one hard link per file and file stability before and after observation. An unknown frame or envelope body aborts the run.

    cargo run --example f03_source_witness --features history-oracle-fixtures --locked --offline -- --source-root <verified-copy-root> --expected-custody <accepted-V01-receipt> --output <fresh-private-output>

Use --resume to regenerate and byte-compare existing batches before continuing. A conflicting batch, unexpected output file or invalid batch stream fails. Use --verify-only to execute the same observation without writing output. Keep the command's exit, logs, complete artifact set, source patch, lockfile, binary and toolchain evidence alongside the run.

The artifact schema is reliquary-f03-historical-state-v1. The coverage report is source-coverage.json and the manifest is source-witness-manifest.json. Each bounded, deterministic batch holds up to 128 rows, the predecessor artifact digest and a zero-based row ordinal. The manifest binds the exact coverage and batch digests. The frame stream records each physical slot's bounds, carrier, owner family group, original allocation mode or grouped range, global clock and payload digest. The global stream records every integer ordering point from zero through the source head with its known transaction time and accepted domain positions. The records stream binds exact decoded historical values by a stable-key digest and a canonical-value digest, with explicit pruned, tombstoned or inert status where exposed by the historical schema. REL owners also record their activity positions, accepted Freshness event cuts and FRS07 settlement authority. An exact historical-reader rejection for an intra-settlement trajectory is represented as unavailable. Unknown errors fail the run.

The exporter uses the old owner's typed read-only replay and existing canonical historical proof helpers. The native projections cover cumulative retained Archive, Memory, Entity, Relationship, Graph, Ego, Vector Generation, stored vector objects and Dream records where the historical helpers support them. Additional typed projections bind Archive first-applying publication coordinates, Community snapshots and lineage, Entity Resolution revisions and inert evidence, project correlations and file bindings, and the retained Insomnia/import journal. Physical frame hashes, native projection hashes and copy custody are separate evidence. The artifact contains hashes and typed coordinates, and excludes text and raw semantic payloads.

Exit code 2 means the scoped observation finished with INCOMPLETE admission. Exit code 1 means the observation failed. The current evidence requires completion of typed row/cut membership, independently authenticated scope, composed historical projections and external sidecar custody before it can freeze the conversion population. A family with no recognized carrier has unaudited semantic presence. A matching native head hash alone does not establish semantic equivalence across formats.

## Related docs

- [f03 identity observation v1](f03-identity-observation-v1.md)
- [f03 preconversion physical census](f03-preconversion-physical-census.md)

## Notes

The older physical census remains separately callable as f03_preconversion_physical_census. Neither physical census nor the historical witness authorizes conversion or activation.
