# CVA → REL naming, current-format and identity migration — Stage 5 (2026-11-03)

Parent index: [Documentation index](INDEX.md)

**Status: COMPLETE AS A TARGET MIGRATION DESIGN, NOT IMPLEMENTED.** All affected files, identifiers and actual corpus outcomes must be verified at the later implementation gates; no Rust, CLI, fixtures or canonical REL/PHY bytes have been converted. **Canonical decisions:** [ADR 0043](decisions/0043-rel-terminology-and-wire-identity.md), [ADR 0048](decisions/0048-rel-current-format-and-stable-identity-domains.md), [Stage 4 dependency architecture](dependency-architecture-and-crate-layout-2026-11-03.md). **Lexical source evidence:** [machine-readable marker and hash-domain inventory](architecture/cva-source-wire-inventory-2026-11-03.json). The [source classification](cva-rel-migration-source-and-proof-matrix-2026-11-03.md) defines affected responsibilities and acceptance.

## Purpose

Eliminate **CVA terminology and its legacy-format signatures from active production Rust**, without corrupting existing owner files, changing stable semantic identity, losing historical or provenance evidence, or accumulating perpetual compatibility readers. Close the engineering choices that would otherwise turn a mechanical rename into an unsafe development-format migration.

## Overview

**Chosen hard cut:**
- Rust/API concept: **`Rel`**, `RelError`, `RelSession` and correctly owned `rel_*` identifiers. `Phylactery`/`PhySession` remain separate. Rename by responsibility; moving `Cva` intact would not fix architecture.
- Persistent current container: **format 2.1**, header signature **`REL\0\r\n\x1a\n`**, otherwise the existing typed 40-byte REL/PHY header structure and owner identity fields. Old 2.0 files are **not readable through normal 2.1 open**. Error is typed `UnsupportedFormatSignature` (or equivalent distinct from repairable tail) when the old signature is presented, before mutating anything; the 2.1 reader also rejects any other version.
- 8-byte record magic: for an actual current-format encoded record whose eight-byte discriminator starts with the old three-character prefix, replace **only that leading prefix** with `REL` and retain its exact five-byte family/version suffix, e.g. `CVAPUB01` → `RELPUB01`, `CVAINSC8` → `RELINSC8`, `CVAXMEM2` → `RELXMEM2`. Apply the same mapping to prefix+terminator lengths such as `CVACMP1\0` → `RELCMP1\0`; nonmatching non-CVA families such as `CVDREAM1/2`, `RQMUX*` and `RQOP*` remain unchanged unless their own independently justified contract requires a change.
- **Decoded semantic data, IDs, clocks and historical proofs are not regenerated to match renamed magic.** The old domain-separated SHA-256 **identity-preimage bytes remain unchanged** behind clearly named, versioned opaque binary domain constants. Source code contains **no textual CVA references** after the cut, including in literals/tests; the old preimage *bytes* remain mathematically necessary to preserve deterministic legacy IDs. A migration cannot claim to remove their values and simultaneously preserve the hashes without a deliberate full identity remap. This explicit stable-binary-contract distinction is documented, not hidden.
- Re-encode accepted **semantic** record values using the new codec/marker, recalculate transport/checksums/digests bound to changed physical bytes, and remap byte-offset ObjectRefs if any offsets move. **Do not perform blanket in-place byte search/replace**, even though the simple marker replacements preserve width.
- All canonical development files are migrated **offline onto verified copies**, promoted in a separately guarded corpus operation, and old normal readers/converter removed. A named **temporary conversion executable** may read both formats only during that one-time conversion phase. Production supports exactly one current format, 2.1.

These choices extend the existing [development hard-cut policy](development-format-hard-cut-interruption-spec-2026-10-19.md); no compatibility exemption is established. They do not change current model algorithms, history retention, REL-vs-PHY authority, or external project VCS.

## 1. Source inventory, scope and semantic risk

A scan of **1,415 tracked Rust files** at `51579a9f` found **12,122 case-insensitive textual CVA matches in 644 files**, including **72 Rust paths with cva in the basename**. This is a lexical count (not 12,122 unique renames). The audit also found **173 distinct exact eight-character `CVA[A-Z0-9]{5}` lexical candidates** in production/test source and six direct, separate SHA-256 identity-domain literal sites. Both the old parser and tests can mention retired or deliberately malformed tokens; **173 is a discovery upper bound, not 173 current writer formats**.

The [inventory JSON](architecture/cva-source-wire-inventory-2026-11-03.json) records each candidate marker, representative production/test files and its evidence class. Implementation must resolve every candidate into: **current writer, current accepted reader, rejected legacy/invalid sample, or test-only historical example**. A candidate can belong to multiple roles. Explicitly check dynamically assembled magic strings and prefix comparators not captured by a literal regex. Normal decoder acceptance is then **only** the newly branded current writer format, never the literal-candidate superset.

| Affected owner / source | Representative legacy bytes and where grounded | Effect of new current code |
| --- | --- | --- |
| Container physical header and clock | `crates/reliquary-container/src/container.rs`: old eight-byte file signature, current `FormatVersion { 2,0 }`; `publication_envelope.rs` `CVAPUB01`, `version.rs` `CVAVERS1/2` | New 2.1 header/signature and `REL` carrier/clock writer; preserve exact global-coordinate and transaction-time semantics |
| Grouped publication | `container/grouped_clock.rs`: `CVAINSC7/8`, `CVAXMEM2` | New carrier discriminators; decode/re-encode grouped embedded versions, body proof and source receipts together, not independently flatten |
| Archive/turn/source/Echo | Archive codecs, `file_store.rs`, retired `reliquary-echo` records | New record tags; preserve turn/branch/Episode history, source-bound attachments, accepted activity and Echo evidence |
| Memory | Memory Core format/delta/body/route/publication codecs | New record tags; preserve Memory IDs, bodies, historical revisions, provenance, tombstones and typed source references |
| Graph/Community | Graph snapshot/mutation/compact/community codecs | New tags; preserve endpoint identity, semantic cuts, Communities and graph lineage |
| Entity/Relationship | Perception Store entity/resolution/relationship/compact codecs | New tags; preserve canonical identity, mention source anchoring, transitional Relationship records and owners |
| Ego | Ego Store compact/Identity/Personality/Anchor/delta codecs | New tags; preserve state/revisions and existing PHY visibility boundaries |
| Freshness | `freshness-store` scores, events, import settlement, activation, rewrite baseline | New tags **after first repairing source-current mismatch**; retain exact event IDs, +25/+50 source proof, clocks, scores and history |
| Vectors/profiles | Vector Store row/packed/profile/generation codecs | New tags; **do not change the six hash domains below**; preserve packed bytes/profile/generation IDs |
| Insomnia/Dream/Import | Insomnia durable work and grouped publication, Dream Begin/journal `CVDREAM*`, Import epoch/proof/retention | Rename applicable current tags only; preserve complete receipt, first-cycle settlement, grouped write and import scope proof |
| External source | `CVAXIDX2`, external Memory grouped carrier and CAS metadata | Re-encode derived index, or discard/rebuild **only if proven nonauthoritative**; retain accepted publication receipts and host reference identity |
| Gateway operational journals | `RQMUXJ01`, `RQMUXS01`, `RQMUXH01`, `RQOP0001` | Remain operationally separate from owner semantics; only owner references and any explicitly old-format embedded record bytes need checked/remapped |
| CLI, tests, examples, docs | `cli/src/args.rs` has `rel` with `visible_alias = "cva"` and `cva: PathBuf` fields; old `.cva` test paths | Remove alias and old exposed variables, rename affected examples/fixtures; historical documentation can describe retired forms |

**Current-format known blocker:** Six promoted authority fixtures contain Freshness `CVAFRS07` receipts; the present Rust `freshness_semantic_settlement.rs` still exposes `CVAFRS06`, and the whole-corpus reopen at `tests/current_format_boundary.rs` remains `#[ignore]`. The prior copy-based observation was three PHY opens and three REL failures. A successful **current semantic FRS07 reopen/proof** is mandatory **before** generating the 2.1 migration baseline. Never relabel FRS07 as FRS06 or omit those events merely to get conversion working.

## 2. Stable identifiers: explicit no-rekey design

Changing a SHA-256 domain separator changes the resulting ID even when the semantic fields are identical. Source-level identity inputs found:

| Stable identity derivation | Current source | Target identity contract |
| --- | --- | --- |
| Non-project Archive `FileId` | `reliquary-archive/src/file_store.rs::file_id` | Preserve exact file-ID v1 prefix/preimage and all field ordering |
| `ArchiveVectorId` | `reliquary-vector-store/src/archive.rs::archive_vector_id` | Preserve exact vector-ID v1 prefix, packed ID and fragment sequence |
| `MemoryVectorId` | `reliquary-vector-store/src/memory.rs::memory_vector_id` | Preserve exact v1 prefix, compatibility profile, packed ID and body-ID order |
| `CompatibilityProfileId` | `reliquary-vector-store/src/profile.rs::compatibility_profile_id` | Preserve exact v1 prefix, dimensions, suite/policy, normalization and probe floats |
| `VectorGenerationId` | `reliquary-vector-store/src/generation.rs::vector_generation_id` | Preserve exact v2 prefix, profile and Archive vector IDs and version |
| `PackedVectorId` | `reliquary-vector-store/src/store.rs::packed_vector_id` | Preserve exact v1 prefix, scalar/schema and matrix bytes |

**Mechanism:** Define a small set of **versioned, opaque binary identity-domain constants** near the actual domain owners, e.g. `FILE_ID_DOMAIN_V1` and `PACKED_VECTOR_ID_DOMAIN_V1`, whose **byte arrays are bit-for-bit equal to the previous SHA input strings** but whose Rust names and literals contain no CVA spelling. Tests pin exact preimage hex and known ID outputs. This is not a legacy **file reader** or a compatibility alias; it is the one stable, still-current identity algorithm. Never switch to an `REL-...` SHA prefix without a new explicit proposal that fully rekeys **all** persisted, external and transitive references.

**Identity classification:** Durable owner UUID, Memory/Entity/Graph/Archive source IDs, FileId, vector IDs, request/receipt stable IDs, reference-only host source IDs and deterministic Dream/Freshness event IDs are *semantic references*. Their **exact bytes and relationships** must survive. Any additional digest bound directly to old file frames or header/record magic is instead a **transport/hash-of-encoding** and may need recomputation; it must never masquerade as an unchanged semantic ID. Build the exhaustive list from all hash/ID constructors and retained corpus proof, not just the six known string sites. If a raw-frame-derived digest is an externally referenced stable identity, that is a **hard stop requiring explicit resolution** before promotion.

**Boundary of the user's naming directive:** Every active Rust symbol/comment/string/test fixture naming the old CVA concept is retired; the byte values of already-published deterministic hash namespaces are not renamed because that would intentionally change identities. This is the only intentional distinction; the migration report must disclose and verify it.

## 3. Current format 2.1 and exact conversion contract

**Header:** preserve existing header size, field positions, file-kind/scope/UUID encoding and V2.0 semantic clocks; update header magic to `REL\0\r\n\x1a\n` and version to **major 2, minor 1**. A new normal opener validates this exact pair **before** payload replay, writable access, scan-state publication or tail repair. Any previous 2.0 signature returns typed format rejection, not generic “repairable tail.” New tests must prove attempted read/repair does not mutate original bytes. Reject a forged 2.1 version with old signature or forged 2.0 version with new signature.

**Record discriminators:** a *current accepted* record codec's existing `CVA` leading prefix becomes `REL` with identical suffix and length (including embedded/grouped record types and terminated prefix strings). Domain owners retain each existing semantic version unless its payload schema actually needs to change; container header 2.1 is the format generation boundary. Do not bulk-convert retired test-only markers into accepted record types. All physical codec dispatchers/prefix checks must use the new canonical tags.

**Encoding and integrity:** Input converter reads old **verified physical framing** under an exclusive copy; decodes accepted grouped carriers into typed records and reconstructs the full owner semantic state, including original immutable source identity, independent clocks and history. It writes a **new** file with the target 2.1 codec, identical ordered semantic record values, and the identical accepted global coordinate and transaction timestamps where contractually applicable. Recalculate length/CRC/SHA/checksum, envelope proof, nested carrier digest, population/birth/event evidence **only when bound to changed encoding**. Retained semantic proofs whose identity inputs are independent of wire bytes must remain bit-for-bit stable. The converter must reject unknown source records rather than silently skip them.

**Object references:** An `ObjectRef`, body span, physical offset or stream checkpoint may point into old file bytes. Do **not** claim length-preserving marker substitution is enough: changed payload/canonical framing can shift offsets, and digests over physical framing can change even at equal width. Maintain an explicit old→new physical reference relocation map for each record/body, then rebind every derived pointer/owner proof via typed record rewrite before writing the final head. If a pointer cannot be safely relocated with its authoritative evidence, **fail conversion and keep the original**.

**Reference and history proof:** Cross-owner/source references, Memory/Graph versions, archived turns, transaction-time history, tombstones, Dream/Insomnia receipts, imported provenance, acknowledged accepted-use, source host/project VCS correlation and Ego states remain canonical. A new container **file SHA-256 necessarily changes**; preserve old and new file digests in the offline custody manifest, and only update external indexes/manifests whose contract is specifically “hash of full file bytes.”

**Derived-only indexes:** Lexical, Community accelerator and external source vector index may be rebuilt **only if proven derivable** and no accepted source event, profile identity or receipt is discarded. Rebuilding must not increment semantic Freshness or generate a new Dream event.

**Gateway/host journals:** Existing operational journal file markers are not rebranded merely because the semantics are REL; journal entries containing owner UUID/cut remain stable. Any journal carrying a raw old payload or content digest derived from old-format bytes must be consumed/reconciled/fenced **before** corpus activation, or migrated by its own typed converter. An unknown host operation status is **Held**, not retried on a fresh ID.

## 4. One-time offline migration sequence (not implementation order of Rust crates)

1. **Freeze and inventory:** record tracked source baseline, all six authoritative corpus paths and bytes, SHA-256, owner UUID/kind, global heads, local domain clocks and full semantic/historical fingerprint, plus all other retainable REL/PHY datasets, imports, project attachments and operational journals. Capture CLI/host consumer call sites and current ABI before any rename.
2. **Repair source semantic support first:** in a disposable old-format baseline, correctly parse/replay **FRS07** as current accepted semantics; unignore/execute six-file current corpus admission using copies. No FRS07 data loss, forged event settlement or unsupported reader under production normal-open.
3. **Compile the converter in a temporary, isolated migration workspace/executable:** it may understand exactly verified **2.0 current input** and selected **2.1 output**. Its old signature codecs/hash-domain text are not linked into the final runtime. It must have a manifest of every accepted codec and refuse unknown current-value families.
4. **Produce new 2.1 files on new paths** with exact owner UUID and scope; re-encode every record, update physical carrier checksums and offset references, maintain stable identity-domain bytes, and return a per-file old→new read/proof/reference map. Do not edit originals.
5. **Independently verify copied output** using *only the new normal 2.1 runtime*; compare each owner's full retained historical fingerprint, exact stable IDs, VCS/host source correlations, temporal cuts, full Graph/Entity/Ego/Freshness/Dream/Insomnia state, vector matrices/profile/gen bytes, indexed candidate/retrieval parity, and source-owner privacy. Running verification may not itself reinforce Freshness.
6. **Verify owner group consistency and host operation reconciliation** across the 3 REL+PHY pairs. A multi-file set is not one filesystem-atomic transaction. Put all verified immutable files in a new generation directory and **atomically switch a single validated generation selector/manifest where the deployment has that capability**; otherwise use a documented release-stop, copy/rename/backups/rollback protocol. Never claim six independent renames are one transaction.
7. **Commit separately:** (a) new format implementation and gold tests and (b) promoted corpus+source/preimage/postimage hash manifest. The development corpus must be migrated and committed at the same release gate as the format cut, not deferred.
8. **Retire all temporary old readers, converter executables/CLI switches, source aliases and old normal dispatch**, retaining only immutable format/identity evidence in documentation or offline archived migration artifacts. Release the final normal writer/reader as **2.1 only**.
9. **Final negative/consumer gate:** old header/marker input fails closed before mutation, new output passes all owner/CLI/host tests, zero-case-insensitive-CVA active Rust source and paths, and no duplicate `Cva` facade or configuration key/CLI alias remains.

**Abort/rollback:** any mismatch in source identity, accepted record count/proof closure, global or domain clocks, linked offsets, private PHY material, host status or model/retrieval parity stops promotion. Original bytes remain read-only with recorded hashes; restore previous generation selection (if switched) and investigate. Do not silently “repair” a source fixture or rekey external IDs.

## 5. Verification matrix and behavior gates

| Category | Positive evidence | Required negative case |
| --- | --- | --- |
| Header + file identity | REL and PHY 2.1 exact magic/version, typed header, same UUID/scope and 40-byte structure | Old or forged mixed headers, bad owner kind, unknown revision, damaged tail fail with no mutation |
| Physical clocks/objects | Exact accepted global coordinates/transaction timestamps and correct read frontier, valid old→new body relocation | Offset dangling, duplicate reservation or wrong grouped clock rejected |
| Memory/Archive/Echo | Same accepted turns, branches, Episodes, bodies, revisions, imports, attachments, Echo events | Forged source, incomplete grouped carrier or tombstone payload fabrication rejected |
| Dream/Graph/Entity | Same pair/canonical/Community generation evidence, Entity/Relationship history and first-cycle receipts | Duplicate/rewire false reinforcement and missing target reference rejected |
| Freshness | Correct FRS07 baseline, same accepted events, scores, import settlement, F8, +25/+50 and history | FRS06 re-interpretation of FRS07 and reinforcement from internal audit rejected |
| Vectors | Exact preimage+hash ID gold vectors and packed bytes/profiles/generations/row counts | Renamed SHA namespace that would rekey IDs; bad profile/backing rejected |
| Ego/PHY privacy | Same Identity/Personality/Anchors and private retained history, no cross-REL leakage | Other principal cannot gain PHY access through migration or new API |
| Gateway/host | Same pending/finalized/held operational receipts, native vs host ownership and checkpoint visibility | Ambiguous host operation auto-replayed or old stream cursor revived rejected |
| Consumer code/API | `Rel`/REL-only CLI/API, all direct dependent compilations, 2.1 files reopen/writable | Old `Cva` aliases, `cva` CLI alias or compatibility runtime accepted |
| Performance | Original cohort and new 2.1 equivalent workload: no material regression in open/replay, memory, retrieval, worker or writer latency | Added whole-file read/copy on ordinary open, global owner lock or ambient migration decode rejected |

**Code naming scan scope:** Git-tracked active Rust files (`src`, `crates`, `cli`, `tests`, `examples`, `benches`, build scripts), generated Rust and active command help/config/schema source. Case-insensitive `cva` in *contents and path names* must be zero. Legacy documentation, immutable history and explicitly archived migration **binary** evidence may contain historical spelling, but never compiled/current source. Do not “solve” the check by excluding production paths or allowing a permanent `Cva = Rel` alias. Test negative signatures via precomputed binary bytes or external archived input fixture, not old textual source constants.

## 6. Alternatives considered

| Alternative | Consequence | Decision |
| --- | --- | --- |
| Only rename Rust type/CLI, leave file bytes | Quick, but old product identity remains in active format and request is incomplete | Reject |
| Replace every `CVA` literal blindly, including hash domains | Recomputes FileId/profile/vector IDs; breaks deterministic references and current-only open | Reject |
| Add permanent readers/aliases for both 2.0 and 2.1 | Accumulates compatibility cost and double semantics while product is pre-release | Reject |
| Rewrite every semantic ID to a new REL-based hash | Requires proven transitive/external remapping and changes established identity; violates current preservation constraints | Reject absent a separately approved rekey |
| **2.1 format cut + offline typed rewrite + opaque stable identity domains + retirement** | One canonical new runtime format, no CVA source symbols, preserved identities and historical proofs | **Select** |

## 7. Closure and implementation prerequisites

**Stage 5 decisions closed:** canonical Rust/API names, CLI removal, new header version/signature, exact prefix translation for current record codecs, independent identity-preimage stability, typed offline conversion, old/current reader retirement, ObjectRef relocation, owner corpus promotion/rollback, host/journal reconciliation and release/test gates.

**Hard prerequisite to executing the cut:** the presently unverified six-file **FRS07** current-corpus baseline must be recovered and proved before producing new 2.1 bytes. This is an **implementation blocker**, not an unspecified format-design decision. A second explicit hard stop applies if subsequent source/fixture analysis uncovers any stable identity that necessarily hashes changed raw physical bytes; the architecture forbids quietly breaking identity and requires reconciling that discovery before promotion.

**Remaining step:** [Stage 6] create the exact bounded source-to-target implementation sequence, freeze acceptance checks and specify gate-by-gate handoffs across `reliquary-owner-session`, `reliquary-application`, Gateway, CLI/consumers and corpus conversion. **Do not begin migration because this design exists alone.**

## Related docs

- [ADR 0048: New current REL format and stable identity](decisions/0048-rel-current-format-and-stable-identity-domains.md)
- [Machine-readable wire source inventory](architecture/cva-source-wire-inventory-2026-11-03.json)
- [Affected source and proof matrix](cva-rel-migration-source-and-proof-matrix-2026-11-03.md)
- [Stage 1 owner sessions](rel-phy-storage-architecture-2026-11-03.md)
- [Stage 4 crate structure](dependency-architecture-and-crate-layout-2026-11-03.md)
- [Storage format baseline](storage-format.md)
- [Development-format hard-cut policy](development-format-hard-cut-interruption-spec-2026-10-19.md)

## Notes

The selected format version and proof machinery are **design targets**, not an invented current compiler/runtime result. The raw 173 marker candidates include historical and invalid examples; format migration work must positively classify the live accepted writers and readers before emitting a converter. A full on-disk byte migration may change file-level digests and physical offsets even where all semantic IDs remain identical; every changed proof must be justified and revalidated.
