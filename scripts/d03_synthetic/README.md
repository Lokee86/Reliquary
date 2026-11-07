# D03 synthetic staging harness

Status: provisional rehearsal foundation only. This is not D03 acceptance, a
product deployment tool, or a selector consumed by any Reliquary opener. The
manifest schema remains provisional pending PG-C01 review and integration.
T01/T02 acceptance remains incomplete. Real opener and consumer inventory,
selected stopped-release binding, contract integration, and an operator rollback
runbook are separate requirements.

`stage` creates six tiny fake JSON owners, a fake 2.1 runtime marker, and a
synthetic D02 `PASS` marker. It accepts no source paths or arbitrary owner data.
It only creates a new `d03-synthetic-*` directory directly under OS temp.
Other operations refuse roots without the harness marker.

The simulated selector is read only by this harness. It changes no host, CLI,
library, or Gateway path binding. A pending intent is recorded before fake
selector replacement. After a process restart, operations reconcile an
unchanged pointer to staged/old or a replaced pointer to candidate-fenced or
rolled-back. Selected failpoints use subprocesses to exit abruptly around
rehearsal and rollback replacement; they are not exhaustive transition coverage.
The simulated replacement-unavailable failpoint is injected before the fake
replacement call and is not a test of an unavailable OS atomic primitive.

No `activate` or `enable-writes` operation exists. Outputs remain `INCOMPLETE`
and `synthetic_fixture`; they cannot establish authentic D02 evidence, D03
acceptance, release readiness, or operator authorization. The harness does not
call `RelSession` or `PhySession`, so current-owner semantic-open checks remain
a separate downstream verification item.

The harness requests file and directory flushes and fails closed if a flush
call reports an error. On the current Windows machine, `FlushFileBuffers`
returned success for a directory handle. That is only an observation here. No
claim is made about other Windows versions or filesystems, or about power-loss
durability. Tests cover selected abrupt-process restart windows, not power loss.

Errors include persisted phase, selected generation, manifest digest, and the
current fake pointer when those records can be read safely. If state cannot be
read, the error directs the operator to run `inspect-state` separately. If that
step also fails to read state, preserve the fake root unchanged for diagnosis.

Run the standard-library tests from the product checkout root:

```text
python -m unittest scripts.d03_synthetic.test_engine scripts.d03_synthetic.test_recovery scripts.d03_synthetic.test_integrity -v
```

Example use, with a fresh path directly under OS temp:

```text
python -m scripts.d03_synthetic stage --root C:\Users\<you>\AppData\Local\Temp\d03-synthetic-demo
python -m scripts.d03_synthetic validate --root C:\Users\<you>\AppData\Local\Temp\d03-synthetic-demo
python -m scripts.d03_synthetic rehearse --root C:\Users\<you>\AppData\Local\Temp\d03-synthetic-demo
python -m scripts.d03_synthetic rollback-prewrite --root C:\Users\<you>\AppData\Local\Temp\d03-synthetic-demo
```

The demonstration root is create-only; choose a fresh name. Fault tests use
only harness-created temp roots. No Cargo command is required.
