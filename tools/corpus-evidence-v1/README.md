# PG-C01 corpus-evidence v1 tooling

Status: provisional implementation, not an accepted evidence contract. This standalone Python tooling is outside the Cargo workspace and production dependency DAG. It implements deterministic framing, strict typed JSON validation, synthetic interoperability fixtures and report examples. It does not export or verify the six authentic source owners, establish a source carrier inventory, or permit conversion/activation.

The 11 semantic group codes and their retained-value obligations are frozen from the post-G4 contract. Group codes identify semantic areas; they are not physical markers or proof that any source carrier was observed. The physical-carrier registry is intentionally empty and fail-closed. The record schemas in this bundle use only a synthetic namespace. Authentic mode must reject this bundle until F03/D01/D02 review and hash-bind a real carrier-to-reviewed-record-schema map. Do not use the 173 lexical marker candidates as an allowlist.

## Byte-level v1 proposal

- Evidence JSON is strict UTF-8, with no BOM, no duplicate decoded object keys, no unpaired surrogates, no unknown fields or enum values, and no JSON numeric tokens. Decimal values are canonical strings. Bytes and digest IDs are lowercase hexadecimal. Floating values are width/endian-tagged IEEE bit bytes and non-finite encodings reject.
- Canonical JSON follows RFC 8785 (JCS), including UTF-16-code-unit object-member ordering. Text is not normalized. The UTF-8 text bytes are preserved after JSON decoding. Row object keys are fixed ASCII schema keys. The supplementary/BMP key-order test distinguishes JCS ordering from UTF-8-byte ordering.
- Semantic JSONL uses LF separators and a final LF. LF delimiters are not included in canonical row bytes or row hashes.
- Row sorting compares the UTF-8 bytes of (family, stable_key, revision_or_event) lexicographically, component by component, then compares ordinal as eight-byte unsigned big-endian. Thus ordinal 2 sorts before ordinal 10 and the byte rule is portable. Ordinals must be canonical unsigned decimal strings in 0..2^64-1. An exact duplicate tuple rejects; otherwise repeated identical events with different ordinals remain distinct.
- LP(x) means u64be(byte_length(x)) followed by x; lengths and record counts are unsigned 64-bit big-endian, overflow rejects. IDs in hash preimages are their lowercase-hex UTF-8 form, not raw ID bytes. The owner key is canonical JCS bytes for exactly {owner_uuid, owner_kind, scope}.
- Family digest: SHA256(b"reliquary:corpus-evidence:v1\\0" || LP(owner_key) || LP(family_id_utf8) || LP(canonical_coverage_json) || u64be(row_count) || each LP(canonical_row_bytes)).
- Owner digest: SHA256(b"reliquary:corpus-evidence:owner:v1\\0" || LP(owner_key) || LP(canonical_semantic_owner_descriptor) || u64be(family_count) || ordered family entries). Each entry binds group code, canonical coverage, the exact ordered typed cut catalog and raw 32-byte family digest. The descriptor binds owner UUID/kind/scope, history generation, typed lineage, version heads and supported cuts. Physical source-file hashes are custody evidence and are excluded from the semantic descriptor.
- Corpus digest: SHA256(b"reliquary:corpus-evidence:corpus:v1\\0" || u64be(owner_count) || owners ordered by owner-key bytes). Each entry binds LP(owner_key), LP(canonical_semantic_owner_descriptor) and raw 32-byte owner digest.
- Custody digest has its own domain, b"reliquary:corpus-evidence:custody:v1\\0", and binds source/target physical hashes and physical relocation offsets. Changing physical bytes or offsets changes custody digest without changing the common semantic digest. Changing logical provenance (source identity, body hash or source-body span) changes the semantic digest.

Coverage has distinct supported, absent, unsupported, unavailable, corrupt, unaudited, and unclassified states. supported plus row_count: "0" is supported-empty; it is not equivalent to any other state. Coverage completeness is separately exhaustive, sampled, or incomplete. Each typed cut binds a separate global version, a sorted vector of domain versions, a transaction timestamp, REL activity position, accepted-event boundary, and typed availability. Missing/unsupported/unavailable/corrupt history cannot be coerced to zero or empty.

## Artifacts

- registry/semantic-groups-v1.json: frozen 11 semantic group codes and obligations.
- registry/record-schemas-v1.json: closed, versioned record-schema registry containing synthetic-only shapes for contract tests.
- registry/physical-carriers-v1.json: intentionally empty, fail-closed pending F03-V02/D01-T01.
- schemas/*.schema.json: generated JSON Schemas for strict common rows, owners, cuts, coverage, reports and custody.
- fixtures/: hand-authored synthetic source/target JSONL, report examples and independent literal golden preimages/digests.
- corpus_evidence_v1.py: dependency-free canonicalization, schema and digest validation; no source traversal or conversion.

Run the lightweight tests from the repository root:

    python -m unittest discover -s tools/corpus-evidence-v1/tests -p "test_*.py" -v

The tests require jsonschema==4.26.0 (see requirements-test.txt); they do not run Cargo or touch corpus data. A successful run verifies this synthetic tooling only. It does not satisfy PG-C01 acceptance, F03 source admission, D01 conversion, D02 parity, D03 staging, G5 release acceptance, or operator authorization.