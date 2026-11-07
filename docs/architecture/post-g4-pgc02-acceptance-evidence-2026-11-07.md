# PG-C02 cross-shell acceptance evidence — 2026-11-07

## Purpose

Record the reviewed PG-C02 runner results and the evidence limit. This planning-side note links to, but does not replace, the execution receipt. PG-C02 remains in progress rather than accepted until the required raw console output is retained.

## Overview

- Planning/specification revision: 5d96db991a130d84bf76a720b445295f95ec0204.
- Execution baseline: a3c3a9643f494bced51faf92ba057b16bcdac8a7.
- Implementation: fix/postg4-pgc02-acceptance, commit 5169c4293d8531f961591282b61772cde0b31caa.
- Labeled execution receipt and integration head: commit 26e84f45b54985d5aa1ceb3ab8e7fd7f183b8b0a.
- Receipt: Reliquary-rebuild@26e84f45b54985d5aa1ceb3ab8e7fd7f183b8b0a:tests/g4-acceptance-pgc02-receipt-2026-11-07.json.
- The receipt commit is the reviewed implementation branch's fast-forwarded head. The planning revision identifies the specification reviewed; it is not asserted as an ancestor of the execution branch.

## Observed test results

The receipt records full acceptance runs in Windows PowerShell 5.1.26100.9711 and PowerShell 7.6.9. Each run reports exit code 0, two filtered G4 test invocations with two cases passed, one external-consumer reopen case passed, 25 CLI observations, and eight negative CLI invocations that all exited non-zero with structured JSON errors. Both report unchanged source bytes. The exact resolved commands, witness paths, and SHA-256 values are in the linked receipt.

The focused native-capture regression script also reports exit code 0 under both shells. Its eight checks cover runner parsing, preservation of the eight original negative calls, stdout/stderr JSON and exit-code handling, rejection of mixed output, Windows argument quoting, and concurrent 200 KB stdout/stderr capture.

A separate review reported no code defects and independently verified the recorded artifact hashes and counts.

## Notes

The receipt's raw_console_log_files is empty; it explicitly says no standalone raw console logs were persisted. The outcome summary and hashes are reviewable, but the execution transcript is not a retained raw log. The plan says partially logged runs remain incomplete, so this record does not claim PG-C02 acceptance.

Next: rerun the full acceptance and focused checks in both supported shells, retain their complete raw console output alongside the receipt, and request another independent review. Update only the PG-C02 status after that evidence is committed. No Cargo build or product test was rerun for this planning record.

## Related docs

- [Post-G4 parallel execution plan](post-g4-parallel-execution-plan-2026-11-06.md)
- [Canonical execution ticket DAG](post-g4-execution-tickets-2026-11-06.json)
