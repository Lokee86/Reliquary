# Reliquary architecture checks

The maintained policy is `policy.json`, including the shared engineering
standard and `semantic.json`. The target logical ownership DAG is
`convergence-component-dag.json`; cfg-reachable production file ownership is
`production-components.json` and owner import constraints are in
`owner-boundaries.json`.

Use `python scripts/check_architecture.py` at the repository root to run the
logical DAG, source mapping, source-cut, physical, owner, runtime, facade,
History and External Source boundary checks. This does **not** claim exhaustive
call-graph verification.

To inspect a repository graph, run
`python scripts/check_architecture.py --graph --refresh` with Lexicon, Arcana,
Pitlord and the normal Rust adapter available. This uses those existing tools;
it does not build a separate Reliquary compiler analyzer or maintain finding
fingerprint ledgers.

Both source checks and graph results are subject to their coverage limitations:
macro expansion, dynamic dispatch and unused/alternate cfg profiles must not
be silently treated as proven. An incomplete scan is a tool limitation, not a
claim that the repository contains thousands of architectural violations.

The large Phase 0–7 snapshots, exact-finding registries and dedicated Phase 7
compiler-analysis scripts were retired on 2026-10-25. Historical copies remain
in Git at `f431401`. The current design and outstanding corrections are in
[the dependency convergence specification](../../docs/dependency-convergence-architecture-refactor-2026-10-21.md)
and [the Phase 7 execution record](../../docs/dependency-convergence-phase7-execution-2026-10-24.md).
