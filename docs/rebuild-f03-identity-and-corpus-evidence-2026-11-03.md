# F03 identity and retained-corpus evidence — 2026-11-03

Parent index: [Documentation index](INDEX.md)

Parent: [Selective-reuse capability plan](architecture/implementation-rebuild-capabilities-2026-11-03.json), item `F03`.

**Status:** F03 evidence capture complete with the exact FRS07 blocker recorded. Six pinned-reference identity vectors and the six-file cohort identities/head expectations are captured. The pinned F01 runtime rejects all three REL fixtures because it lacks the promoted CVAFRS07 Freshness reader; all three PHY fixtures reopen. A separate historical FRS07 runtime later reopened all six copied fixtures and matched their pinned heads, as described below. No conversion or corpus mutation is authorized by this evidence note.

## Purpose

Capture stable identity and source-corpus evidence for the rebuild.

## Overview

The source corpus remains unmodified. Historical replay and owner heads are verified on copies; complete semantic fingerprints and conversion remain unverified.

## Reference and gate

- Pinned detached reference: `C:\!bin\workspace\Reliquary-reference`, commit `f62b1fb44ca7f0bd8351e3a30fb5d76211f85102`.
- The gate is the ignored `promoted_canonical_corpus_reopens_after_runtime_retirement` test in [`tests/current_format_boundary.rs`](../tests/current_format_boundary.rs). It verifies source hashes before copying, rechecks copy hashes, opens only disposable copies, checks expected owner heads, and verifies each copy's bytes and modification time remain unchanged.
- Fixture root: `C:\!bin\workspace\reliquary-fixtures\local\authoritative` (present at inspection).
- The original fixtures were not opened or written by this task. The pinned-reference gate result was reported as **3 PHY pass / 3 REL fail**; each REL failure is `unsupported Freshness prototype format; explicit migration is required`. This matches the independently recorded copy-only gate outcome. The present crate-first source has the `CVAFRS06` semantic-settlement reader, while the promoted cohort is CVAFRS07.

## Six authoritative fixture identities

These are the SHA-256 source identities and expected owner heads embedded in the pinned gate. REL head order is latest-global, Archive, Memory, Entity, Relationship, Graph, Memory-Graph, Vector, REL-turn count. PHY head order is latest-global, Memory, Entity, Relationship, Graph, Memory-Graph.

| Fixture | Owner | SHA-256 | Expected heads | Admission |
|---|---|---|---|---|
| `chatgpt-first14d/project.prj.rel` | REL | `a02c8696db754c53a73c896133b06df9828a32923aee610c9f918a16698f76d0` | `[18938, 7526, 3348, 150, 0, 7913, 7913, 1, 5857]` | Blocked: unsupported CVAFRS07 Freshness |
| `chatgpt-first14d/user.phy` | PHY | `7e690b4853afe0936702dc023f511dd7c78ffaae3e18e8e66f6ab1c6891a6c63` | `[1088, 421, 0, 0, 667, 667]` | Passed |
| `chatgpt-first28d/project.prj.rel` | REL | `76c15780bd7cfe7e3083dbff5684e236a8fd2090fb0a82ec3d66529d14307d87` | `[52804, 21068, 5727, 671, 0, 25337, 25337, 1, 16465]` | Blocked: unsupported CVAFRS07 Freshness |
| `chatgpt-first28d/user.phy` | PHY | `32970f3975bfcd6de0fcdb78df9c80c78b76787832f528c1b09fd75dfa4f501d` | `[2215, 490, 35, 0, 1690, 1637]` | Passed |
| `rhelm-david-r-ellis-pre-relationship/project.prj.rel` | REL | `cbc38d055936fc5702a17c4d569557d8c35e8887d1946c5562ba419038de6ee3` | `[11725, 3763, 1452, 199, 0, 6309, 6309, 2, 2972]` | Blocked: unsupported CVAFRS07 Freshness |
| `rhelm-david-r-ellis-pre-relationship/user.phy` | PHY | `58ac7872215e126749f024fc742828abc9a8a8f84775b39c0706d638392ce9e0` | `[5221, 976, 86, 0, 4159, 3889]` | Passed |

The gate records the source digest and expected per-domain owner heads as its current accepted-record inventory. REL admission does not reach those head assertions because Freshness replay fails first. Therefore the three REL head tuples remain pinned expected values, not newly validated observations for this reference run.

## Stable identity domains found in source

The source-backed migration matrix identifies six hash-derived IDs whose preimage contracts must survive the rebuild. The byte-domain prefixes below are the exact current source constants. The reference must retain these preimages; output IDs are SHA-256 over the ordered components shown in the source functions.

| ID | Source function | Preimage components after domain prefix |
|---|---|---|
| `FileId` | `reliquary-archive/src/file_store.rs::file_id` | content digest, little-endian byte length, length-prefixed filename, length-prefixed MIME type; domain `CVAFILE1-ID\0` |
| `PackedVectorId` | `reliquary-vector-store/src/store.rs::packed_vector_id` | little-endian dimensions, scalar tag, vector bytes; domain `CVA-PACKED-VECTOR-V1\0` |
| `CompatibilityProfileId` | `reliquary-vector-store/src/profile.rs::compatibility_profile_id` | dimensions, normalization tag, probe-suite version, policy version, ordered mode/vector probes; domain `CVA-COMPATIBILITY-PROFILE-V1\0` |
| `ArchiveVectorId` | `reliquary-vector-store/src/archive.rs::archive_vector_id` | packed-vector ID, ordered ArchiveFragment IDs; domain `CVA-ARCHIVE-VECTORS-V1\0` |
| `MemoryVectorId` | `reliquary-vector-store/src/memory.rs::memory_vector_id` | compatibility-profile ID, packed-vector ID, ordered MemoryBody IDs; domain `CVA-MEMORY-VECTORS-V1\0` |
| `VectorGenerationId` | `reliquary-vector-store/src/generation.rs::vector_generation_id` | compatibility-profile ID, ArchiveVector ID, little-endian source version; domain `CVA-VECTOR-GENERATION-V2\0` |

These contracts and implementation seams are also listed in [the format and identity migration proof matrix](cva-rel-migration-source-and-proof-matrix-2026-11-03.md). The next section records concrete deterministic input/output pairs from the pinned reference for all six domains. Rebuilt-implementation parity assertions remain a later rebuild acceptance task.

### Executed reference vectors

The following fixed vectors were executed through the pinned reference libraries at commit `f62b1fb44ca7f0bd8351e3a30fb5d76211f85102`. A small standalone runner depended on the pinned `reliquary-archive` and `reliquary-vector-store` path packages; `CARGO_TARGET_DIR` pointed to `C:\!bin\workspace\Reliquary-crate-first\.f03-target`, so compilation emitted no build files into the reference worktree. The runner called the actual exported reference functions and printed each 32-byte ID as lowercase hexadecimal.

| ID | Fixed input | Reference output (32 bytes, hex) |
|---|---|---|
| `FileId` | `build_stored_file_from_content_id(filename="report.txt", mime_type="text/plain", content_id=[0x10; 32], byte_length=123)` | `f88c8992a4741a09f22417bc8fe4ca378ba21ab9d66ae19caeab0e552f6c7a88` |
| `PackedVectorId` | `VectorSchema(dimensions=3, scalar=F32)`; bytes `01 02 03 04 05 06 07 08 09 0a 0b 0c` | `358743b8ccfdeece4cef22af3df6b76da9b5f474a2595b99fe598efcf47a5419` |
| `CompatibilityProfileId` | dimensions `3`; `L2`; probe-suite `1`; policy `2`; ordered refs: Query `[0.5, -1.25, 2.0]`, Query `[-0.25, 0.75, 1.5]`, Document `[1.0, 0.0, -1.0]`, Document `[0.125, 0.25, 0.5]` | `fb87924dad756b01c75e8925a8fefe999727124ef2c81f5349c98116f1f2a37a` |
| `ArchiveVectorId` | packed ID `[0x11; 32]`; ordered fragment IDs `[0x22; 32]`, `[0x33; 32]` | `957cce787993df75646ed70892f9db461c207c67c55605391e49c23361335833` |
| `MemoryVectorId` | profile ID = preceding `CompatibilityProfileId`; packed ID `[0x11; 32]`; ordered body IDs `[0x44; 32]`, `[0x55; 32]` | `c592f25bd556512a331f404469e4a31a48c9da08291ca20b92d2a515f791b9ee` |
| `VectorGenerationId` | profile ID = preceding `CompatibilityProfileId`; ArchiveVector ID = preceding `ArchiveVectorId`; source version `0x0102030405060708` | `803cbae7840970444914e0be23cf1faaf4bda07744444200dbc96d29703c4c3e` |

These are deterministic reference vectors, not corpus-observed IDs. The generation source version is passed as a `u64`; its reference hash function encodes it little-endian. The build completed successfully offline against the pinned path dependencies and returned exit code 0. It emitted only an existing dead-code warning from the reference Archive history codec; it did not run the corpus gate.

## Exact F03 disposition

- **Captured:** pinned reference revision; six fixture paths, six source SHA-256 values, expected owner-head tuples; copy-only test behavior; three PHY successes and three REL CVAFRS07 failures; six identity domains and byte-preimage components.
- **Not accepted:** full six-file old-format corpus admission. The three REL fixture semantic heads cannot be confirmed until the actual CVAFRS07 Freshness/Import recovery is restored on a disposable copy.
- **Identity golden subdeliverable:** six executable pinned-reference input/output pairs are now recorded above. The independent rebuilt-implementation parity assertions remain a later rebuild acceptance task.
- The exact blocker is documented as the F03 alternative disposition in the capability plan; it is limited to later conversion/corpus acceptance and does not block unrelated new capabilities.

## Related docs

- [Rebuild plan](architecture-rebuild-implementation-plan-2026-11-03.md)

## Notes

Identity vectors are captured; full corpus proof remains open.


An earlier attempted gate build used the reference checkout's default target directory by mistake, creating ignored partial build output there. Tracked reference files and source corpus remain unchanged; the generated output was not fully removed because Windows denied deletion of some files.

## Historical FRS07 replay result (2026-11-04)

A separate source snapshot from Git commit `c6010fd30dd52acb81c667f7aed99b831307e49f`, whose historical implementation activates the FRS07 causal settlement decoder, was exported into `C:\!bin\workspace\Reliquary-frs07-validation`. All six source fixtures were copied into that snapshot before testing. `cargo test --test frs07_copy_proof --locked --offline -j 1 -- --nocapture` passed: each copied file had the pinned source SHA-256 before open, successfully reopened with that historical implementation, matched every expected owner head in the table above, and had identical bytes after open. The test and copies remain in that validation directory. The historical REL head order is global, Archive, Memory, Entity, Relationship, Graph, Memory-Graph, Vector, turn count.

This is a successful historical decoder admission and head check, not a complete semantic/history fingerprint comparison and not proof that the pinned F01 reference runtime was repaired: that runtime still rejects the three REL files. No original corpus bytes were changed. Conversion and activation remain gated on full rebuilt product acceptance and a validated typed conversion with retained history, identities, provenance and FRS07 semantics.
