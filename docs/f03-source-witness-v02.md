# F03-V02 source witness validation tool

This feature-gated example is pinned to historical-reader base `c6010fd30dd52acb81c667f7aed99b831307e49f`. It is isolated validation tooling and is not part of the production dependency path.

Run it only after the six disposable copies have been reverified against the expected custody document, in the isolated historical validation tree, with a protected output directory outside the source-copy root:

```text
cargo run --example f03_source_witness --features history-oracle-fixtures --locked --offline -- --source-root <six-copy-root> --expected-custody <physical-custody-v1.json> --output <private-output>
```

The executable accepts `--resume` to re-read and compare existing deterministic frame batches before continuing, and `--verify-only` to verify custody and scan framing without writing evidence. It refuses source-root/output overlap, symlinks inside the source root, unbound owner files, changed file hashes, and conflicting resume artifacts. The output directory must not already exist for a fresh run.

The exporter hashes each source before and after scanning, records modification-time/length stability, matches owner kind and UUID from the container header, and traverses frames with the historical read-only physical cursor. It writes at most 128 frame observations per batch. Batches contain source/frame coordinates, payload SHA-256, and a SHA-256 fingerprint of at most the first 16 frame bytes. Raw frame bytes and semantic values are never written.

The recursive source-root inventory is custody-bound, not an authenticity claim. It records a path-redacted digest of file and directory membership/identity, re-enumerates before finalization, and rejects any source-root file with more than one physical hard link so an input cannot alias an authoritative original outside the disposable-copy root. Coverage and manifest bytes are checked against their exact serialized values and kept under read guards through finalization.

This is deliberately only a physical carrier census. It does not map those observations to authentic record schemas, semantic families, history cuts, unavailable states, or sidecars outside the source-copy root. Header scope is recorded separately but not authenticated against the custody scope. Every semantic group remains unclassified with unknown row counts, the cut catalog is not started, authentic admission is false, and the command exits nonzero with `INCOMPLETE`. That status must not be upgraded by interpreting an unknown marker as absent or by treating this output as the PG-C01 synthetic PASS fixture.

Outputs are `source-coverage.json`, `source-witness-manifest.json`, and immutable `source-####-batch-######.json` files. Resume replays from the beginning to byte-compare existing batches; it retains them rather than overwriting. A malformed or conflicting partial artifact fails closed. This mode is bounded in batch aggregation but may still allocate one physical frame at a time through the historical cursor.

No Cargo build or test is implied by these instructions; run Cargo only in an explicitly released serialized build window.
