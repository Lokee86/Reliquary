# D03 — verified generation staging, rollback and converter retirement

Parent index: [Architecture](INDEX.md). Parent plan: [post-G4 execution](post-g4-parallel-execution-plan-2026-11-06.md).

**Status: implementation-ready specification; D03-T01 has an isolated documentation checkpoint in progress; overall D03 remains unaccepted.** D03 prepares and rehearses activation. It does not activate production before G5, and does not imply that six file renames form one atomic operation.

## Purpose

Avoid a mixed-format/mixed-generation deployment, accidental writes to rollback data, and unacknowledged loss of new writes during rollback. Preserve access to the existing system while the converted replacement is independently validated and measured.

## Overview

Stage a complete immutable candidate generation after D02. Bind its six owners, optional operational sidecars, binary versions and expected digests into one validated manifest. Rehearse release-stop, validation, selector change, admission, rollback and fault handling against disposable deployment directories. Retire conversion machinery from all shipping paths. G5 performs final performance/release acceptance before any authorized real selector change.

## Target architecture and decisions

### Deployment inventory and selector

D03-T01 inventories the actual host/CLI launch configuration and owner-path consumers; do not invent a selector that no consumer uses. Prefer one same-volume atomic replacement of a small generation-selection manifest consumed by every relevant opener. Where an existing consumer cannot follow that selector, use an explicit stopped-release path update protocol with all writers disabled until every required path is verified. No background dual-write period or old-format fallback.

A generation manifest includes schema/version, generation ID, runtime artifact hashes, the complete REL/PHY pairing and scope map, relative paths confined to its generation directory, per-file hashes, D02 report/hash, operational journal policy, previous generation identifier and activation preconditions. Validate canonical paths, aliases/symlinks, duplicate owner UUIDs, kind/scope mismatch, missing/extra required files and directory traversal. Secrets remain outside the manifest.

The runtime selected for the new generation must be the current-only 2.1 artifact that passed D02/G5. A launch must reject a stale/mismatched runtime or unverified generation rather than falling back to the old reader. Pinned source code/data are rollback assets, not linked runtime dependencies.

### State machine

`Unverified -> VerifiedCandidate -> StagedReadOnly -> Rehearsed -> ReleaseReady -> ActivatedReadOnly -> Writable`

Only D02 can establish VerifiedCandidate; D03 establishes StagedReadOnly/Rehearsed; G5 establishes ReleaseReady. Real activation requires a recorded operator authorization and the same validated manifest digest. The first writable transition is a distinct recorded boundary. No incomplete candidate may be named by an active selector. Persist step/manifest identity before changing routing so crash recovery can determine which generation was selected.

### Quiescence and pending work

Stop new owner writes, drain or durably hold accepted work, close writer leases, verify no surviving process is writing the source generation, and revalidate hashes under that stop. Never kill unrelated processes or delete caches as a cutover shortcut. Capture queue/stream/host operation state. Unknown external outcome remains Held until the host can prove its disposition; do not redispatch it merely because files moved. A stale credential, owner epoch or stream cursor must not gain authority after remount.

After selection, open all required owners read-only, check exact owner IDs/scopes/history proofs and operational receipt alignment, then run bounded authorized health checks. Only the G5-authorized writable step resumes producers. Stop on any discrepancy; no partially active owner set.

### Rollback boundary

Before the first new-generation write, rollback may restore the previous validated selector/configuration and reopen the untouched previous runtime/data. Test this exact path, including failure after selector replacement but before health checks.

After any new accepted write, blindly switching to old data loses work and is forbidden. Stop producers, preserve the new generation and accepted receipts, and choose forward recovery or a separately verified reconciliation/export plan. This specification does NOT authorize an unimplemented reverse converter or promise lossless automatic downgrade. The runbook must display whether the write boundary was crossed.

### Retirement and retained custody

Remove converter binaries, old decoder dependencies, temporary migration flags and tool entry points from shipping packages, normal command help, installers and runtime search paths. Keep immutable source hashes, conversion receipts, parity evidence and the existing rollback assets in their approved private custody location. An archived offline evidence snapshot is not a runtime fallback. Do not delete authoritative originals, unrelated worktrees or other agents' build caches.

Commit converted development data and source/postimage manifests only to the existing authorized fixture repository, respecting its ignore/private-data rules. Never add conversation payloads to the public code repo or push a remote without authorization. If that repository cannot legally/technically carry the data, record the conflict for a deliberate custody decision; do not claim data committed from a local untracked directory.

## Tickets

| Ticket | Hard predecessors | Deliverable | Acceptance and evidence |
| --- | --- | --- | --- |
| D03-T01 — inventory and generation contract | PG-C01 | Actual consumer/path inventory, selector or stopped-release binding, state machine and rollback runbook | Every opener accounted for; no mixed-generation path; schema rejects missing/wrong-owner/aliased/escaped files; synthetic tests |
| D03-T02 — staging and fault rehearsal | D03-T01 | Create-only staging and selector/configuration transition tooling | Faults before/after sync, selector replacement, partial path updates and health checks leave either old active or new wholly fenced; stale writer/receipt negatives |
| D03-T03 — stage verified authentic generation | D03-T02, D02-T04 | Private immutable six-owner generation and rehearsal record | Source and candidate hashes unchanged; all owners current-only open; prewrite rollback restores exact previous state; postwrite rollback refuses blind downgrade |
| D03-T04 — retire tooling and hand off | D03-T03 | Shipping dependency/command audit, committed data/custody manifest, G5-ready artifact | No old reader/converter in release graph/install; G4 regressions; reviewed activation/recovery runbook; no real activation performed |

## Testing decisions and operational commands

Use an isolated fake deployment root for every destructive/fault test. The selected implementation must expose `stage`, `validate`, `rehearse`, `inspect-state` and `rollback-prewrite` operations with explicit generation/manifest digests. `activate` and `enable-writes` are separately authorized G5 operations, not defaults of `stage`. Failures return structured state and nonzero status; logs must not expose credentials or private content.

Mandatory cases: torn/missing manifest, corrupt file, wrong kind/UUID, mixed generation, stale D02 hash, owner still writable, unrelated process, unresolved host operation, destination collision, unavailable filesystem atomic primitive, restart at each state, rollback before/after write boundary, and old runtime invoked against new data. No observed error may be reclassified as permission to repair data automatically.

Run current runtime/CLI/host regression suites and architecture/dependency checks against the exact staged runtime artifact. Revalidate file hashes after health/read tests and record all command exits and operator-visible state. The PowerShell 5.1 negative-command runner defect from G4 must be corrected or a validated cross-version runner used; expected nonzero exits must be asserted, not swallowed.

## Completion gate and user outcome

D03 finishes with one reviewed immutable generation, a tested all-owner activation procedure, an explicit no-data-loss rollback boundary and a release package free of old-format machinery. G5 can benchmark and rehearse against copies of that generation without touching active data. The user retains a known working rollback source until the release gate succeeds.

## Out of scope

Automatic activation during specification/implementation, live dual writes, arbitrary fleet/cloud rollout, lossless rollback after unexported new writes, deleting rollback assets, or silently adding a backward converter.

## Related docs

- [Shared evidence contract](post-g4-corpus-evidence-contract-2026-11-06.md)
- [D02 parity](d02-independent-parity-verifier-implementation-spec-2026-11-06.md)
- [G5 release gate](g5-performance-release-acceptance-spec-2026-11-06.md)
- [Independent rebuild ADR](../decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md)

## Notes

D03 preparation may be implemented before authentic D02 results using synthetic manifests. Actual source staging and retirement acceptance wait for D02. The D03->G5 order is intentionally acyclic: preparation first, measured release decision second, real activation last.
