# Adoption and Enforcement

Parent index: [Engineering Standards](INDEX.md)

## Purpose

This document defines how repositories adopt and enforce the shared documentation and architecture standards.

## Overview

The shared repository owns the normative standards, profiles, documentation checker, reusable Pitlord policies, templates, and adoption registry. Each product repository owns its current documentation, architecture areas, local specialization, exceptions, baselines, and CI configuration.

Documentation enforcement uses the shared checker and repository `docs-standard.json`. Architectural enforcement uses Pitlord as the expected deterministic policy engine, backed by Lexicon and Arcana for semantic graph evidence. Architecture remains repository-specific, so rollout and semantic policy are adopted repository by repository rather than copied as one universal area graph.

## Documentation adoption

1. Select a primary profile and capabilities from [Repository profiles](profiles.md).
2. Add `docs-standard.json` using the template.
3. Ensure `README.md`, `AGENTS.md`, documentation index, policy, procedure, planning, limits, coverage, behavioral-contract, and maintainer-map owners exist or add explicit justified exemptions.
4. Configure the repository's root and folder index convention.
5. Map production paths to canonical current documentation.
6. Add focused change-impact rules.
7. Run the shared checker and record existing debt explicitly.
8. Configure Demon Docs for index, link, frontmatter, move, and managed-surface maintenance when appropriate.
9. Include the reusable Pitlord documentation policy or equivalent repository rules.
10. Add CI checks and remove baselines as debt is repaired.

## Architecture adoption

1. Identify the repository's durable responsibility areas from its canonical architecture documents.
2. Add `tools/pitlord/policy.json` as the local policy composition root.
3. Include `.standards/policies/architecture-core.json` after syncing the shared standards snapshot.
4. Add `tools/pitlord/semantic.json` with meaningful ownership, forbidden-dependency, and cycle rules appropriate to the repository.
5. Add repository path/content rules for generated state, prohibited legacy surfaces, required architecture controls, and other deterministic invariants where applicable.
6. Add `tools/pitlord/README.md` describing policy validation, Lexicon preparation, Arcana synchronization, Pitlord execution, timeout behavior, baselines, and troubleshooting.
7. Map every rule to a canonical architecture owner and protecting rationale.
8. Add focused tests or runtime scenarios for lifecycle, concurrency, failure, recovery, and behavioral invariants Pitlord cannot observe.
9. Add the Pitlord gate to CI.
10. Remove rollout baselines as existing findings are repaired.

See [Architectural enforcement with Pitlord](architecture/enforcement.md) for the normative division of responsibility and policy-design rules.

## Tool ownership

### Shared checker

Owns deterministic cross-repository documentation checks for required paths, indexes, direct index coverage, relative links, required sections, configured coverage owners, and change-impact mappings.

The canonical implementation lives in `tools/docs_policy/`. `tools/sync_checker.py` publishes generated snapshots under each adopted repository's `.standards/` directory so CI does not depend on a sibling checkout or unavailable network resource. The generated manifest records exact SHA-256 identities. Generated snapshots are not edited directly.

The checker inventories repository paths for coverage rules while **pruning generated and transient directories before traversal** (including Rust `target/`, Git metadata, worktrees, dependency directories and Python bytecode caches). Such directories are not documentation coverage inputs and may contain short-lived test output; ordinary source and documentation directories remain checked, and unexpected traversal errors remain failures.

### Demon Docs

Owns deterministic maintenance and verification of documentation indexes, links, frontmatter, moves, reverse indexes, managed codemaps, and repository-local document policy.

### Pitlord

Owns deterministic repository and architectural policy evaluation, including required or forbidden paths/content, ownership coverage, forbidden dependencies, area cycles, evidence, baselines, and machine-readable reporting.

Reusable adoption policies live under `policies/pitlord/`. Repository-specific semantic architecture remains in the repository that owns the architecture. Pitlord does not replace behavioral tests, runtime scenarios, ADRs, or design review.

### Lexicon and Arcana

Lexicon owns language analysis and normalized facts. Arcana owns immutable graph snapshots and graph queries. Architecture-enforcing Pitlord rules consume these boundaries rather than parsing source or reading graph storage directly.

### Warlock

Should surface compliance state, failed checks, missing owners, and repair actions. It is a presentation and orchestration surface, not the canonical policy owner.

## CI contract

The minimum documentation gate inside an adopted repository is:

```bash
python .standards/docs_policy/check.py --repo .
```

Pull requests should also run:

```bash
python .standards/docs_policy/check.py --repo . --changed-from origin/main
```

Architecture-enforced repositories also run their repository-owned Pitlord wrapper. The wrapper must validate policy, prepare current Lexicon and Arcana evidence, run Pitlord with a bounded timeout, and fail on invalid, incompatible, or truncated evidence.

Repositories using Demon Docs add their local maintenance or verification commands alongside these gates.

Refresh the generated standards snapshot from the canonical repository with:

```bash
python tools/sync_checker.py /path/to/repository
```

## Baselines

Create or refresh an explicit documentation baseline with:

```bash
python /path/to/engineering-standards/tools/docs_policy/check.py \
  --repo . \
  --write-baseline docs-standard.baseline.json
```

Documentation and Pitlord baselines may suppress known legacy findings during rollout, but:

- every suppressed item remains visible as debt;
- new failures are blocked;
- a baseline is not evidence of compliance;
- the repository policy states the removal plan;
- semantic gaps are never hidden solely by a machine baseline;
- normal check output reports the number of baselined findings;
- fixed findings become stale baseline entries and should be removed.

## Adoption registry

`adoption/repositories.json` records the selected profile, enforcement state, and known migration status for active repositories. It is coordination data, not a substitute for repository-local configuration.

Architecture adoption may be recorded separately from documentation adoption when rollout is incomplete. A repository is not architecturally enforced merely because it contains architecture documents or has Pitlord installed; meaningful local semantic rules and an active gate are required.

## Related docs

- [Documentation standard](documentation-standard.md)
- [Repository profiles](profiles.md)
- [Completeness and status claims](completeness.md)
- [Architecture standard](architecture/architecture-standard.md)
- [Architectural enforcement with Pitlord](architecture/enforcement.md)

## Notes

Repository-local rules may be stricter. Space Rocks' type-specific service/protocol/data/design taxonomy and Demon Docs' coverage and behavioral-contract rules remain valid specializations.

Pitlord is the expected architecture-enforcement mechanism. This standard does not authorize unreviewed bulk propagation of repository-specific architecture policies; each rollout must encode the actual architecture of the repository being governed.