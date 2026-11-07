# Post-G4 corpus evidence contract — 2026-11-06

Parent index: [Architecture](INDEX.md). Execution authority: [post-G4 plan](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation specification, not generated evidence or conversion acceptance.** Contract identifier: `reliquary-corpus-evidence-v1`. F03, D01 and D02 must freeze the same contract before independently implementing their producers. This is an offline interchange/evidence contract, not a new production database, crate or semantic authority.

## Purpose

Allow independent historical-source verification, conversion and current-runtime verification to agree on exactly what must survive. A file that opens, matching version heads, a converter's own success flag, or a digest produced by the converter cannot establish parity. Old and new runtimes have different encodings; byte-for-byte equality of entire files is neither expected nor sufficient.

## Overview

`verified old bytes -> independent old-reader witness -> frozen source expectation`

`verified old bytes -> isolated converter -> new candidate bytes + conversion receipt`

`candidate bytes -> current-only reader -> independent target witness -> comparator`

The comparator consumes independently produced source and target witnesses. It never imports the converter or reconstructs missing expected values from its output. The same schema and canonical serializer may be shared as test/evidence tooling; record traversal, semantic extraction and eligibility decisions must not be shared between converter and verifier.

Source authority remains the six hash-pinned files in [F03 evidence](../rebuild-f03-identity-and-corpus-evidence-2026-11-03.md). The historical FRS07 implementation is `c6010fd3`; the older F01 source `f62b1fb4` is NOT a valid reader for the three FRS07 RELs. The accepted rebuilt baseline is `a3c3a964` (production/test changes through `31e7adb2`). Every execution records full resolved commits, binary digests and toolchain versions rather than relying on these abbreviated names.

## Target ownership and artifacts

| Artifact | Producer | Required consumer | Authority |
| --- | --- | --- | --- |
| `source-custody.json` | F03 read-only inspection | All lanes | Exact source identity and immutable baseline |
| `source-witness/` and `source-coverage.json` | Independent historical reader | D01 planning, D02 comparator | Expected semantic/history observations |
| `codec-coverage.json` | D01, reviewed against F03 inventory | D02 | Every actual physical family mapped, rejected or proven derived |
| `conversion-receipt.json` and `relocation.jsonl` | D01 | D02/D03 | Audit input only; not proof that target semantics match |
| `target-witness/` and `parity-report.json` | D02 current reader/comparator | D03/G5 | Independent parity result bound to exact output hashes |
| `generation-manifest.json` | D03 | Activation/rollback controller | Cohort membership and verified release candidate |
| `performance-report.json` and `release-decision.json` | G5 | Operator/activation | Measured release gate; not permission to migrate unchecked data |

Each artifact includes schema ID, producer role, full source and code hashes, UTC capture time plus local reporting date, command/arguments, exit status, coverage, parent artifact hashes and explicit `PASS`, `FAIL`, or `INCOMPLETE`. A later timestamp or renamed path never substitutes for the original source digest. Reports contain hashes/IDs, not conversation text, credentials or raw private PHY payloads; full witnesses remain access-controlled alongside disposable copies.

## Semantic witness data shape

An owner descriptor contains owner UUID bytes, REL/PHY kind, scope, history generation/lineage, source-file digest, per-domain version heads and the actual supported cut enumeration. Every family has a presence/coverage row even when empty. An unsupported, absent, unavailable, corrupt or unaudited family must not be serialized as an empty supported family.

A semantic row contains `family`, `stable_key`, `revision_or_event`, `ordinal`, `owner`, `coordinate`, `availability`, `value`, and `source_binding`. A coordinate preserves global/domain versions, transaction timestamp, REL activity position and any accepted-event boundary separately. Relationships between coordinates are evidence, not interchangeable clocks. Array order is semantic unless a field is explicitly defined as an unordered set.

The v1 family catalog is exhaustive for the selected source population:

| Family group | Values that must survive |
| --- | --- |
| Container/lineage | Owner kind/scope/UUID, generation, accepted reservation ordering, transaction times, grouped publication boundaries and lineage proof |
| Archive/Echo | Conversations, branches/parents/canonical selection, every retained turn revision/body/role/author/time, Episodes/fragments, files/attachments, Echo/compaction records and typed terminal/unavailable payload state |
| Memory | IDs, body IDs, all retained revisions, lifecycle/tombstones/supersession, categories/type/authority/scope, temporal evidence, source refs and routing receipts |
| Entity/Relationship | Stable IDs, aliases/participants, full revision streams, source-body/spans, resolution/withdrawal/merge/retirement evidence and retained descriptions |
| Graph/Community | Nodes/edges/origins/active state, full accepted mutations, version cuts, membership generations, user versus inferred names and lineage |
| Ego | Identity, Personality, Anchors, retained revisions and owner scope; no new synthesis |
| Freshness | Policy, accepted birth/use/settlement events, original authority, activity clock, scores, propagation inputs, event cuts, FRS07 causal basis and explicit intra-settlement unavailability |
| Insomnia/Dream/import | Work identities, claims/retries/held/terminal receipts, source Episodes, initial populations/frontiers/pairs, completed import scope and accepted prefix proofs |
| Vectors | Packed bits, dimensions/scalar, all six stable hash-derived identities, profile probes/normalization, ordered row bindings, active/retired generations and coverage |
| External/project/operational | Host namespaces/message identities/revisions/hashes, repository revision/file refs, acknowledged delivery, queue/stream receipts and sidecar references actually present |
| Maintenance | Prune/retention proofs, exact removed versus retained payload state, repair/rewrite/reconcile lineage and source authority |

If an actual retained carrier is absent from this catalog, F03/D01 records it as `unclassified`; conversion of that source stops until the contract is extended and independently reviewed. The old lexical inventory's 173 marker candidates are a discovery aid, not the accepted family allowlist.

## Canonical encoding and identity

Use strict UTF-8 JSON Lines for semantic rows with a separately versioned canonical serializer. Reject duplicate object keys, invalid UTF-8, unknown mandatory fields and non-finite values. Object keys are sorted lexicographically; insignificant whitespace is removed. Text is preserved exactly: no Unicode normalization, case folding or newline rewriting. All integer coordinates/timestamps are decimal strings; byte sequences/IDs are lowercase hexadecimal. Floating-point values are encoded as exact scalar-width bit strings with explicit endianness, not rounded decimal JSON numbers. Optional values have an explicit present/absent tag when absence is semantically distinct from null.

Rows sort by `(family, stable_key, revision_or_event, ordinal)` using a documented byte ordering. Ordered event history retains its original ordinal even when keys sort elsewhere. Repeated identical events remain distinguishable where repetitions are themselves retained history. Duplicate semantic keys are errors, not last-writer-wins.

For each family compute SHA-256 over `reliquary:corpus-evidence:v1\0`, length-prefixed owner/family identifiers, declared coverage, row count, and length-prefixed canonical row bytes. Owner and corpus digests bind the ordered family/owner catalog, including zero-row families and unavailable markers. Golden tests pin exact preimages/outputs for this serializer. Schema changes require a new version and regeneration from unchanged source bytes; never update expected digests from a failing target run.

Three identity classes must remain distinct:

1. Stable semantic IDs and published identity-preimage bytes: exact equality, including FileId, PackedVectorId, CompatibilityProfileId, ArchiveVectorId, MemoryVectorId and VectorGenerationId.
2. Common semantic witness digest: exact equality when comparable fields/coverage are identical, regardless of old/new physical encoding.
3. Physical file/frame/envelope digests and offsets: expected to change only through declared re-encoding; validate their own transport integrity and relocation rather than pretending they are stable IDs.

A native 2.1 historical proof is not automatically byte-comparable with an older proof namespace. Verify each proof through its own runtime and compare the underlying common semantic projection; require exact proof equality only for an explicitly unchanged semantic proof domain. A stable external reference derived from changed physical bytes is a real compatibility conflict, requiring an explicit per-family resolution before that conversion can pass.

## History coverage and resource bounds

Enumerate every retained semantic event/revision and every valid domain cut. Use incremental/streaming hashes to avoid materializing the entire history at every cut. Check complete global-coordinate/transaction-time ordering and cross-domain publication closure. Query whole-owner projections at every semantic-changing global boundary when supported; a supported cut may not be omitted merely because rebuilding it is slow.

A reader may checkpoint a long verification and continue in bounded batches. A batch manifest binds source hash, reader/schema hash, cut range and preceding digest; concatenate only contiguous, nonoverlapping ranges. `sampled` or `incomplete` is never promoted to exhaustive acceptance. Historically unavailable intra-settlement states remain explicitly unavailable with their retained reason and evidence; the converter must not invent them. Stored terminal history may survive without its payload; distinguish missing, tombstoned, pruned/unavailable, unauthorized and corrupt.

## Conversion receipt and relocation contract

For each file record original/target hashes, immutable source identity, target identity, exact source/target runtime revisions, codec-map digest, input/output counts per family, transport checksum changes and output state (`candidate`, never `active`). Relocation rows bind old object/offset/range -> new object/offset/range, containing owner/family, payload identity and reference kind. All retained references must resolve within the intended owner/file and permitted cross-owner set. Check injectivity where uniqueness is required, collisions, dangling/cyclic physical references, bounds, body hashes and grouped nested references. No raw-pointer transplant or blanket prefix replacement qualifies.

The converter may spool a versioned typed transfer stream between separate old-reader and new-writer executables. That stream includes original semantic values/coordinates and source locations, is hashed, and is never the independent expected witness. It must preserve exactly the families in the reviewed map. It is not a permanent runtime API or generalized migration framework.

## Testing decisions and acceptance

PG-C01 freezes schema fixtures before F03/D01/D02 consumers diverge. Fixtures must include empty families, two convergent heads with different intermediate histories, reordered events, unavailable bodies, non-ASCII text, integer precision beyond 2^53, floating bit differences and unknown records. Both independent readers must produce the same known semantic fixture encoding without importing converter code.

Every report names tested coverage and diagnostics by owner/family/cut/stable key. D02 emits all bounded discrepancies plus total counts, not only a final Boolean. Truncated diagnostics cannot hide failure. Internal observation must leave source/target bytes, versions and Freshness receipts unchanged. A successful comparator over empty or self-produced expected data fails the test.

## Out of scope

New storage architecture, a shared mutable owner facade, permanent 2.0 readers, re-inference/re-embedding, fabricated history, arbitrary repair, corpus publication to public repositories, or automatic activation. No source text is sent to a model during evidence generation.

## Related docs

- [F03 implementation](f03-historical-verification-implementation-spec-2026-11-06.md)
- [D01 converter](d01-offline-converter-implementation-spec-2026-11-06.md)
- [D02 verifier](d02-independent-parity-verifier-implementation-spec-2026-11-06.md)
- [Stage 5 format decisions](../cva-to-rel-format-and-identity-migration-2026-11-03.md)
- [Source/proof matrix](../cva-rel-migration-source-and-proof-matrix-2026-11-03.md)

## Notes

This document fixes the offline evidence boundary. Implementing the serializer, schemas and golden fixtures is PG-C01; no generated artifact or passing semantic proof is claimed by writing this specification. Lexicon was unavailable in the rebuilt checkout, so ownership verification used the accepted package DAG and direct source inspection.
