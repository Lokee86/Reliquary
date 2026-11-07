# F03-V02 historical source witness

## Purpose

The feature-gated f03_source_witness example reads the six disposable copies bound by the accepted F03-V01 receipt. It uses historical reader base c6010fd30dd52acb81c667f7aed99b831307e49f and writes private evidence outside the source root. The historical reader is isolated validation tooling.

## Overview

Before a run, verify the six copy pins and the restricted output parent's ownership, access rules and absence of reparse components. Supply the exact V01 custody receipt bytes. The exporter opens the six copies read-only through guards that bind the physical files and deny writers or removal on Windows. It checks the pinned hashes, sizes, exact tree membership, physical identities, one hard link per file and file stability before and after observation. An unknown frame or envelope body aborts the run.

    cargo run --example f03_source_witness --features history-oracle-fixtures --locked --offline -- --source-root <verified-copy-root> --expected-custody <accepted-V01-receipt> --output <fresh-private-output>

Use --resume to regenerate and byte-compare existing batches before continuing. A conflicting batch, unexpected output file or invalid batch stream fails. Use --verify-only to execute the same observation without writing output. Keep the command's exit, logs, complete artifact set, source patch, lockfile, binary and toolchain evidence alongside the run.

The original verified artifact schema is reliquary-f03-historical-state-v1. The coverage report is source-coverage.json and the manifest is source-witness-manifest.json. Each bounded, deterministic batch holds up to 128 rows, the predecessor artifact digest and a zero-based row ordinal. The manifest binds the exact coverage and batch digests. The frame stream records each physical slot's bounds, carrier, owner family group, original allocation mode or grouped range, global clock and payload digest. The global stream records every integer ordering point from zero through the source head with its known transaction time and accepted domain positions. The records stream binds exact decoded historical values by a stable-key digest and a canonical-value digest, with explicit pruned, tombstoned or inert status where exposed by the historical schema. REL owners also record their activity positions, accepted Freshness event cuts and FRS07 settlement authority. An exact historical-reader rejection for an intra-settlement trajectory is represented as unavailable. Unknown errors fail the run.

The exporter uses the old owner's typed read-only replay and existing canonical historical proof helpers. The native projections cover cumulative retained Archive, Memory, Entity, Relationship, Graph, Ego, Vector Generation, stored vector objects and Dream records where the historical helpers support them. Additional typed projections bind Archive first-applying publication coordinates, Community snapshots and lineage, Entity Resolution revisions and inert evidence, project correlations and file bindings, and the retained Insomnia/import journal. Physical frame hashes, native projection hashes and copy custody are separate evidence. The artifact contains hashes and typed coordinates, and excludes text and raw semantic payloads.

Exit code 2 means the scoped observation finished with INCOMPLETE admission. Exit code 1 means the observation failed. The current evidence requires completion of typed row/cut membership, independently authenticated scope, composed historical projections and external sidecar custody before it can freeze the conversion population. A family with no recognized carrier has unaudited semantic presence. A matching native head hash alone does not establish semantic equivalence across formats.

## Comparable cuts and retained membership

The extended exporter writes reliquary-f03-historical-state-v2 to a fresh protected output directory. The prior v1 witness remains a separate verified artifact. Each REL records all accepted Archive bindings in local order. A binding carries its exact global version, effective transaction timestamp, allocation or reservation-binding mode, and the first-applying Archive and global versions. The head digest uses the common Archive comparison projection. The separate Archive cut stream binds each ordered prefix from zero through the head, including an empty prefix at zero. Every binding first enters the history at its own local version.

For the six owners, clocked membership intervals carry the first and last inclusive cut of each retained native row. They cover Archive and its content closure, Memory, Entity, Relationship, Graph, MemoryGraph, Ego, and Vector Generation. The MemoryGraph cut set consists of qualifying Graph publications and cut zero. Revision, terminal, and immutable publication rows persist across later retained history. Current row intervals advance at each accepted revision or terminal. Referenced backing rows begin at their first retained publication. The native head fingerprint supplies the exact endpoint row set for every covered domain.

The clocked composition stream binds the global coordinate, transaction time, and applicable native domain prefixes at every global point. Each domain prefix hashes the ordered membership activations and removals through its local cut. Ego remains on its independent local clock. The composed clocked projection covers retained typed history and differs from a full physical-prefix replay of clock-neutral owner state. The evidence retains the scope in each composed row.

Run scripts/f03_verify_clocked_membership.py against the protected v2 output. Pass --baseline-output with the v1 directory to compare every prior frame, global, Freshness-event, and native record row while also independently recomputing all v2 clocked composition digests. Use scripts/f03_verify_archive_bindings.py to independently reconcile the raw source Archive reservations and publications with the v2 stream.

Pass --exhaustive-freshness as the first exporter argument to run every native Freshness activity cut. That run can use the historical optimized build profile and retains exact supported or typed unavailable states. The default output declares activity-cut proof coverage unaudited and retains the existing accepted event evidence. Common-schema semantic value coverage outside the Archive binding projection and complete clock-neutral as-of composition remain separate release gates.

## Independent evidence

The six-file custody check matched 124,664,599 bytes against the accepted F03-V01 pins. The archived oracle is bound to the current files by the archived CVAFRS07 promotion receipt and twelve independently hashed Git blobs. The three PHY blobs have identical before and after images. The three REL before-images match the immutable archived oracle and their after-images match the current V01 custody. Reproduce that bridge with scripts/f03_verify_archived_oracle.py.

The independent verifier in scripts/f03_verify_independent_repeat.py parses each physical frame directly, checks its source byte range and payload digest, verifies the accepted carrier and envelope bindings, reconstructs the pinned source hash and verifies every batch chain. It compares the coverage and all batch bytes from separate processes on distinct physical copies. The corrected exporter finalized both invocations with exit 2 and identical coverage plus 4,880 batches. They bind 266,507 physical frames, 264,402 typed record commitments and 91,997 global ordering points. The first richer invocations hit an inherited output-membership limit after writing their reports. Their outputs were kept separately as diagnostic evidence before the corrected runs.

Scripts/f03_compare_archived_projections.py independently compared 60 native semantic head fingerprints and reconstructed the archived ordering digest for all 91,997 global points. All six ordering digests and all 60 comparable heads matched, including the corrected output. The old oracle samples Archive snapshots for the 14-day and 28-day RELs and records six composed global cuts per REL. It uses a different query basis for archived Freshness event cuts. The ledger retains those scope distinctions and lists newer domains outside the archived oracle's projection.

The retained optimized archived test log matches the promotion receipt's SHA-256 and names all six files as verified. A fresh unoptimized replay verified the first REL before it was stopped. Post-run custody passed. That run supplies no fresh complete archived replay result. The isolated archive runner now selects the same optimized profile as the retained test.

Five disposable negative sets were rejected at custody admission with exit 1. The cases cover an unsupported format, a forged owner binding, a truncated tail, altered FRS07 authority bytes and a changed intermediate Archive turn. The checks leave positive copies and originals untouched. They establish custody rejection for the altered bytes.

Scripts/f03_verify_project_backing.py independently decoded the Git attachment bindings and matched the stable file IDs, paths, exact content hashes and lengths to the pinned index and regular Git blobs. The 14-day REL binds 95 files. The 28-day REL binds those 95 and four additional files. All 99 distinct blobs verified against fixture revision ec1a547f81294bb540b86bc54da875e1d9945ea3. The Ellis REL has no such binding.

The 14-day and 28-day REL files share an owner UUID, and their PHY files share a second UUID. These are separately processed variants of the same owners in the fixture repository. The Ellis REL header has an absent native scope. Preserve the repeated IDs and absent scope as source facts. The current gateway rejects simultaneous mounts that use the same owner ID, and the staging contract requires unique owner IDs in an active generation.

A scoped discovery found no queue, owner-operation, stream or external-processing sidecar files in the fixture and copy roots. Historical host state directories come from caller configuration. A local service, scheduled-task, process and startup inspection found no verified deployment binding for these fixtures. External operational state remains unknown.

The first execution receipt is target/f03-private/checked-witness-execution-receipt.json in the source worktree. The independent run is under .test-tmp/f03-archive-1efb937a23494549b1648f53dc3cf557/repeat-final-checked in the isolated FRS07 validation checkout. Its independent-match.json binds both manifests and the corrected exporter. The coverage digest is 37959b2c3efe3e46aef962d13b3e2e66bb66987112e430ce29af362f8a3e15e6. The independent match report digest is d1ceb9326e6e48ea8690f634eadce22de3d91f22c92175fa8e072bbb1bef021d. The original and repeat copy pins still matched after the runs.

The exact artifact hashes, command exits and full logs are retained under protected scratch directories. All 47 focused F03 tests passed. The full historical worktree architecture guard reports a pre-existing raw-file capability escape. The owner and historical read boundaries passed. Source admission remains incomplete. The shared interchange still needs exhaustive typed cut membership and composed historical projections, and activation requires a verified deployment population and operational-state binding.

## Related docs

- [f03 identity observation v1](f03-identity-observation-v1.md)
- [f03 preconversion physical census](f03-preconversion-physical-census.md)

## Notes

The older physical census remains separately callable as f03_preconversion_physical_census. Neither physical census nor the historical witness authorizes conversion or activation.
