# Reliquary documentation policy

Parent index: [Documentation](./INDEX.md)

## Purpose

Apply the synchronized engineering standard to Reliquary's library, CLI and stateful host integration.

## Overview

The primary profile is `library-engine` with `stateful`. The separately shipped CLI also meets the CLI-product requirements for getting started, exact commands, configuration, diagnostics and operations. Reliquary has no deployed service/daemon contract. The normative authority is the generated [documentation standard](../.standards/docs/documentation-standard.md), [profiles](../.standards/docs/profiles.md) and [status definitions](../.standards/docs/completeness.md). Repository specialization is executable in [docs-standard.json](../docs-standard.json).

## Ownership and shape

Current behavior belongs to the public reference, operator guide or focused architecture owner. [Maintainer routes](architecture/maintainer-map.md) identify the owner for a change intent. [Coverage](development/documentation-coverage.md) accounts for packages and contracts; [behavioral contracts](development/behavioral-contracts.md) identify protecting tests. These maps link to owners rather than repeat their behavior.

Every normal current page contains Purpose, Overview, Related docs and Notes plus its type-specific obligations. Architecture owners explain responsibility, non-ownership, flow, state, concurrency, recovery, invariants, code and tests. Guides explain prerequisites, steps, results and recovery. Reference pages describe exact public contracts and diagnostics. Every docs folder uses `INDEX.md`; indexes list every direct Markdown file and direct subfolder index.

## Historical and planning material

[Document disposition](development/document-disposition.md) explicitly classifies retained pre-remediation records. Dated acceptance, audit, continuation and implementation-specification records remain at their original paths to preserve citations and durable evidence. Their historical status is declared by the navigation and disposition owner; their past claims are not rewritten as present acceptance. Exact historical paths listed in `section_exemptions` retain their original shape. This exception applies only to those immutable records, not to links, indexing, current owners or new documents. The supplied remediation specification keeps its purpose/problem/target/work-package shape and is an exact section-shape exception; it is not a canonical product page. No baseline hides findings.

Unfinished behavior belongs in [planning](planning/roadmap.md); active defects, unsupported capabilities and unavailable verification belong in [limits](limits/current-limitations.md). The remediation specification governs this execution and does not establish implementation acceptance. The homogeneity roadmap remains a future correction.

## Enforcement and status

Run the shared checker over the entire repository and committed PR diff. Concrete path rules require the affected canonical owner to change; a generic docs edit cannot satisfy every responsibility. The checker proves structural coverage and that an owner changed, while review verifies the content. Demon Docs owns additional index/link verification. Pitlord owns supported static architecture rules; the independent Cargo DAG guard remains mandatory. Behavioral tests protect state, authority, recovery and concurrency.

Refresh `.standards/` only with `python tools/sync_checker.py <reliquary-checkout>` from the authority checkout. The manifest identifies each exact file by SHA-256; `.gitattributes` preserves those bytes. [Provenance](../standards-provenance.json) records source revision, sync-script identity and manifest identity; after a deliberate refresh, update that repository-owned record from the authority and verify the copied bytes. CI must use the committed snapshot without a sibling checkout. Full compliance requires all selected gates and semantic review; a missing required gate prevents an unqualified complete/compliant claim.

## Related docs

- [Procedure](documentation-procedure.md)
- [Development workflow](development/workflow.md)
- [Architecture policy operations](../tools/pitlord/README.md)

## Notes

Scope is documentation and governance. Do not change Rust APIs, storage or runtime behavior to make prose or policy pass. Record a real implementation violation separately with reproduction, owner, severity and focused correction.
