# Engineering standards remediation — post-rebuild compliance specification

**Status:** Specification; not an implementation or compliance acceptance.
**Repository:** `C:\!bin\workspace\Reliquary`.
**Canonical standard:** `C:\!bin\workspace\engineering-standards` (documentation standard, documentation procedure, profiles, completeness/status, adoption, and architecture enforcement/procedure).
**Inspected baseline:** `main` at `fa574eec`, 2026-11-10; the working tree contains unrelated in-progress Rust changes. Preserve them.

## Purpose

Close the engineering-standard obligations left incomplete by the clean REL 2.1 rebuild, and make those obligations enforceable for future changes. This is documentation and governance remediation with an evidence-based architecture audit, **not** a new architecture rebuild.

## Problem statement

The implementation shipped substantial acceptance evidence, but historical acceptance reports have been allowed to substitute for current product and maintainer documentation.

Direct observations at the inspected baseline:

- `README.md` and `AGENTS.md` still describe the rebuilding worktree, retired adjacent specification locations, and early acceptance status rather than the current product.
- The repository lacks `docs-standard.json`, the synchronized `.standards/` snapshot, `docs/INDEX.md`, `docs/architecture/INDEX.md`, the canonical maintainer/coverage/behavioral-contract maps, and `.github/` CI workflow configuration.
- `scripts/check_architecture.py` protects the approved Cargo package DAG and selected forbidden imports, but is not a substitute for the shared documentation checker, repository-specific Pitlord architecture policy, or behavioral tests.
- G4 acceptance (`docs/g4-product-corpus-acceptance-2026-11-06.md`) recorded **117 historical documentation-policy findings** plus failed Demon Docs index/link checks. That is **not** a freshly measured current violation count.
- Later documentation records state the documentation checker could not run because `docs-standard.json` was missing; the current checkout still lacks it.
- Current documentation mixes acceptance evidence, implementation specifications, future roadmaps, operator guides, and stale historical assertions without canonical navigation and ownership.

Consequences: future maintainers lack a trustworthy single entry point and change-to-owner route; implementation changes can bypass documentation updates; and the repository cannot make a standards-compliance claim.

## Target state and ownership

1. **Standards authority:** `engineering-standards` owns normative policies, templates, shared checker source and synchronized snapshots. Reliquary owns its profile selection, local configuration, exceptions, architectural areas, and CI gates. Never edit generated `.standards/` by hand.
2. **Current product documentation:** canonical docs under `docs/` own implemented behavior and stable public contracts. Architecture pages own responsibilities, non-owners, call direction, lifecycle, persistent state, failure/recovery, invariants, code maps, and tests. Guides and reference pages own workflows and exact public API/CLI/configuration contracts.
3. **Historical evidence:** existing G2/G3/G4/GM/H/M/P/OC acceptance and specification records remain identifiable as dated evidence, not authoritative current product guidance. Reference them from current docs without replicating their content.
4. **Planning and limitations:** unfinished work (including homogeneous REL/dependency-DAG decisions) lives in planning; actual defects and known capability limits live in limits. Neither may silently masquerade as current behavior.
5. **Maintainer map and coverage:** one repository-level maintainer map routes change intents to canonical document, implementation owner and verification surface; focused coverage maps account for production crates, CLI families, public contracts, persistent data, stateful flows, concurrency, and recovery. A focused subsystem may own several small utility crates; do not create one page per file.
6. **Enforcement:** the existing package-DAG script remains an independent exact dependency gate. Repository-local Pitlord policy protects meaningful static ownership and forbidden edges using reliable evidence; tests and runtime verification protect behavioral and recovery invariants that static analysis cannot prove. Shared documentation checker, Demon Docs and CI make violations visible.

**Profile decision:** use `library-engine` as the primary documentation profile with `stateful` capability, and explicitly meet CLI-product requirements for the separately shipped CLI. Document actual host protocols where present. Do not claim a deployed service/daemon profile or create service/deployment instructions for an unimplemented surface.

## Implementation decisions and constraints

- Restore the existing engineering-standards enforcement workflow by syncing the current snapshot with `engineering-standards/tools/sync_checker.py`, then configure a repository-local `docs-standard.json` based on the canonical template.
- Use concrete coverage/change-impact mappings for the root public crate (`src/**`), `crates/reliquary-*/**`, `cli/**`, persisted format/configuration, tests, and scripts. Avoid a generic `docs/**` catch-all that lets unrelated edits satisfy a documentation change rule.
- Configure direct-file/direct-folder index coverage and required section checks. Distinguish a navigation index from a canonical architecture or reference owner.
- Introduce a local Pitlord composition root (`tools/pitlord/policy.json`), meaningful semantic owner/dependency rules (`tools/pitlord/semantic.json`), and an operations guide (`tools/pitlord/README.md`). Validate that the Rust Lexicon/Arcana evidence actually supports selected rules. Do not invent semantic guarantees from an unavailable or stale graph; record tool evidence gaps and keep the existing exact Cargo check active.
- Preserve approved architecture boundaries and the current-format REL/PHY files, credentials, source corpora, and all existing acceptance evidence. Treat `23 packages / 76 selected production edges` as the currently approved DAG, not an immutable product constant; any future approved DAG change must deliberately update its authoritative specification, guard, and canonical architecture documentation together.
- When the documentation contradicts actual code, verify behavior using source and tests before amending the canonical text. A passing checker is necessary for structural compliance, never sufficient for semantic currency.
- Do not alter Rust APIs, storage formats, runtime semantics, or architecture to make the documentation easier. If an actual implementation violation is discovered, record its reproduction, owner, severity and required focused correction as a separate change.
- Do not erase historical findings, retroactively change acceptance claims, add placeholder compliance files, create duplicate compatibility layers, or use a baseline as evidence of final compliance.
- Keep the current unrelated Rust worktree changes intact. Documentation remediation should be independently reviewable from concurrent feature work.

## Work packages and dependency order

### ES01 — Restore standards tooling and establish an authoritative baseline

**Depends on:** none.

**Deliverables:** synced `.standards/` with manifest hashes; `docs-standard.json` with correct profile, required paths, index convention and repo-specific path mappings; an initial timestamped audit inventory with exact command outcomes and findings. Configure only the initial policy required to measure the repo; do not mark failures passed.

**Acceptance:** local shared checker executes against Reliquary and produces a meaningful report rather than `missing configuration`; synced files match the authoritative source; audit distinguishes missing artifacts, mechanical findings, semantic gaps and already-known limitations; the existing architecture guard still runs unchanged. Any temporary baseline is explicit, narrow, counted and scheduled for removal, and is never called compliance.

### ES02 — Establish canonical product documentation and navigation

**Depends on:** ES01.

**Deliverables:** current product `README.md`; current `AGENTS.md`; `docs/INDEX.md` and required folder indexes; documentation policy/procedure; canonical system architecture plus focused subsystem owner pages; current public API, CLI, configuration, developer and operator entry points. Select the smallest justified folder/document structure and move existing docs rather than copy them unnecessarily.

**Minimum coverage:** Root library/opaque REL-PHY API; Container and durable store ownership; owner-session, grouped commits and history; Application workflows; semantic processing (Insomnia, Dream, Entity, Vectors, Freshness, retrieval); Gateway/authenticated Instances and worker lifecycle; Switchboard config/security/provider routing; CLI and external host contracts; failure, recovery, compatibility, tests and extension seams.

**Acceptance:** current docs accurately describe the inspected implementation; each architecture owner explains responsibilities and non-owners, state, flow, failure/recovery, invariants and protecting tests; public guides include prerequisites, concrete use and errors; `README.md` and `AGENTS.md` link only to existing canonical paths and stop referring to obsolete rebuild milestones as the present. Historical evidence remains discoverable and is labeled historical.

### ES03 — Make documentation coverage and status auditable

**Depends on:** ES02.

**Deliverables:** maintainer map, implementation coverage map, behavioral-contract matrix, planning owner, limitations owner; accurate references from current docs to acceptance evidence; classified archival/legacy decisions for old rebuild records; updated link/index graph.

**Acceptance:** every production crate, public command family, persistence/mutation boundary, stateful/concurrent/recovery flow and machine-readable contract is routed to a canonical owner and (where critical) a protecting test. Every direct documentation file and subfolder is indexed. Each old document is retained as current/evidence/planning/legacy, moved, or retired with no missing durable fact. No claim of implementation completeness is inferred from dated acceptance records.

### ES04 — Repository-specific architectural compliance review and enforcement

**Depends on:** ES02 and ES03. May investigate independently while ES02 is written, but integrate only against the agreed current architecture.

**Deliverables:** documented inventory of architectural obligations; repository-local Pitlord rules/operations guide linked to canonical architecture pages; evidence-backed review of responsibility ownership, forbidden reach-through, dependency direction, single-writer authority, state/lifecycle/concurrency, recovery and extension seams; explicit audit results and issues for real violations.

**Acceptance:** Pitlord policy validates and runs with current compatible Lexicon/Arcana evidence or records the precise tooling blocker without falsely reporting an architectural pass; meaningful ownership, dependency and cycle rules reflect actual architecture; existing `scripts/check_architecture.py` continues to pass; critical behavioral invariants are covered by tests or explicitly listed as gaps. No speculative refactor is performed.

### ES05 — Activate continuous enforcement

**Depends on:** ES01, ES02, ES03 and ES04.

**Deliverables:** CI workflow(s) running the synchronized documentation checker on complete repository and change impact, Demon Docs structural/link checks, Pitlord validation/check, existing package DAG guard, applicable Rust build/test checks, and reproducible local command documentation. Register Reliquary's actual adoption state in `engineering-standards/adoption/repositories.json` when verified; do not label it `enforced` prematurely.

**Acceptance:** checks fail on a deliberately introduced missing required document, broken link, undocumented mapped public-surface change, and prohibited package/dependency policy change; tests also pass on the clean current baseline. CI does not depend on a sibling checkout; generated snapshot provenance is reproducible. Changed-path comparisons use the appropriate PR base without treating local uncommitted work as accepted production.

### ES06 — Independent final documentation and compliance acceptance

**Depends on:** ES05.

**Deliverables:** concise final report identifying repository revision, tool/standards versions, current document owners, check commands/results, semantic source-to-document spot checks, unresolved gaps and final adoption state. Preserve reports as acceptance evidence, not replacements for canonical docs.

**Acceptance:** all configured documentation structure, index, link, change-impact, architecture, CI and relevant Rust/CLI gates pass; no hidden material documentation debt remains; coverage and critical behavioral-contract mappings are complete; independent review confirms public/operator workflows and critical state/failure claims agree with code. Any unresolved material implementation violation or unavailable required policy gate prevents an unqualified `compliant` or `complete` declaration.

**DAG:** ES01 → ES02 → ES03 → ES04 → ES05 → ES06 (ES04 fact-finding may overlap ES02/03; final integration remains ordered).

## Verification commands and evidence

Use the repository's exact supported commands and syntax, adjusting only paths and target directory as needed:

```powershell
# From C:\!bin\workspace\engineering-standards (one-time and refresh)
python tools/sync_checker.py C:\!bin\workspace\Reliquary

# From C:\!bin\workspace\Reliquary
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --changed-from origin/main
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1
cargo test --workspace --locked --offline -j 1 -- --test-threads=1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1
```

Use the actual repository-configured Demon Docs commands and Pitlord validation/graph/check commands documented in `tools/pitlord/README.md`; verify availability before specifying executable invocation. Retain external Root-only consumer tests and relevant operator CLI tests in the final gate. Do not run real provider/corpus mutations merely to validate documentation; cite existing verified operational acceptance and run targeted tests when necessary.

The final report records actual exit codes, committed revision, policy/source snapshot manifest, matched test suites, remaining limitations, and any deferred blockers. It must not use dated test results as evidence that a freshly changed documentation/CI gate passed.

## Out of scope

- Rebuilding the 23-package architecture, reorganizing crates for aesthetic reasons, API redesign, CLI feature expansion, cross-owner storage migrations, REL 2.1 format changes, new model/provider behavior, Hermes/plugin UI work, and old 2.0 conversion.
- Implementing the deferred homogeneous-REL / inter-REL dependency-DAG roadmap as part of documentation reconciliation; document the present difference and keep the future correction independently scoped.
- Large automated document generation with thin content, mirrored descriptions across owners, or additional tool layers solely to make a checker green.

## Completion gates

1. A clean repository-revision documentation audit has **zero unbaselined findings** and **zero remaining baselined material compliance debt**; missing required artifacts, section/index/link errors and broken change mappings are corrected.
2. An implementation-backed review confirms current documentation ownership, complete navigation, developer entry points, documented limits and public/operator contract accuracy.
3. Pitlord's meaningful local architectural rules, the selected Cargo DAG guard and appropriate behavioral tests pass, with no unknown critical bypass simply waived as `not checked`.
4. CI reproduces the local checks, including negative-control tests proving new documentation and architecture violations are rejected.
5. Existing Rust and CLI behavior remains unchanged except for independently approved, separately verified fixes to actual violations.
6. An independent final report supports the precise claim made: structurally compliant, current and complete only if each corresponding engineering-standards definition has been satisfied.

## Related docs

- Local historical evidence: `docs/g4-product-corpus-acceptance-2026-11-06.md`, `docs/public-api-cli-operational-verification-2026-11-07.md`.
- Local current-facing candidates: `docs/public-product-api.md`, `docs/operator-workflow.md`, `docs/configured-runtime.md`.
- Deferred architecture change: `docs/rel-homogeneity-and-dependency-roadmap-2026-11-10.md`.
- Normative sources: `engineering-standards/docs/documentation-standard.md`, `documentation-procedure.md`, `completeness.md`, `adoption.md`, `architecture/architecture-standard.md`, and `architecture/enforcement.md`.

## Notes

This specification creates **no implementation acceptance**. The first work package must measure actual current findings rather than assume the historical 117 remain the current count. Do not overwrite or stage unrelated in-progress work.
