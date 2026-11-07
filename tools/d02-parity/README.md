# D02-T01 synthetic comparator foundation

Status: provisional fixture-only prototype. This is not an accepted D02 implementation, a current-only target reader, a complete PG-C01 report, or an authentic parity claim.

Run:
python -m unittest discover -s tools/d02-parity/tests -p "test_*.py" -v

The comparator imports no Reliquary converter or product traversal. It consumes strict semantic-row JSONL plus the exact pinned JSON Schema, an independently supplied expected owner/family/cut inventory, and a separately supplied target-coverage sidecar. Row identity includes owner, family, stable key, revision/event, and ordinal. A row's coordinate remains its immutable publication/origin coordinate; explicit sidecar memberships separately state which snapshot cuts contain that row. It rejects duplicate rows, rows absent from either independent envelope, unknown schema bytes/IDs, family/group/value-schema mismatches, invalid typed integer ranges, non-finite IEEE values, malformed byte/digest hex, unknown or unordered/duplicate domain coordinates, invalid provenance spans, and unavailable-reason mismatches. It preserves nested array order and ignores JSONL file-line order. Diagnostics omit field values.

The synthetic oracle and target-coverage sidecar are separately hand-authored in tests/synthetic_inventory.py, with fixed SHA-256 pins asserted by the test harness. They are test-only, non-authoritative inputs; the fixture deliberately has REL/project and PHY/unspecified owner tuples without asserting a universal kind/scope rule. Real D02 admission must bind completeness and expected values to the accepted preconversion F03 witness, not these fixtures.

## Contract provenance

The schema and row fixtures in contract-v1 were copied from Reliquary-pg-c01-v1 commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87:

- tools/corpus-evidence-v1/schemas/semantic-row-v1.schema.json
- tools/corpus-evidence-v1/fixtures/comparator-pass-source-rows.v1.jsonl
- tools/corpus-evidence-v1/fixtures/comparator-pass-target-rows.v1.jsonl

Commit 0f824a09 adds test-only independent reference-encoder tests. The schema and fixture blob IDs listed above, the C01 goldens/report example, and the bundle hash match dd77dfa8 exactly. The PG-C01 contract remains under independent review; this pin does not declare that ticket accepted.

## Reader boundary for the next increment

The target-reader executable is not implemented here. Its future adapter must open candidates through current-only 2.1 APIs (RelSession::open_path_current and PhySession::open_path_current) and use only typed historical reads/proofs plus narrowly selected immutable observations. It must run in a fresh process and never import or reuse converter traversal, source mappings, or output-derived expected values. Missing fields need an independently reviewed immutable/test-only owner observation, not a mutable store handle. The offline tool remains outside the 23-package/76-edge production dependency graph.

No Cargo command, authentic corpus, converter, or reader output is used by this foundation. These synthetic diagnostics are not the final D02 evidence/report. D02 still requires full source/receipt binding, cut coverage, custody, physical-reference and operational/privacy checks before any authentic comparison.
