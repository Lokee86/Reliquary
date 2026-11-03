# ADR 0048: REL-branded current format 2.1 with stable identity preimages

Parent index: [Architectural decisions](INDEX.md)

**Status: Selected design — 2026-11-03. Not implemented and subject to corpus/identity acceptance gates.** Detailed [Stage 5 migration contract](../cva-to-rel-format-and-identity-migration-2026-11-03.md), [wire source inventory](../architecture/cva-source-wire-inventory-2026-11-03.json), [ADR 0043 naming](0043-rel-terminology-and-wire-identity.md), [current-only development format policy](../development-format-hard-cut-interruption-spec-2026-10-19.md).

**Build strategy:** [ADR 0049](0049-parallel-architecture-rebuild-and-verified-cutover.md) builds only final 2.1 runtime codecs in the new target. The isolated converter uses read-only verified old-format copies; no runtime compatibility reader is introduced.

## Purpose

Retire CVA in active Rust and persisted current-format tags while protecting deterministic identifiers, source/provenance chains and current-only storage policy.

## Context

A 2026-11-03 scan of 1,415 tracked Rust files found 644 containing 12,122 case-insensitive CVA matches, 72 basename matches, 173 distinct eight-character CVA-prefixed marker candidates (including legacy/test-only values), and at least six **SHA-256 input-domain literal sites** for FileId and vector identities. Existing Container files use 2.0 with the eight-byte CVA header signature. Current promoted REL corpus has a separate **FRS07 reopen gap**: current source names FRS06, while the authoritative copied REL fixtures contain FRS07. The tests for that six-file corpus remain ignored. A superficial rename would either preserve old file-byte branding or change established identifiers/replay semantics.

## Decision

1. **Canonical code surface:** `Rel`, `RelError`, `RelSession`, `rel_*` and responsibility-specific names. Keep PHY distinct. Eliminate `Cva` aliases, `cva` CLI alias/variables, source comments/error strings, test fixture filenames and old active Rust identifiers; no permanent compatibility layer.
2. **Canonical current container format:** **2.1** with exactly the eight-byte signature `REL\0\r\n\x1a\n`, same typed 40-byte REL/PHY identity header layout, file-kind and owner UUID. Ordinary 2.1 runtime opens no 2.0 file. Reject old or mixed signatures with a typed unsupported/invalid-format error **before mutations**.
3. **Current record discriminators:** replace leading old `CVA` prefix in accepted current writer/reader record families with the same-width `REL` prefix; preserve their domain/version suffix, record schema and semantic clocks. Record codes not bearing that prefix (including `CVDREAM*`, `RQMUX*`, `RQOP*`) are not renamed automatically. Test-only/retired marker candidates must not become new valid record readers.
4. **Stable deterministic hash algorithms:** FileId, ArchiveVectorId, MemoryVectorId, CompatibilityProfileId, VectorGenerationId and PackedVectorId continue to use **exact pre-existing domain separator byte values and input ordering**. Express those separators as **opaque versioned binary constants with meaningful domain names** in active code rather than CVA-named strings. Golden identity/preimage tests prove exact old/new equality. Preserved binary hash preimages are a necessary continuity contract; they do not mean old format readers remain.
5. **Semantic rather than bytewise conversion:** an **isolated, temporary offline** converter decodes all verified current 2.0 semantic records and writes all verified 2.1 records, including nested/grouped carriers, offset/reference remapping, digest updates and explicit owner history. Never use blind string/byte replacement of a live file.
6. **FRS07 gate first:** before conversion, recover the known promoted **2.0 FRS07** current semantics and execute the ignored six-file reopening test on copies. Reject attempts to reclassify accepted FRS07 events as FRS06 or to skip them for convenience.
7. **Exact proof and promotion:** preserve all owner IDs, source IDs, domain clocks, Memory/Graph/Ego/Insomnia/Dream/Freshness history, vector profile/matrix bytes, host reference authority and PHY privacy; compare full semantic fingerprints and deterministic behavior. Record old/new file digests because **file bytes necessarily change**. A multi-file corpus may only be activated after an independently verified generation set; no fake cross-file atomic transaction.
8. **Retire one-time migration code:** normal software and CLI support **2.1 only** after the hard cut. Remove every temporary old decoder, old writer, converter CLI, alias and legacy dispatch. Negative-format tests use binary fixtures/hex to avoid adding old naming literals to active Rust.

## Tradeoff explicitly decided

It is not possible in general to **both** change the bytes fed to SHA-256 **and** preserve those same SHA-256-derived stable IDs. The product's existing behavior/identity constraints take precedence over rebranding an internal hash's preimage. Preserving its **opaque binary bytes** while removing old textual identifiers from Rust is intentional and fully disclosed. **If “remove CVA” is later interpreted to include the underlying hash preimage bytes themselves, then a full rekey of all persisted/external references must be separately authorized.** That is a different behavioral change and cannot be smuggled into this architectural refactor.

## Alternatives considered

- Rename Rust symbols only, leaving old current-format wire markers: rejected.
- Search/replace all old source and disk bytes including SHA domain input: rejected as identity-corrupting.
- Keep old/new reader compatibility in production: rejected by current-only development-format policy.
- Recompute/rekey all owner and external semantic IDs to new namespace: rejected unless a separate end-to-end ID migration is explicitly approved.
- Current-only **2.1**, typed offline conversion and preserved immutable ID domain algorithms: **selected**.

## Required implementation acceptance

- Complete and classify [all 173 lexical marker candidates](../architecture/cva-source-wire-inventory-2026-11-03.json) into actual writer/reader/retired/test-only; additionally inspect dynamic magic and digest builders.
- Demonstrate exactly one normal current signature/version, old unsupported input leaves bytes unchanged, and a structurally valid 2.1 wrong-kind input fails.
- Verify independent old/new owner history/proof fingerprints, vector/identity golden hashes and ObjectRef mapping; no unknown record silently discarded.
- Reconcile operational host journals and cross-file owner refs before promotion; no duplicate native turn, group completion, Freshness event or host operation.
- Promote canonical REL/PHY development corpora with a source/target hash manifest; remove temporary reader/converter and perform case-insensitive **zero-CVA-source** and Rust path scan.
- Perform actual compiler/CLI/host integration and performance regression gates during Stage 6 implementation. **No passing tests or code changes are claimed by this ADR.**

## Related docs

- [Full Stage 5 design](../cva-to-rel-format-and-identity-migration-2026-11-03.md)
- [Source/proof matrix](../cva-rel-migration-source-and-proof-matrix-2026-11-03.md)
- [CVA naming decision](0043-rel-terminology-and-wire-identity.md)
- [New physical crate DAG](../architecture/target-crate-dag-2026-11-03.json)
- [Current format baseline](../storage-format.md)

## Notes

Historic immutable documentation may retain the prior terms for provenance. The existence of the new format design is not proof that the currently promoted six-file corpus is recoverable under this branch; conversion remains forbidden until its specific Freshness mismatch is correctly resolved.
