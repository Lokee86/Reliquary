# Phase 7 — Consolidated implementation and verification record

## State
Production refactor changes were integrated in commit `f431401`. This is a
behavior/source checkpoint, **not final graph acceptance**.

- 924/924 cfg-reachable production modules assigned to components.
- 106 bounded source-cut guards, plus physical, owner, runtime, facade, History
  and External Source guards, passed at the frozen checkpoint.
- Locked regression suite: 2,015 library and 163 integration tests passed,
  zero failures (six ignored); CLI: 32 passed. These were run against the
  frozen implementation.
- Last standalone compiler checker found 78 unpermitted *candidates* across
  28 component pairs. These are not 78 confirmed executable defects.
- The same checker could not resolve 58,080 calls, mostly generic/local call
  mapping and destructor glue. They are **not** a production defect count.
- Fresh full Lexicon/Arcana/Pitlord architecture acceptance was not achieved.

## Simplification decision (2026-10-25)

Retire the redundant repository-local compiler-analysis toolchain, ticket
registry, huge generated captures, and per-finding fingerprint reconciliation.
All were preserved in repository history at `f431401` before deletion. The
previous 320-to-78 comparison used different checker revisions and is not a
controlled rate of architectural improvement.

Keep the actual source-level owner checks, component mapping, 34-node logical
DAG, canonical Pitlord policy, Rust tests, and real behavior/authorization
contracts. The architecture command now runs fast source guards by default;
`--graph --refresh` invokes the existing Lexicon/Arcana/Pitlord tools.

## Remaining concrete work

Review genuine remaining dependency inversions using normal source
inspection, ripgrep and external graph tooling. Do not add new abstraction
layers just to make every value-constructor call appear layer-local. Distinguish
live implementation defects from analyzer coverage. Consolidate wrappers only
when admission, error mapping, and externally supported signatures are
preserved and tests succeed.

Phase 7 remains **open** until known actionable violations are eliminated
and a current graph inspection is completed or its limitations explicitly
accepted as limitations of the shared analysis tools.

The archived exhaustive phase ledgers and historical terminal outputs can be
recovered from commit `f431401` with `git show`.
