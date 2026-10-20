# Development storage-format lifecycle hard cut — interruption specification (2026-10-19)

Parent: [Documentation index](INDEX.md).

Status: **Policy interruption I1–I12 integrated and verified; runtime retirement not implemented.** This interrupts the [minimal-lossless-history-storage linear plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md) after accepted Step 86 and before Step 87. Frozen checkpoint: `39eea4a2fdce2549e161972422958f369d05e9f3`. The [interruption execution record](development-format-interruption-execution-2026-10-19.md) supplies the completed policy/caller/corpus audit; original Step 87 may resume. Final R1–R22 migration and retirement remain mandatory.

## Purpose

Prevent Reliquary from accumulating permanent compatibility code for development-only persisted formats. Historical *data* and semantic history must survive deliberate migration; old development *encodings*, decoders, format dispatch, and one-time conversion logic are not permanent product requirements.

## Overview

### Development-format policy

1. Establish exactly one canonical format for normal REL/PHY runtime reads and writes.
2. When changing the persisted format, build the smallest temporary offline converter necessary to migrate controlled retained development data.
3. Migrate the development corpus immediately, including authoritative REL/PHY fixtures and other inventoried retained corpora.
4. Validate exact historical semantics, owner identity and clocks, proof integrity, vector bytes/profiles, reconciliation, prune/rewrite behavior and retrieval parity before promotion.
5. Commit the canonical format implementation and migrated corpus (in their respective repositories) with hash manifests and acceptance evidence.
6. Remove temporary migration machinery and old-format readers/dispatch from the runtime and developer CLI. The normal runtime must reject older formats with a typed unsupported-format error.
7. Retain backward compatibility only with a specific documented requirement: a real consumer, supported version, named owner, and explicit review/sunset condition. No exception is currently established for CVAFRS03/04/05, CVAILEG1/2 or REL/PHY container V1.0.

This is a policy correction, not a change to history-retention or pruning semantics. Compatibility *profiles* for embedding/vector data are independent live semantic objects and must not be confused with obsolete file formats.

### Current-state baseline

- Main was frozen at commit `39eea4a`; compact-history Steps 1–86 were recorded complete at the prior checkpoint. Next old-plan step is 87.
- Current canonical target for the container overhaul is V2.0 with compact owner records; CVAFRS06 is the current semantic import-settlement writer.
- The present design retains V1.0 REL/PHY reader paths and an upgrade shell, explicitly planning a persistent read/migration compatibility boundary.
- Existing import compatibility includes CVAFRS03/04/05 and CVAILEG1; the ongoing Memory migration work has introduced CVAILEG2 proof transport. These are candidates for retirement, not assumed permanent formats.
- Earlier signature inspection found CVAFRS03 and CVAILEG1 in three authoritative REL fixtures (first 14 days, first 28 days, Ellis). Their corresponding PHY fixtures contain neither marker. This is not a complete corpus or parsed-payload inventory.
- At Step 86 the V2 migration path was incomplete for later owner families. Therefore removing all V1 readers immediately **before** the V2 migrator can preserve those owner histories would be unsafe.

## Invariants and scope

- Preserve complete accepted semantic history at every supported historical cut, not only current live projections; maintain logical IDs, immutable payloads, ownership, local/global clocks, timestamps, reconciliation evidence, exact vector values and authentic proofs.
- Never overwrite original authoritative files before disposable-copy migration, exact comparison and hashes pass.
- Do not forge missing historical evidence, silently discard old records, or treat a V2-header copy containing obsolete payload records as a fully migrated canonical corpus.
- Separate **temporary development migration readers** from **permanent runtime readers**. Temporary offline tooling may be needed until all V2 domain encodings exist; it is removed after final promotion, not exposed as public compatibility.
- Do not broaden this into import, Freshness, owner-clock, compact-codec, memory graph, or pruning redesign. A narrow refactor needed to separate format translation from semantic replay is allowed with a specific acceptance test.
- No partial implementation: if a specific source corpus cannot be migrated losslessly, leave that corpus unchanged and keep runtime retirement gated; report the exact obstruction.
- The separately committed historical storage steps remain accepted unless verification proves a regression.

## Required changes

### A. Freeze and dependency audit

Record freeze hash, clean-tree status, fixture repository hash and retained-corpus inventory. Identify all V1 and legacy settlement/epoch dispatch in container, facade, Freshness, imports, Memory, repair, reconciliation, CLI, tests and docs. Classify every path as canonical runtime behavior, temporary upgrader-only behavior, or obsolete compatibility. Inspect the actual canonical emission format, not just magic strings.

### B. Canonicalization boundary

Make V2 the sole target for new REL/PHY storage. Ensure V2 migration translates every legacy domain record to a currently supported canonical payload representation; the later compact-owner phases must finish before their old record variants can be retired. For import settlements, translate accepted legacy physical evidence to semantic CVAFRS06 while preserving authenticated logical evidence and reconstructability. Legacy epoch transport (including CVAILEG2 if only a migration bridge) must not become a permanent normal-runtime obligation.

### C. Disposable migration and promotion

Enumerate authoritative and other retained development corpora. Snapshot source hashes and domain fingerprints. Convert copies; reopen with the *new-format-only* reader, run historical oracle including Ellis retrieval parity, and compare every retained cut where tractable. Capture unavailable cuts explicitly. Reject corruption or nonrepresentable proof instead of guessing. Stage/promote migrated fixtures atomically; verify the source was not modified before promotion. Commit fixture changes independently and record new checksums.

### D. Legacy retirement

Only after successful corpus promotion, remove old V1 read-only runtime paths, V1 container dispatch, old Freshness-settlement codecs, CVAILEG1/2 transport and special prefix validation, exposed normalization/repair migration entry points, one-time upgrader shell/tooling and obsolete compatibility matrices, unless an exception was formally approved. Keep negative tests for explicit unsupported old-format errors and adversarial malformed current-format payloads. Remove dead compatibility flags/branches from current-format record models.

### E. Verification and release gates

Check root and CLI formatting, compilation and library/integration tests; exact rewrite/prune/reopen and migration parity matrices; historical owner/global cut oracle; import retry/reconciliation; vector/profile fidelity; Ellis retrieval; architectural and docs guards. Re-run the tests after *removal*, not just after successful migration. Require clean intended diffs, isolated commits and a new development policy statement in `docs/storage-format.md`, `docs/development.md`, and relevant ADRs.

## Plan correction and sequencing

**Do not implement immediate removal at the current Step 86 frontier.** The canonical V2 compact-domain writer/upgrader is not yet complete. Implement the *policy seam and acceptance plan* immediately; continue domain migration work as provisional, gated work, then execute the final migration-and-removal hard cut before declaring the V2 overhaul complete. This resolves the apparent conflict between “fix immediately” and safe preservation: compatibility retirement is mandatory, but destruction cannot precede a capable lossless migrator.

Existing Steps 1–86 remain immutable accepted history. Replan Steps 87 onward to treat every old-format converter as temporary, add explicit post-migration deletion/negative-test gates, and supersede the old final acceptance wording claiming continued V1 persisted compatibility.

## Acceptance criteria

- No permanent V1 container read path, CVAFRS03/04/05 settlement decode, CVAILEG1/2 decoder, or temporary migration command is reachable from normal runtime, except independently justified and approved exceptions.
- Three known authoritative REL fixtures and their PHY pairs, plus all other deliberately retained corpus files, are in canonical V2 and reopen with current-only readers.
- Equivalence evidence covers all semantically supported historical cuts and explicit unavailability cases; Ellis retrieval parity is exact.
- Every temporary migration helper is removed following corpus promotion and checked for unreachable/dead symbols.
- Fixture and source commits are separately recorded, full acceptance rerun passes, and documentation has no misleading “V1 remains supported” promise.
- If any gate fails, **do not claim completion**; the original corpus remains safe.

## Related docs

- [Previous compatibility audit](development-format-compatibility-retirement-audit-2026-10-19.md)
- [Minimal lossless history storage specification](minimal-lossless-history-storage-spec-2026-10-19.md)
- [Minimal lossless history storage execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)
- [Storage format](storage-format.md)

## Notes

Specification and plan amendments do not constitute migration, source-code implementation, or fixture verification. This specification explicitly rules out using a permanent runtime decoder to avoid a controlled development migration.
