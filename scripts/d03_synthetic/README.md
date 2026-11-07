# D03 synthetic staging harness

Status: provisional rehearsal foundation only. This harness is not D03 acceptance, a product deployment tool, or a selector consumed by any Reliquary opener. The manifest schema is provisional until PG-C01 is reviewed and integrated.

The `stage` command generates its own six tiny JSON fixture owners, a fake 2.1 runtime marker, and a clearly synthetic D02 `PASS` marker. It accepts no source-file paths or arbitrary owner bytes. It only creates a new `d03-synthetic-*` directory directly under the operating system temporary directory. `validate`, `rehearse`, `inspect-state`, and `rollback-prewrite` refuse roots without the harness marker.

The simulated selection record is read only by this harness. It changes no host, CLI, library or Gateway path binding. No `activate` or `enable-writes` operation exists. Outputs are `INCOMPLETE` and `synthetic_fixture`; they cannot establish authentic D02 evidence, D03 acceptance, release readiness, or operator authorization. The harness does not call `RelSession` or `PhySession`, so current-owner semantic-open checks remain a separate downstream verification item.

Run its standard-library tests from the product checkout root:

```text
python -m unittest scripts.d03_synthetic.test_engine -v
```

Example use, with a new path directly below the OS temp directory:

```text
python -m scripts.d03_synthetic stage --root C:\\Users\\<you>\\AppData\\Local\\Temp\\d03-synthetic-demo
python -m scripts.d03_synthetic validate --root C:\\Users\\<you>\\AppData\\Local\\Temp\\d03-synthetic-demo
python -m scripts.d03_synthetic rehearse --root C:\\Users\\<you>\\AppData\\Local\\Temp\\d03-synthetic-demo
python -m scripts.d03_synthetic rollback-prewrite --root C:\\Users\\<you>\\AppData\\Local\\Temp\\d03-synthetic-demo
```

The demonstration root is create-only; choose a fresh name. Destructive/fault cases in tests use only harness-created temporary roots. No Cargo command is required.