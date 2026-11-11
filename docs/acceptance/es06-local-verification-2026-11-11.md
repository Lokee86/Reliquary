# ES06 local engineering-standards verification

Parent index: [Acceptance evidence](./INDEX.md)

## Purpose

Record the independently reviewed local result of ES01–ES06. **Local remediation is verified. Hosted CI has not been run; formal adoption remains `audit-pending`.** This is a scoped documentation/governance result, not an unqualified claim that every product capability or future architecture requirement is complete.

## Overview

Implementation baseline: `fa574eecd11dddbe22474f382f8018cf0c0f98c5`. Final enforcement checkpoint: `f965110c4802893cbb640f737d550bdd5a46d2ad`. Rust test checkpoints below identify the exact tested source; subsequent changes concern Python enforcement, documentation, CI and evidence only. The installed committed tree and preservation checks are recorded separately at installation, avoiding a self-referential commit identifier in this report.

Work ran locally in an isolated clone and writer worktrees, with Luna subagents for documentation, architecture investigation, test-fixture corrections, Cargo-guard hardening and independent review. The original checkout's eight unrelated Rust files were excluded from the accepted source and checked by SHA-256 before installation. No remote publishing or hosted execution is part of this local result.

## Work-package outcomes and current owners

| Package | Result and canonical route |
| --- | --- |
| ES01 | Canonical snapshot restored; 28 files match manifest/provenance. [Initial inventory](./es01-initial-audit-2026-11-11.md) measured 184 findings, distinct from 117 historical findings; no suppression baseline. |
| ES02 | Current README/AGENTS, navigation, [system and subsystem owners](../architecture/INDEX.md), [API](../public-product-api.md), [CLI](../reference/cli.md), [configuration](../configured-runtime.md), [operator](../operator-workflow.md), [developer](../development/workflow.md) entry points. |
| ES03 | [Maintainer map](../architecture/maintainer-map.md), [coverage](../development/documentation-coverage.md), [behavioral contracts](../development/behavioral-contracts.md), [limits](../limits/current-limitations.md), [planning](../planning/roadmap.md), [historical disposition](../development/document-disposition.md). All 23 workspace packages plus separate CLI are routed. |
| ES04 | [Architecture audit](./es04-architecture-audit-2026-11-11.md), populated Rust graph, 13-rule Pitlord composition, exact Cargo guard and real negative controls pass. |
| ES05 | Mandatory workflow added with complete/base-aware docs checks, Demon Docs, snapshot, semantic/DAG gates, negative controls, Rust/CLI/consumer suites and G4 fresh-process acceptance. Local constituent commands pass; hosted workflow execution remains unverified. |
| ES06 | Separate read-only Luna review confirmed source-to-document claims and found corrections integrated before this report. Final local outcomes and limits are recorded here and in [machine-readable results](./es06-verification-results-2026-11-11.json). |

The 41 old historical records remain at their original paths and are routed explicitly. Exact-path historical section-shape exceptions preserve their dated structure; link/index checks still apply. The supplied specification has its own exact shape exception. No broad compliance baseline or deleted historical findings are used.

## Standards and tool provenance

Engineering-standards revision: `11cb28aa8c317eeeea647470e67f60598f28a2f0`. Canonical sync script SHA-256: `c22256d539ab7847730284862f7b75884c9098e4d35d6132f4166761f925f185`. Manifest SHA-256: `acebb5f25b554b1dbde631409028a8f4aac20c4e2de4d40f1d3f20ce92d2f7eb`. Generated `.standards/` files were synchronized with the canonical script, never hand-edited; their bytes are protected against newline conversion and checked by `scripts/check_standards_snapshot.py`.

| Tool | Local version | Reproducible source |
| --- | --- | --- |
| Pitlord | 0.1.2, required-content capable build | `9dedd4b556b9e54de514e4cafe4d94093780178f` |
| Demon Docs | 0.3.5 | `45318a8f2602aeebaeea4828894769256773f4f4` |
| Lexicon / Rust adapter / Arcana | dev / 0.5.0 source / 0.1.0 | `f09517262fdfad98fcede2ad93a947ccee8406ff` |
| Go / Rust | 1.26.5 / 1.97.1 | Explicit workflow versions |

The Go tools built locally offline with repository-local caches/temp paths. Local source differences from the first two published pins were documentation-only; executable source matched. The installed Windows executables are isolated in ignored `target/standards-tools-bin`, with binary hashes in the result record and local provenance file; global tools were preserved. An older global Pitlord reports the same version but rejects `require_content`, so syntax/capability validation and source identity are required. See [operations](../../tools/pitlord/README.md) for explicit local commands and pinned CI build steps.

## Local check results

All commands below exited **0** on the accepted source. The final documentation audit is rerun after this report and index reconciliation; its exact commit/exits accompany the installation evidence. `--changed-from origin/main` in the isolated clone uses baseline `fa574eec`; CI uses the actual PR base or push-before SHA with full history. Uncommitted feature work is never treated as accepted production.

| Command | Measured result |
| --- | --- |
| `python scripts/check_standards_snapshot.py` | 28 synchronized hashes match |
| `python .standards/docs_policy/check.py --repo . --json` | Zero findings, zero baselined findings, zero stale baseline entries |
| `python .standards/docs_policy/check.py --repo . --changed-from origin/main --json` | Zero change-impact findings |
| `python scripts/check_documentation_entrypoints.py` | Root README/AGENTS links pass |
| `ddocs check --root docs --indexes --links --no-global-config` | Complete docs indexes and links pass |
| `python scripts/check_architecture.py` | 23 packages / 76 selected production edges / 15 test-only edges |
| `python scripts/check_gate_controls.py` | Clean fixture passes; missing owner, links, documentation-impact and prohibited dependency controls reject |
| `python scripts/check_policy_tools.py --pitlord <local-bin> --lexicon <local-bin> --arcana <local-bin> --adapters <pinned-adapters>` | Current scan/sync, policy validation, nonempty evidence, 13-rule check and actual dependency negative control pass |
| `cargo fmt --all --check` | Pass, repeated at final Rust checkpoint |
| `cargo check --workspace --locked --offline -j 1` | Pass |
| `cargo test --workspace --locked --offline -j 1 --no-fail-fast -- --test-threads=1` | **678 passed, 0 failed, 0 ignored**, 55 suite reports; source `3a6be786fafd42b8973688b56dd5e8ee001bca2b` |
| `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1` | **17 passed, 0 failed, 0 ignored** |
| `cargo test --manifest-path tests/external-consumer/Cargo.toml --locked --offline -j 1 --no-fail-fast -- --test-threads=1` | **15 passed, 0 failed, 1 default-ignored**; source `46fef0c13cbd07b12173fefb9d315959e722c5dd` |
| `pwsh -File scripts/run_g4_acceptance.ps1 -TargetDirectory <shared-target>` | Pass at `d82ff0e9fe420fb0cb16a8dc99fead8a02884d34`; runs the default-ignored process-reopen consumer, fresh-process and CLI checks with a deterministic synthetic authored cohort and repeat witnesses |

Demon Docs initializes transient link-tracking state with `fix --links` on a clean checkout before `check`; CI fails if that initialization changes authored docs or creates an untracked authored file. It is not a suppression baseline. Temporary paths and analysis/build caches stay in the repository/worktree; no `C:\tmp` was used.

### Semantic and rejection evidence

Final Lexicon snapshot SHA-256: `7578f5de5e5591a19c908b535d43ddb70b29524f0ee941d9da5d5f74e3e5eeb4`. Arcana synchronized with zero compatibility warnings. There are **24 populated areas, 568 owned checked files, 2,051 selected import/dependency edges, 111 cross-area dependency groups, no ownership overlaps/unowned files, no area cycles, and zero violations across 13 rules**.

Both `imports` and Cargo `depends-on` are selected: local resolved imports may be intra-area while Cargo coupling supplies cross-crate evidence. Dependency categories include dev/build/target edges. The lower-layer upward prohibition deliberately applies to every category; the independent exact Cargo guard distinguishes production and dev edges. CLI may depend on public Root and external libraries, but not implementation areas. No graph absence is represented as a runtime proof.

The real policy negative control forbids the known Gateway-to-Application dependency on that same snapshot and requires its exact diagnostic and exit 1. Isolated tracked-repository controls reject missing required docs, broken docs/root links, mapped public-source changes with missing or unrelated owner edits, forbidden production edges through explicit/path aliases, target-conditioned/workspace-inherited declarations, package/path mismatches and version-only substitution of an internal package. A permitted dev-only edge remains accepted. The original guard's alias/target bypass was reproduced before hardening; approved manifests and the 76-edge production DAG were unchanged.

### Failures found and resolved

The first workspace run exited 101: two Gateway fake-model fixtures cited source/prior grounding before evidence resolution. Only fixtures were corrected to provide grounding after the evidence phase. Both failures were reproduced and the final full workspace passed. The first external-consumer run exited 101 because its synthetic mock master-key guard was dropped before reopen; moving the guard to the outer test fixed the fixture lifetime. A subsequent linker attempt failed with LNK1318 during low disk space; an immediate retry passed. These earlier logs are retained alongside the passing results.

Initial semantic scans were stopped or timed out without being called passes. The completed optimized scan used bounded 1200-second phases/1800-second total allowance, with tool-only build optimization and process-tree cleanup. No product build profile, suppression policy or architectural baseline was changed to get a pass.

## Independent source-to-document review

Read-only Luna review checked the Root opacity and mutation boundaries; receipts, cuts and typed replay; authenticated correction/retirement/association and verified-copy refusal paths; confirmed-use privacy; external Hermes/community/inspection host obligations; Gateway mounts, grants, queues, host/worker handoff and recovery; Container partial-append refusal; Switchboard fallback/configuration framing and security; CLI exact invocation and ownership maps. It found and corrected broken root navigation, status commands missing project paths, missing API detail and overbroad configuration claims. A final independent pass confirmed CLI rule orientation and identified stale audit wording, now reconciled.

Behavioral claims route to concrete tests in the canonical matrix. A new direct per-path configuration lock test proves competing-handle exclusion, threaded acquisition after release and reacquisition. Its start signal does not prove the contender was already waiting before release, and it does not prove every high-level credential writer or legacy-migration interleaving. Static policy cannot prove durability, authorization, single-writer behavior, privacy, external-host completion or recovery; the corresponding tests and explicit limits remain necessary.

## Adoption and remaining limits

Reliquary is registered locally in engineering-standards adoption as `library-engine` plus `stateful`, **`audit-pending`**. The registry's pre-existing unrelated continuity-memory edit was preserved; no unrelated registry change was committed. CI has been authored and its constituent commands/negative controls verified locally. No hosted workflow run, branch protection or remote enforcement is claimed. Private tool source checkouts require repository-read access via `STANDARDS_TOOLS_TOKEN` or an authorized GitHub token; unavailable access fails closed.

A concurrent local roadmap exposed another material capability gap, independently confirmed in the accepted source: Community persistence/inspection/rename and retrieval routing exist, but Leiden detection and supported Root/CLI recomputation do not. Canonical limits/planning/semantic ownership now state that gap. The concurrent roadmap and historical-document paragraph remain uncommitted user work and are preserved during installation; they are excluded from accepted production. Their corpus-specific observations are not freshly validated here.

The material [homogeneous-REL/inter-REL dependency-DAG follow-up](../planning/roadmap.md) remains explicitly deferred outside this remediation. The historical `native-two-day-live-smoke-2026-11-07.md` could not be recovered locally; references disclose its absence rather than invent evidence. Existing dated operational acceptance remains historical. No real provider, credential or source-corpus mutation was used. Accepted changes contain documentation/governance, guard hardening, targeted fixture corrections and focused lock coverage; production Rust APIs, runtime semantics, formats and Cargo dependency manifests are unchanged. Existing unrelated feature and documentation edits are outside this acceptance. Final installed-commit checks use a clean local checkout, so concurrent uncommitted documentation is not silently treated as accepted production.

## Documentation impact

- **Inspected:** supplied specification, canonical standards/templates, every workspace manifest and CLI, current/historical documents, policy facts and selected public/state/recovery implementation and tests.
- **Updated:** current owners/navigation, maintenance/coverage/behavioral maps, status/planning/limits, explicit historical disposition, checker mappings, policy operations, CI and this acceptance evidence.
- **Not affected:** production APIs, REL/PHY 2.1 and configuration formats, runtime composition, approved package DAG, real corpora/credentials/providers and unrelated working-tree Rust changes.
- **Compliance check:** zero unbaselined structural/change-impact findings; current graph, exact Cargo gate, negative controls and relevant Rust/consumer/fresh-process suites pass locally.
- **Known gaps:** hosted CI/adoption audit, missing community detection, deferred owner-dependency architecture, unavailable historical smoke artifact and the explicitly bounded concurrency/static-analysis evidence described above.

## Related docs

- [Specification](../engineering-standards-remediation-spec-2026-11-10.md)
- [Documentation procedure](../documentation-procedure.md)
- [Current architecture](../architecture/INDEX.md)
- [Machine-readable verification](./es06-verification-results-2026-11-11.json)

## Notes

This report is dated acceptance evidence. Current behavior remains owned by the linked canonical product and architecture pages. The local result supports the precise scoped claim above and does not authorize an unqualified `enforced`, full-product `compliant`, or future-capability `complete` declaration.
