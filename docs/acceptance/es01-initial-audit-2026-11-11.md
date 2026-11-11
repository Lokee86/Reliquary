# ES01 initial standards inventory — 2026-11-11

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the first newly measured audit after restoring standards tooling. This is historical remediation evidence, not a compliance acceptance.

## Overview

Implementation baseline: `fa574eecd11dddbe22474f382f8018cf0c0f98c5`. The remediation checkout excludes the source worktree's unrelated uncommitted Rust edits. The supplied remediation specification is retained unchanged. Profile: `library-engine` with `stateful`; the separately shipped CLI must also satisfy CLI-product requirements. No findings are baselined or suppressed in this initial measurement.

## Commands and results

Measured 2026-11-11 using Python 3.11.15. Commands ran from the remediation checkout unless stated otherwise.

| Command | Exit | Result |
| --- | --- | --- |
| `python C:\!bin\workspace\engineering-standards\tools\sync_checker.py .` | 0 | Generated checker, normative docs, shared policies and SHA-256 manifest |
| `python .standards/docs_policy/check.py --repo . --json` | 1 | **184 current findings**; detailed inventory in [initial audit JSON](es01-initial-docs-2026-11-11.json) |
| `python scripts/check_architecture.py` | 0 | 23 packages, 76 selected production edges, 15 test-only edges; guard unchanged |

| Finding class | Count | Interpretation |
| --- | --- | --- |
| Missing required path | 12 | Canonical product, architecture, coverage, planning and limits artifacts absent |
| Missing coverage owner | 8 | Configured responsibility owners not yet written |
| Missing index | 1 | Root documentation navigation absent |
| Missing section | 161 | Current and historical documents lack the configured canonical shape; classification required before applying historical exemptions |
| Broken link | 2 | Existing document links do not resolve |

The historical G4 count of 117 is preserved in its original report and is not this inventory. The manifest at `.standards/manifest.json` records exact synchronized file hashes. Generated snapshot files must be refreshed from standards authority, never hand-edited.

## Semantic gaps and known limitations

The mechanical count does not measure semantic currency. README and agent guidance still describe the old rebuild; source-backed current architecture owners, public/CLI/configuration references, maintainer routes, contract mappings, document disposition, Pitlord policy, and CI are required by ES02–ES05. Pitlord/Lexicon/Arcana and Demon Docs availability must be measured separately. The deferred homogeneous-REL/dependency-DAG roadmap is not implemented by this remediation. No storage, runtime, provider, credential or corpus mutation is authorized for documentation verification.

## Related docs

- [Remediation specification](../engineering-standards-remediation-spec-2026-11-10.md)
- [Historical G4 findings](../g4-product-corpus-acceptance-2026-11-06.md)
- [Documentation policy](../documentation-policy.md)

## Notes

This is the ES01 measurement before canonical owners are added. Subsequent reports must record fresh outcomes and retain this dated evidence. There is no temporary compliance baseline to remove.
