# D02-T01 synthetic comparator foundation

Status: provisional fixture-only prototype. This is not an accepted D02 implementation, a current-only target reader, a complete PG-C01 report, or an authentic parity claim.

Run:
python -m unittest discover -s tools/d02-parity/tests -p "test_*.py" -v

Example:
python tools/d02-parity/compare_synthetic.py --source tools/d02-parity/fixtures/comparator-pass-source-rows.v1.jsonl --target tools/d02-parity/fixtures/comparator-pass-target-rows.v1.jsonl

The comparator imports no Reliquary converter or product traversal. It consumes strict semantic-row JSONL plus an explicit JSON Schema, compares semantic row sets keyed by owner/family/stable key/revision-or-event/ordinal, rejects duplicates and empty expected data, preserves array order, and ignores JSONL file-line order. Diagnostics omit field values. Tests use only hand-authored synthetic fixtures from the pinned contract revision.

## Contract provenance

The schema and row fixtures in contract-v1 were copied from Reliquary-pg-c01-v1 commit dd77dfa8f7d3c73e3e03cd07b2f4f0f7c9850fae:

- tools/corpus-evidence-v1/schemas/semantic-row-v1.schema.json
- tools/corpus-evidence-v1/fixtures/comparator-pass-source-rows.v1.jsonl
- tools/corpus-evidence-v1/fixtures/comparator-pass-target-rows.v1.jsonl

The PG-C01 contract remains under independent review. This copy pins the inputs used by synthetic tests and does not declare that ticket accepted.

## Reader boundary for the next increment

The target-reader executable is not implemented here. Its future adapter must open candidates through current-only 2.1 APIs (RelSession::open_path_current and PhySession::open_path_current) and use only typed historical reads/proofs plus narrowly selected immutable observations. It must run in a fresh process and never import or reuse converter traversal, source mappings, or output-derived expected values. Missing fields need an independently reviewed immutable/test-only owner observation, not a mutable store handle. The offline tool remains outside the 23-package/76-edge production dependency graph.

No Cargo command, authentic corpus, converter, or reader output is used by this foundation. These synthetic diagnostics are not the final D02 evidence/report. D02 still requires full source/receipt binding, cut coverage, custody, physical-reference and operational/privacy checks before any authentic comparison.
