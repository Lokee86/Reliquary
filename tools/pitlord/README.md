# Pitlord architecture policy

Parent index: [Development workflow](../../docs/development/workflow.md)

## Purpose

This policy composes the synchronized engineering-standards architecture policy with Reliquary's local semantic rules. It adds source-level ownership and dependency evidence to Reliquary's existing exact Cargo package guard. The package guard in [`scripts/check_architecture.py`](../../scripts/check_architecture.py) remains the authority for the selected Cargo package set, exact production edge set, cycles, and the two existing Application/Gateway direct-import prohibitions. Pitlord does not replace or relax it. The composition also includes synchronized documentation-core path/content rules; the shared documentation checker independently owns structure, links and change impact. An older installed binary labeled `0.1.2` rejected `require_content`, while the current source build with the same version label supports it. Pin the source revision and validate capabilities rather than assuming version-label equality establishes compatibility.

## Overview

The policy separates responsibilities in `policy.json` from semantic dependency rules in `semantic.json`. The areas follow the current implementation owners: Container and durable stores, owner-session, semantic services, Application, Gateway, the public root API, and the separate CLI. The named rule set:

- assigns file nodes under `src/`, `crates/`, and `cli/src/` to exactly one architectural package or public-surface area, including tests under those package trees;
- rejects Rust dependencies of any category from foundation/service areas into Application or Gateway; and
- rejects CLI dependencies on implementation crates, preserving its Root-only consumer boundary; and
- rejects cycles in the observed cross-area Rust `imports` and `depends-on` graph (including test dependencies).

Pitlord policy v1 does not filter relationship attributes such as Cargo's runtime, development, and build dependency categories. Its local import evidence also includes inline `#[cfg(test)]` modules. Application and Gateway have test-only dependencies on concrete stores, and those tests import store types. The local policy therefore does not ban Application/Gateway-to-store test edges or classify all dependency edges as production. It prohibits the opposite, unsupported foundation/service-to-Application/Gateway orientation in every category, and checks observed cycles including test edges. The current manifests have no dependency in that forbidden orientation. Integration tests belong at the higher orchestration/Root/consumer boundary. The exact Cargo guard remains the source of truth for those production package restrictions.

The canonical ownership and responsibility descriptions belong in [`system`](../../docs/architecture/system.md), [`storage`](../../docs/architecture/storage.md), [`owner-session`](../../docs/architecture/owner-session.md), [`application`](../../docs/architecture/application.md), [`semantic processing`](../../docs/architecture/semantic-processing.md), [`gateway`](../../docs/architecture/gateway.md), and [`switchboard`](../../docs/architecture/switchboard.md) architecture pages. Pitlord area descriptions are concise selectors, not replacements for those owner pages.

## Run the policy

Install compatible Lexicon, Arcana, and Pitlord executables and make them available on `PATH`, or pass executable paths to the wrapper. Rust adapter sources are needed for Lexicon initialization; supply their `adapters/` directory with `--adapters` when it is not adjacent to the Lexicon executable.

```powershell
python scripts/check_policy_tools.py `
  --lexicon <path-to-lexicon> `
  --arcana <path-to-arcana> `
  --pitlord <path-to-pitlord> `
  --adapters <path-to-lexicon-adapters>
```

The wrapper validates the policy, initializes Rust-only Lexicon state on a new checkout or scans an existing one, synchronizes a fresh Arcana snapshot, verifies nonzero file ownership in every declared area plus nonempty global selected-relation and cross-area dependency evidence, and runs Pitlord against that snapshot. Generated state lives in `.lexicon/`, `.arcana/`, and Cargo's shared `target/lexicon-tools/` directory. The wrapper has per-command and whole-run timeouts; any missing tool, failed stage, or timeout fails the command and stops the spawned process tree. `--validate-only` checks policy syntax without claiming graph enforcement. Use `--timeout-seconds` and `--overall-timeout-seconds` only when the repository or runner requires a larger bound.

For a new checkout, initialize Lexicon and then run the graph and architecture gates:

```text
lexicon init --repo . --languages rust --adapters <adapter-directory>
arcana sync --lexicon .lexicon --state .arcana
pitlord validate --policy tools/pitlord/policy.json
pitlord check --repo . --policy tools/pitlord/policy.json --arcana <arcana-executable>
python scripts/check_architecture.py
```

On later runs, use `lexicon scan --repo .` instead of `init`, then synchronize Arcana and run the policy check. The local documentation structure and links are a separate Demon Docs gate: `ddocs check --root docs --indexes --links`. Run it after the canonical documentation indexes exist.

## Evidence boundaries

The local Lexicon Rust adapter contract emits Cargo normal, development, build, target-conditioned, and literal path dependency facts, plus resolved local module-import facts. Pitlord matches named area paths against Arcana node paths and checks the explicit `imports` and `depends-on` relations in the semantic rules. A missing relationship is not proof of architectural absence: unresolved Cargo metadata and dynamic dependencies are omitted, and external dependencies remain external nodes.

The adapter is static. It does not expand procedural macros, execute build scripts, prove unsafe aliasing, infer unconstrained runtime registration, or resolve unconstrained dynamic dispatch. Therefore this gate cannot prove writable-store handle containment, one-writer behavior, authorization, REL/PHY replay correctness, publication semantics, state lifecycle, concurrency, or recovery. Those obligations remain owned by the architecture pages and their behavioral tests. Do not report a Pitlord pass as a behavioral or full architecture acceptance.

## Tool provenance

The investigation used the local Lexicon + Arcana source at `f09517262fdfad98fcede2ad93a947ccee8406ff`, published Pitlord source at `9dedd4b556b9e54de514e4cafe4d94093780178f`, and published Demon Docs source at `45318a8f2602aeebaeea4828894769256773f4f4`. The local installed executables reported Pitlord `0.1.2`, Arcana `0.1.0`, and Demon Docs `0.3.5`; local Lexicon Go and Rust binaries reported `dev`, so their binary provenance is not inferred from their filenames. Record the actual versions and source revisions used for each CI run.

## Related docs

- [Architecture](../../docs/architecture/INDEX.md)
- [Behavioral contracts](../../docs/development/behavioral-contracts.md)
- [Architecture audit](../../docs/acceptance/es04-architecture-audit-2026-11-11.md)

## Notes

No architectural baseline is configured. Missing or incompatible evidence fails the gate; syntactic validation alone is not architecture acceptance.

Tool performance: the unchanged Cargo-run Rust adapter is compiled with dev optimization level 3 and debug information disabled inside the tool process environment. The repository build profile is unaffected. A 600-second optimized initial scan timed out on this checkout; the bounded per-stage allowance is 1200 seconds, with 1800 seconds total. No timeout is reported as a policy pass.

Rust relation semantics: local `imports` can be intra-area while Cargo cross-crate coupling appears as `depends-on`. The gate selects both and requires real cross-area evidence. Root/CLI manifests are owned, and source snapshots/results remain in dated acceptance reports.

The complete gate also creates a temporary policy under excluded `target/standards-policy-controls`, forbids the known real Gateway-to-Application edge on the unchanged snapshot, requires Pitlord to reject that specific rule, and removes its own temporary policy. This proves the dependency gate is active; it does not mutate source or manufacture graph facts.

## Repository-local Windows tools

This remediation provisions tested binaries and pinned Rust adapter runtime sources under ignored `target/standards-tools-bin/` in the installation checkout; it leaves global tools unchanged. `provenance.json` records their hashes. Use explicit paths when an older executable on PATH shares a version label but lacks required capabilities:

```powershell
$policyToolBin = Join-Path (Get-Location) "target/standards-tools-bin"
python scripts/check_policy_tools.py `
  --pitlord (Join-Path $policyToolBin "pitlord.exe") `
  --lexicon (Join-Path $policyToolBin "lexicon.exe") `
  --arcana (Join-Path $policyToolBin "arcana.exe") `
  --adapters (Join-Path $policyToolBin "adapters")
```

These are generated local artifacts, not tracked source or a compatibility layer. A fresh checkout uses the pinned build steps in the CI workflow or installs compatible tools and supplies their paths.
