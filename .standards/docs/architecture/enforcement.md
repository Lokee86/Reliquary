# Architectural Enforcement with Pitlord

Parent index: [Architecture Standards](INDEX.md)

## Purpose

This document defines Pitlord as the expected deterministic architectural enforcement mechanism for Laughing Skull repositories.

## Overview

Architecture remains repository-specific, but architectural invariants must not remain prose-only when they are statically detectable. Pitlord is the canonical policy engine for ownership coverage, dependency direction, forbidden coupling, area cycles, required or forbidden repository surfaces, and other deterministic architecture rules.

The shared standards repository defines the enforcement model and reusable adoption policy. Each product repository owns its architecture areas, dependency rules, exceptions, baselines, and CI integration. There is no universal area graph that can be copied unchanged into every repository.

## Enforcement ownership

Pitlord owns deterministic architectural policy evaluation, evidence, baselines, and machine-readable reporting.

Lexicon owns language analysis and normalized facts. Arcana owns immutable graph snapshots and graph queries. Pitlord consumes those boundaries; it does not parse source languages or read Arcana storage directly.

Focused tests own behavioral, lifecycle, concurrency, failure, recovery, and compatibility invariants that cannot be proven from the static graph. Runtime scenarios own multi-process and operational behavior. Architecture documents and ADRs own intent, tradeoffs, and current responsibility boundaries.

Pitlord is therefore the expected architecture gate, not the only architecture evidence.

## Required repository surface

A repository adopting architectural enforcement should own:

```text
tools/pitlord/policy.json
  composition root for shared and repository-local policies

tools/pitlord/semantic.json
  repository-specific areas, ownership rules, dependency rules, and cycle rules

tools/pitlord/README.md
  preparation, execution, timeout, baseline, and troubleshooting procedure
```

The policy composition root should include the generated shared adoption policy when available:

```text
../../.standards/policies/architecture-core.json
```

It should also include repository-local path/content rules and semantic rules. Merely requiring the policy files is not architectural enforcement; the semantic policy must encode real invariants derived from the repository's documented architecture.

## Expected rule coverage

Use Pitlord when an invariant can be expressed as deterministic repository or graph evidence, including:

- every production file belongs to exactly one declared ownership area where that model is appropriate;
- foundational packages do not depend on higher-level runtime or orchestration packages;
- independently usable components do not import host-specific implementations;
- protocol, domain, storage, UI, tooling, and composition-root dependency direction remains explicit;
- forbidden reach-through and bypass dependencies remain absent;
- selected ownership areas remain acyclic;
- generated state, legacy surfaces, or prohibited compatibility files are absent where repository policy requires that;
- required architecture-policy and operational surfaces remain present.

Do not create speculative rules for hypothetical failures. Add or strengthen a rule when the architecture declares a durable invariant and the active language adapters provide reliable evidence for it.

## Policy design rules

- Rules must map to a canonical architecture owner and a stated invariant.
- Area names should express responsibilities, not arbitrary folder partitions.
- Prefer direct `imports` rules for compile-time dependency direction. Add `calls`, `references`, `extends`, or other relations only when adapter evidence is reliable and the invariant requires them.
- Keep composition roots and explicitly approved adapters outside lower-level ownership areas when they intentionally connect components.
- Fail closed on invalid policy, incompatible protocol, invalid pagination, or truncated graph evidence.
- Keep exceptions narrow and visible in current architecture or limits documentation.
- A warning is not an adequate replacement for an error when violating the rule would create duplicate authority or forbidden dependency direction.

## Architecture-change contract

When a change adds or changes an architectural invariant:

1. Update the canonical architecture owner and any ADR.
2. Decide whether the invariant is statically detectable.
3. If detectable, add or update the repository's Pitlord areas and rules in the same change.
4. Add focused tests for behavior Pitlord cannot observe.
5. Validate the policy, refresh Lexicon and Arcana state through the repository-owned procedure, and run Pitlord.
6. Report any rule intentionally deferred, including why current evidence is insufficient and what would make enforcement reliable.

A repeated ownership violation, forbidden dependency, bypass, or cycle is strong evidence that a Pitlord rule should be added rather than relying on another review reminder.

## CI contract

The repository-owned wrapper should perform the equivalent of:

```bash
pitlord validate --policy tools/pitlord/policy.json
lexicon scan --repo .
arcana sync --lexicon .lexicon --state .arcana
pitlord check \
  --repo . \
  --policy tools/pitlord/policy.json \
  --arcana arcana \
  --timeout 5m
```

Initialization, adapter discovery, state paths, and executable resolution remain repository-owned. Generated `.lexicon/`, `.arcana/`, and related analysis state must remain excluded from authored-source traversal and version control unless a repository explicitly owns a fixture.

## Baselines and rollout

Architectural enforcement is adopted repository by repository because meaningful areas and dependency rules are local architecture. The expected destination is an unbaselined Pitlord gate.

A temporary baseline may support initial rollout, but:

- every suppressed finding remains visible debt;
- new findings are blocked;
- the baseline has a named removal owner and plan;
- duplicate authority and newly introduced forbidden dependencies are not normalized as acceptable architecture;
- stale resolved entries are removed promptly.

Adding Pitlord to the shared standard does not authorize an automatic bulk rewrite of every repository policy. Repository rollout remains explicit and reviewed.

## Related docs

- [Architecture standard](architecture-standard.md)
- [Architecture procedure](architecture-procedure.md)
- [Ownership and dependency direction](ownership-and-dependency.md)
- [Testing, evolution, and decisions](testing-evolution-and-decisions.md)
- [Adoption and enforcement](../adoption.md)

## Notes

Pitlord should make declared architecture difficult to violate accidentally. It does not decide architecture on behalf of the owning repository and must not replace design review, focused tests, ADRs, or operational verification.