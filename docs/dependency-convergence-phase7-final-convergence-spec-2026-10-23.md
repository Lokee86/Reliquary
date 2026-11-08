# Phase 7 — Dependency convergence acceptance

Phase 7 is **not architecture-accepted**. Production corrections from Phases 1–7
are integrated at commit `f431401`, with passing Rust regression tests.
Do not reopen completed source moves merely to satisfy compiler-tooling receipts.

## Intended boundaries
- Physical storage cannot invoke semantic owners or runtime code.
- Semantic stores do not import the public facade or runtime orchestration.
- Durable owners own mutation and admission; cross-owner orchestration stays above them.
- Runtime composes services; public Cva/Phylactery APIs remain stable.
- The accepted 34-component logical DAG is `tools/pitlord/convergence-component-dag.json`.
  Treat it as a design constraint; it does not prove a source graph is conformant.

## Completion conditions
1. Review the known concrete cross-boundary calls and correct **actual executable
   responsibility inversions**. Harmless immutable values and error types do not
   require new forwarding layers merely to avoid a lexical reference.
2. Run `python scripts/check_architecture.py` for the maintained source
   boundaries and negative fixtures, and `cargo fmt --check`,
   `cargo check --locked`, and `cargo test --locked`.
3. Validate the source graph using existing Lexicon/Arcana/Pitlord tooling when
   available: `python scripts/check_architecture.py --graph --refresh`.
   State scan incompleteness as a tool limitation, never as 58,000 presumed defects.
4. Preserve the exact supported public APIs, persisted state, and rejection/
   authorization behavior. Document any remaining concrete structural violations.

The legacy Reliquary-specific Rustdoc/MIR/HIR/THIR compiler analyzer, raw
fingerprint ledgers and 68-ticket counting process have been **retired** as
unnecessary local duplicate infrastructure. Their history remains in Git at
`f431401`; this change does not waive actual architectural violations.

See [execution and verification](dependency-convergence-phase7-execution-2026-10-24.md)
and [target DAG](dependency-convergence-target-dag-2026-10-21.md).
