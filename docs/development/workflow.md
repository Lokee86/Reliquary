# Development workflow

Parent index: [Development](./INDEX.md)

## Purpose

This guide describes a repeatable, scoped workflow for changing Reliquary and verifying the affected behavior.

## Overview

The workspace contains the Root consumer package, a separate CLI package, and focused implementation crates. Responsibility ownership and documentation routes are in the [maintainer map](../architecture/maintainer-map.md); implementation and behavioral coverage are tracked in [coverage](documentation-coverage.md) and [contracts](behavioral-contracts.md). Architecture pages explain boundaries and persistent state.

## Prerequisites

- Windows or another supported development environment with Rust toolchain for edition 2024 and Python for repository scripts.
- A clean understanding of the relevant owner page, existing tests and current Git diff.
- For Windows pytest, use `python -m pytest --basetemp .pytest_tmp`; for Go cache use `.go-cache`. Never use `C:\tmp`.
- Provider credentials and source corpora are not needed for ordinary unit/contract tests. Do not use them as disposable test input.

## Change procedure

1. Identify the owner from the relevant architecture page. Current paths include [system](../architecture/system.md), [storage](../architecture/storage.md), [owner session](../architecture/owner-session.md), [application](../architecture/application.md), [semantic processing](../architecture/semantic-processing.md), [gateway](../architecture/gateway.md) and [Switchboard](../architecture/switchboard.md).
2. Read the nearest existing tests and confirm how the behavior crosses Root or CLI before changing implementation.
3. Keep the change at its owning layer. Update public docs, CLI reference, coverage mappings and behavioral contract routes for externally visible changes.
4. Run focused relevant tests first. For applicable workspace Rust changes, use:

   ```powershell
   cargo fmt --all --check
   cargo check --workspace --locked --offline -j 1
   cargo test --workspace --locked --offline -j 1 -- --test-threads=1
   python scripts/check_architecture.py
   ```

   For CLI changes, also run `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`. Use `python -m pytest --basetemp .pytest_tmp` for pytest checks where relevant.
5. Review the final diff, capture exact commands and outcomes, and commit the completed unit.

Expected result: formatting, compilation, relevant tests and dependency checks pass for the changed boundary, and documentation describes the implementation and tested failure/recovery behavior. A passing package test is not full product or standards-compliance acceptance.

For Community recompute, the focused Root test covers archived graph members. The real-corpus acceptance runs preview, apply, and fresh reopen on an inactive copy according to the [Leiden execution plan](../planning/leiden-community-recomputation-plan-2026-11-18.md).

## Documentation and policy gates

Run from the repository root, with compatible `ddocs`, Pitlord, Lexicon and Arcana tools installed as described in [policy operations](../../tools/pitlord/README.md):

```powershell
python scripts/check_standards_snapshot.py
python scripts/check_documentation_entrypoints.py
python .standards/docs_policy/check.py --repo .
python .standards/docs_policy/check.py --repo . --changed-from origin/main
ddocs check --root docs --indexes --links --no-global-config
python scripts/check_gate_controls.py
python scripts/check_architecture.py
python scripts/check_policy_tools.py
cargo test --manifest-path tests/external-consumer/Cargo.toml --locked --offline -j 1 -- --test-threads=1
```

The required fresh-process acceptance step uses the same repository-local Cargo target directory as the other CI builds. From PowerShell at the repository root, run:

```powershell
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target' }
New-Item -ItemType Directory -Force $env:CARGO_TARGET_DIR | Out-Null
.\scripts\run_g4_acceptance.ps1 -TargetDirectory $env:CARGO_TARGET_DIR
```

The script creates a synthetic authored G4 cohort, runs its Gateway completion test twice and compares semantic witnesses for determinism, builds the CLI and Root-only external consumer, and explicitly runs the ignored process-reopen consumer test. It then compares fresh-process public Root reports with CLI reports and exercises CLI failure cases while checking the exported REL/PHY bytes remain unchanged. This gate needs no provider credentials and does not read or mutate a user corpus. It writes evidence under `.test-tmp/g4-completed-<run-id>/`: `witness.json`, `process-witness.json`, and `cli-witness.json`. A second deterministic-generation witness is kept in the adjacent `g4-completed-<run-id>-repeat/` directory. Pass the existing shared target directory with `-TargetDirectory`; omitting it makes the script use its own `target-codex-check` fallback.

CI checks tool sources out under the adapter-excluded `target/standards-tool-sources/` tree so analysis does not ingest its own tools. It builds the pinned tool revisions in `.github/workflows/standards.yml`; tool-source access must be available to the runner. If those repositories are private, configure `STANDARDS_TOOLS_TOKEN` with read access. Restricted fork runs without that access fail the required gate rather than skipping it. Action versions are explicit tags; tool source and Rust/Go/Python toolchain versions are pinned as declared in the workflow. No hosted run is implied by a local audit.

Root README/AGENTS links are outside the shared checker’s configured `docs/` scope and are independently checked using its synchronized Markdown link parser. The negative controls include a broken root entry-point link.

The external consumer is a separate Cargo workspace restricted to Root imports; workspace tests alone do not run it. The CI workflow uses the actual PR target base with full Git history; the shared change-impact checker compares committed `base...HEAD` and excludes uncommitted work. For local staged work, first commit the completed unit before claiming change-impact acceptance. The negative-control script copies tracked files into a temporary repository under adapter-excluded `target/standards-gate-controls/`, proves the clean documentation/DAG baseline, then verifies missing required docs, broken links, unmapped public changes, unrelated-owner changes and forbidden package edges fail. It removes its own fixture.

`ddocs fix --root docs --indexes --no-global-config` reconciles managed navigation after authored changes; inspect its diff, then rerun check.

### Fresh-checkout Demon Docs link state

A fresh checkout has no Demon Docs identity state. The CI job first requires a clean `docs/` tree, then runs `ddocs fix --root docs --links --no-global-config` once to initialize its private state. The first pass records link identities and reports existing issues without repairing authored links. This internal link state is not a compliance-finding baseline: `ddocs check --root docs --indexes --links --no-global-config` runs afterward and still fails on broken links.

CI checks the tracked and untracked `docs/` tree both before and after initialization. Any authored-document edit or new document caused by `ddocs fix` fails the job before the checker runs. The generated tool state under `docs/.ddocs/` is ignored by Git and is not committed. Do not auto-repair documentation in CI and then accept the repaired result as a pass.

Historical bodies remain evidence; parent navigation can be repaired. No real provider or corpus is touched by this procedure.

## Recovery

If offline Cargo cannot resolve a required dependency, report the exact command and missing package; do not switch to a network build without an approved project workflow. If a focused test fails, preserve its output and repair or revert the responsible change before running broader gates. If a check or source tool is unavailable, record it as a blocker instead of describing the check as passed. Never remove a user corpus or unrelated worktree cache to free space.

## Related docs

- [Maintainer map](../architecture/maintainer-map.md)
- [Coverage map](documentation-coverage.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Planning](../planning/roadmap.md)
- [Known limits](../limits/current-limitations.md)
- [Documentation index](../INDEX.md)

## Notes

The standards snapshot is generated by its authority; do not edit `.standards/` directly. Record actual gate outcomes and tool versions in dated acceptance evidence. Structural success alone does not establish semantic completeness.

The semantic wrapper rejects empty or partial Rust area evidence before checking rules. Its tool-only optimization and bounded scan allowances are documented in policy operations; production Rust build profiles are unchanged. The architecture negative controls also verify that aliased, target-conditioned, workspace-inherited, and externally replaced internal runtime dependencies are rejected, while an aliased dev dependency remains test-only.
