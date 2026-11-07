# PG-C02 cross-shell acceptance evidence — 2026-11-07

## Purpose

Record the accepted cross-shell runner repair and the source-side test evidence. The test outputs live in the execution repository; this planning record links to the committed receipt and summarizes its scope.

## Overview

- Planning/specification revision: 5d96db991a130d84bf76a720b445295f95ec0204.
- Execution baseline: a3c3a9643f494bced51faf92ba057b16bcdac8a7.
- Implementation: fix/postg4-pgc02-acceptance, commit 5169c4293d8531f961591282b61772cde0b31caa.
- Reviewed integration checkpoint: C:\!bin\workspace\Reliquary-postg4-pgc02-integration-2026-11-07, branch codex/postg4-pgc02-integration-2026-11-07, HEAD 6bf398b9fd005d6f870413111f20bbc40504ace6, tree 3c0069e7cfde98b0375744c35aa2480f65c0bc81. It was clean at verification.
- Receipt: Reliquary-rebuild@6bf398b9fd005d6f870413111f20bbc40504ace6:tests/g4-acceptance-pgc02-receipt-2026-11-07.json.

## Observed test results

Windows PowerShell 5.1.26100.9711 and PowerShell 7.6.9 full acceptance runs report exit code 0. Each records two filtered G4 test invocations with two cases passed, one external-consumer reopen case passed, 25 CLI observations, and eight negative CLI invocations that all exited non-zero with structured JSON errors. Both report unchanged source bytes.

The focused native-capture regression script reports exit code 0 in both shells. Its eight checks cover runner parsing; retention of all eight negative calls; stdout/stderr JSON and exit-code handling; mixed-output rejection; Windows argument quoting; and concurrent 200 KB stdout/stderr capture.

The first Windows PowerShell attempt with a contaminated process module path stopped before CLI observation; the receipt preserves it as a failed diagnostic, not acceptance evidence. A clean process-local module-path rerun passed without changing machine/user settings. The source receipt and focused tests were independently reviewed as accepted.

The rerun directory tests/evidence/pgc02-rerun-2026-11-07 contains 24 stdout/stderr logs. Git attribute /tests/evidence/pgc02-rerun-2026-11-07/*.log -text preserves their bytes. All 24 log hashes match their per-run metadata and the exact Git blob bytes at the 6bf398b9 integration head.

## Notes

PG-C02 is completed at the runner-repair acceptance boundary only. The receipt's top-level planning_revision field remains null; this planning record binds the run to the full plan revision above, while the run metadata carries its abbreviated prefix. The receipt commit records all test results and artifact hashes; no Cargo command was rerun for this planning update.

## Related docs

- [Post-G4 parallel execution plan](post-g4-parallel-execution-plan-2026-11-06.md)
- [Canonical execution ticket DAG](post-g4-execution-tickets-2026-11-06.json)
