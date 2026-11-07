# Post-G4 — parallel execution and acceptance plan

Parent index: [Architecture](INDEX.md). Canonical capability tracker: [rebuild capabilities](implementation-rebuild-capabilities-2026-11-03.json).

**Status: execution specifications complete; PG-C01, PG-C02, F03-V01 and G5-T01 are completed at their scoped boundaries; all other ticket and capability statuses remain unchanged.** Baseline: rebuilt `a3c3a964` accepts G4, with production/test changes through `31e7adb2`. The original F03 capability remains complete under its limited historical evidence contract. F03-V01 now accepts copy-only current-head admission; V02-V04 and full historical assurance remain open. D01, D02 and D03 remain not started. G5-T01 freezes the pre-measurement policy; the overall G5 capability remains not accepted and T02-T05 remain not started. Hermes provider/UI are a separate new-native-data product track.

## Purpose

Make the remaining work safe to assign to independent agents without repeating G4's disconnected-fixture problem. Define shared evidence, explicit ownership, executable handoffs and acceptance dependencies before parallel implementation. Do not redesign the accepted 23-package/76-production-edge architecture or reproduce a source-file migration checklist.

## Overview

### Specification set

| Lane | Contract | Intended outcome |
| --- | --- | --- |
| Shared evidence | [Corpus evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md) | One versioned source/target witness and custody boundary |
| Historical source | [F03 deep verification](f03-historical-verification-implementation-spec-2026-11-06.md) | Six independently verified complete FRS07 source witnesses |
| Conversion | [D01 converter](d01-offline-converter-implementation-spec-2026-11-06.md) | Isolated typed create-only 2.1 candidates, no runtime legacy reader |
| Independent verification | [D02 parity verifier](d02-independent-parity-verifier-implementation-spec-2026-11-06.md) | Exact independent history/identity/privacy parity |
| Activation preparation | [D03 staging/rollback](d03-safe-activation-implementation-spec-2026-11-06.md) | Complete staged generation, rehearsed cutover, explicit rollback boundary |
| Release | [G5 measured acceptance](g5-performance-release-acceptance-spec-2026-11-06.md) | Comparable performance and operational release evidence |
| Hermes host | [Provider/service specification](hermes-reliquary-plugin-implementation-spec-2026-11-06.md) | Standalone plugin on accepted native 2.1 owners |
| Hermes UI | [Dashboard port specification](hermes-reliquary-ui-implementation-spec-2026-11-06.md) | Warlock-derived graph/inspector/scopes through real authenticated APIs |

The machine-readable [execution ticket DAG](post-g4-execution-tickets-2026-11-06.json) mirrors the ticket tables in these documents. Each ticket is a bounded behavior/evidence unit, not a placeholder crate. Keep canonical capability numbering; do not rename G4 to a new G04 capability or overwrite completed F03 history.

## Verified baseline and resolved constraints

G4's integrated cohort, full workspace/CLI suites and independent consumer were re-executed successfully. The supplied PowerShell runner has a separately reproduced Windows PowerShell 5.1 issue: expected native stderr under `ErrorActionPreference=Stop` terminates its negative checks. Direct CLI checks pass. PG-C02 fixes the runner without weakening error assertions; this does not reopen product semantics or conceal the earlier failed script run.

F03 sources/copies are hash-intact; the prior historical FRS07 head check passed, but full semantic/history assurance is incomplete. The newer cold compilation timeout is not a test pass. The older F01 reader cannot serve as the FRS07 oracle.

Local Hermes has the MemoryProvider and web-dashboard extension contracts. New providers must be standalone plugins; Warlock UI donors depend on Tauri and need a transport replacement. The user's newer copied-Hermes-input decision is explicitly preserved in the plugin spec, with Hermes retaining canonical history. This differs from the older reference-only plan and does not silently supersede its long-term ownership goal.

Lexicon configuration was absent in the rebuilt checkout. Structural inspection therefore used direct current source and the accepted Cargo DAG; no refreshed graph is claimed. Documentation policy before this task reported 117 existing findings; new files must add none, and unrelated findings must not be hidden by blanket baselining.

## Work and acceptance dependency graph

```text
PG-C01 evidence schema/goldens ─┬─ F03-V02..V04 (also needs F03-V01)
                              ├─ D01-T01..T04 synthetic converter
                              ├─ D02-T01..T03 independent verifier
                              ├─ D03-T01..T02 synthetic staging rehearsal
                              └─ G5-T01..T02 benchmark preparation

F03-V04 + D01-T04 + accepted G4 -> D01-T05 authentic conversion
D01-T05 + D02-T03 + F03-V04    -> D02-T04 authentic independent parity
D02-T04 + D03-T02             -> D03-T03..T04 staged generation/retirement
D02-T04 + F03-V04 + G5-T02     -> G5-T03 comparative measurement
D03-T04 + G5-T03 + PG-C02      -> G5-T04 final release/recovery review
G5-T04 + operator authorization -> G5-T05 actual activation

accepted G4 -> HP01 host/DTO contract -> HP02..HP05 -> HP06
                          └-> HU01..HU02 -> HU03 (also HP02)
HP05 + HU03 -> HU04 real UI acceptance
HP06 + HU04 -> HP07 real demo/submission evidence
```

PG-C01 synthetic contract, F03-V01 copy-only current-head admission, PG-C02 runner repair, and G5-T01 pre-measurement policy freeze are now recorded complete at their scoped boundaries. F03-V02 can proceed with its two hard predecessors satisfied, and G5-T02 runner/adapter preparation may proceed. Comparative G5-T03 measurements remain gated by F03-V04 and D02-T04; full G5 acceptance and release remain open. HP01 can proceed independently now. Converter/verifier/staging/benchmark implementation is parallel-ready; actual authentic conversion remains gated by F03-V04 and accepted G4. A missing acceptance result is not a reason to abandon safe predecessor work.

Canonical capability dependencies remain `G4/F03 -> D01 -> D02 -> D03 -> G5`. The graph above permits provisional implementation where appropriate without declaring those gates complete early. It introduces no circular acceptance dependency between converter and verifier, staged generation and release, or provider and UI demo.

## Shared enabling tickets

| Ticket | Predecessors | Scope/deliverable | Acceptance/evidence |
| --- | --- | --- | --- |
| PG-C01 — freeze evidence implementation contract | None | Strict schemas, canonical serializer goldens, hand-authored source/target semantic fixtures and reporting examples | Independent encoders agree on exact bytes; unknown/duplicate/precision/order/history cases reject; schemas and golden tests committed; F03/D01/D02 review the same version |
| PG-C02 — repair cross-shell acceptance runner | G4 | Correct native-command exit/stderr capture in `scripts/run_g4_acceptance.ps1`, preserving every negative assertion | Same success and eight expected failure cases complete under Windows PowerShell 5.1 and supported PowerShell 7 when available; real JSON/exit/source-hash assertions remain; record unavailable platform explicitly |

No generated witness, converter, verifier, benchmark or plugin exists merely because these specs are written. The CLI names prescribed in lane documents are interfaces to implement; agents must not report them as executed before their binaries/scripts exist.

## Ownership, repositories and integration

| Lane | Writes | Reads / protected assets |
| --- | --- | --- |
| Planning/integrator | These specs, ticket status and canonical capability evidence links | All lane receipts; no bulk staging of other changes |
| F03 | Isolated historical validation instrumentation and private witness outputs | Original six files as immutable bytes; pinned reference never modified |
| D01 | Dedicated converter worktree/tooling; narrow target owner corrections when proven | Frozen source admission; never edits F03 expectations |
| D02 | Separate verifier worktree/tooling and adversarial cases | Read-only source/target bundles; no converter traversal dependency |
| D03 | Staging/rehearsal tooling and approved generation manifest | D02-approved candidates; no live switch during preparation |
| G5 | Measurement runner, raw reports and release decision | Matched source/target copies; machine-exclusive measurements |
| HP/HU | Standalone plugin repository and narrowly scoped host API worktree | Hermes pinned API, Warlock UI donor; neither runtime is transplanted |

Reserve worktrees/branches before mutation; record exact paths and HEADs per ticket. Do not pre-create one worktree or Cargo target directory per ticket. The integrator owns shared Root/owner-session API changes: a D01 representation correction and HP02 public facade change touching the same source must serialize or be rebased and reverified, never overlap blindly. No agent edits another lane's expected witness to make its test pass.

The current planning tree contains an unrelated modified Graph evidence file and many pre-existing untracked artifacts. Stage only this task's files. Keep `Reliquary-reference`, authoritative data, G4 implementation and user caches untouched by specification work. No remote push or corpus publication is authorized.

## Resource policy and practical allocation

Use separate worktrees for truly independent writers, but reuse an appropriate isolated build cache per incompatible runtime. Serialize Cargo commands within each target directory and avoid simultaneous I/O-heavy old/new builds on the space-constrained drive. Check free space before a cold historical build. Preserve incomplete compilation cache; don't delete/recreate targets to restart a timed-out run.

Recommended initial frontier: one agent completes PG-C01/F03-V01; another fixes PG-C02 or pins HP01. Once the evidence contract is frozen, F03 deep export, D01 synthetic converter and D02 verifier can proceed as three independent lanes. D03/G5 preparation and UI work can use additional capacity without changing those artifacts. Benchmarks run in an exclusive measurement window; concurrent code editing is fine, concurrent compilation/model inference is not.

## Verification and status discipline

Every ticket records source/spec revisions, commands/exits/test counts, exact generated artifact hashes, negative cases, remaining limits and a reviewed commit. Accepted tasks have committed evidence and no unresolved changes in their assigned scope. Whole worktree cleanliness is required at integration/acceptance, not before unrelated work can continue. Filtered, ignored, timed-out or partially logged runs remain incomplete.

A real earlier-gate regression requires a red case and fix at its proper owner boundary, plus the relevant accepted regressions. Do not weaken G4/GM/P01 or declare unsupported behavior successful. Product changes invalidate dependent code/artifact measurements and must trigger the affected verifier/performance runs; unchanged data hashes alone are insufficient.

Acceptance artifacts reside with the executing repository; canonical tracker entries point to them. F03 deep acceptance updates `corpus_status` and adds its evidence without rewriting the original limited F03 completion. D01–D03/G5 remain unaccepted until their final tickets pass. `G5 release-ready` and `activated` are distinct until authorized activation completes.

## Documentation and architecture impact

Inspected: G4/GM/P01 evidence, Stage 5/ADR 0049, current source ownership, F03 continuation, Hermes provider/plugin/dashboard surfaces, Warlock UI transport and repository documentation rules. Updated: this specification set, architecture/root indexes and scoped capability breakdown links. Not affected: runtime behavior, source corpus, production DAG, accepted G4 evidence or release routing.

Specification verification completed: all 9 specification documents and 35 task-table entries resolve, local links exist, the machine DAG is acyclic and every task's prerequisites match its prose table. Canonical capability statuses/dependencies are unchanged. Documentation policy reports 117 findings both before and after this task, with **zero new findings**. `ddocs check --indexes --links` still exits 1 naming `docs/INDEX.md` and `docs/architecture/INDEX.md`, the same index paths already reported by G4; no whole-repository documentation pass is claimed. Machine-readable results are retained locally in `.test-tmp/post-g4-spec-review/validation-final.json` with policy before/after and Archivist output.

No Rust/product tests or Pitlord/Arcana source refresh are claimed for these documentation-only additions; implementation owners rerun the architecture guard and relevant runtime gates for actual source changes.

## Out of scope

Starting any implementation lane automatically from this planning request, activating converted owners, new source-format compatibility, full Context Engine/Ego/Observation development, unrelated documentation cleanup, extra business/hosting features or automatic hackathon submission.

## Related docs

- [Execution tickets](post-g4-execution-tickets-2026-11-06.json)
- [Shared evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md)
- [Canonical rebuild plan](../architecture-rebuild-implementation-plan-2026-11-03.md)
- [F03 continuation](../f03-independent-verification-continuation-2026-11-06.md)
- [Independent rebuild ADR](../decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md)

## Notes

The specifications retain the settled architecture and make missing execution contracts explicit. Immutable source truth, independent verification, one current runtime and a single controlled cutover are mandatory. Parallel-ready implementation is not parallel permission to accept downstream gates early.
