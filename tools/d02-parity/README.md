# D02 independent checker

The fixture comparator and current-only reader verify selected synthetic history. Authentic corpus parity is `INCOMPLETE`.

```powershell
python -m unittest discover -s tools/d02-parity/tests -p "test_*.py" -v
```

The comparator imports no Reliquary converter or product traversal. It consumes strict semantic-row JSONL plus the exact pinned JSON Schema, an independently supplied expected owner/family/cut inventory, and a separately supplied target-coverage sidecar. Row identity includes owner, family, stable key, revision/event, and ordinal. A row's coordinate remains its immutable publication/origin coordinate; explicit sidecar memberships separately state which snapshot cuts contain that row. It rejects duplicate rows, rows absent from either independent envelope, unknown schema bytes/IDs, family/group/value-schema mismatches, invalid typed integer ranges, non-finite IEEE values, malformed byte/digest hex, unknown or unordered/duplicate domain coordinates, invalid provenance spans, and unavailable-reason mismatches. It preserves nested array order and ignores JSONL file-line order. Mismatch diagnostics identify fields and stable keys. The optional report path must be new.

The synthetic oracle and target-coverage sidecar are separately hand-authored in tests/synthetic_inventory.py, with fixed SHA-256 pins asserted by the test harness. They are test-only, non-authoritative inputs; the fixture deliberately has REL/project and PHY/unspecified owner tuples without asserting a universal kind/scope rule. Real D02 admission must bind completeness and expected values to the accepted preconversion F03 witness, not these fixtures.

## Contract provenance

The schema and row fixtures in contract-v1 were copied from Reliquary-pg-c01-v1 commit 0f824a09fe01f5ff327ab56f3296f36d8f4d7c87:

- tools/corpus-evidence-v1/schemas/semantic-row-v1.schema.json
- tools/corpus-evidence-v1/fixtures/comparator-pass-source-rows.v1.jsonl
- tools/corpus-evidence-v1/fixtures/comparator-pass-target-rows.v1.jsonl

Commit 0f824a09 adds test-only independent reference-encoder tests. The schema and fixture blob IDs listed above, the C01 goldens/report example, and the bundle hash match dd77dfa8 exactly. The PG-C01 contract remains under independent review; this pin does not declare that ticket accepted.

## Current owner reader

The standalone reader uses current-only 2.1 replay and immutable observations from the same read-only file handle. It runs independently of the converter and remains outside the 23-package, 76-edge production dependency graph. Synthetic end-to-end tests compare local Archive cuts and standalone Memory history and clocks against separate literal oracles. See the [reader commands](reader/README.md) and [synthetic candidate results](reader/test-evidence/native-synthetic-candidate-checks-2026-11-07.json).

Authentic parity still requires a frozen F03 admission, common retained-family and cut coverage, conversion receipt and relocation verification, and operational and privacy checks. The fixture schema represents five synthetic families. The current reader's native observations have separate schema identifiers.
