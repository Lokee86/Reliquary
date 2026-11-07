# F03 identity observation v1

## Purpose

This is a separate, header-only identity observation probe for the six disposable copies bound by the reviewed V01 receipt. It is not an expected-custody document, source-authenticity claim, historical replay, or admission result. Its only successful status is `IDENTITY_OBSERVATION_ONLY`; `authentic_admission`, `semantic_history_assessed`, `owner_identity_independently_authenticated`, and `scope_independently_authenticated` are all false.

## Overview

The expected six-copy path, SHA-256, and byte-length table is embedded in the probe and pinned to the V01 copy-proof test, custody receipt SHA-256, planning commit, and planning-record blob. A canonical table digest is also checked before any observation. The probe does not accept caller-supplied UUID, owner-kind, or scope expectations. It reports kind, UUID, and scope only as values parsed from the stored header. It never derives them from a filename. A missing identity header and a typed header with no scope value remain distinct explicit unknown states. A malformed or unsupported header fails closed and produces no report.

On Windows, each source copy is opened through a read-only handle whose share mode permits readers but denies new writers and deletion. Hashing and header parsing use clones of that same held file object. The historical reader's path-based API is unchanged; the crate-private file-handle entry point reads the header from the caller's already-held object. The probe reads no container frames or semantic payloads and does not invoke `PhysicalCursor`, a converter, a repair path, or a reconstruction helper. Final full-file hashes and exact source-tree membership must still match the V01 binding.

Run only against the already hash-verified, genuinely disposable copies. Never point it at authoritative originals. The output must be a fresh child of a separately verified protected directory, disjoint from the source tree. Before a real run, independently verify output directory ownership and ACLs, all ancestor reparse status, and that the fresh output path does not exist; the probe itself cannot establish or enforce the caller's ACL policy. The JSON report contains observed header UUIDs and physical file identities, so keep it inside that protected output location. The CLI emits only a sanitized status line and never prints paths, hashes, UUIDs, or payload text.

Invocation:

```text
cargo run --example f03_identity_observation --features history-oracle-fixtures --locked --offline -- --source-root <verified-disposable-copies> --output <fresh-protected-output>
```

## Related docs

- [f03 source witness v02](f03-source-witness-v02.md)

## Notes

An identity observation alone does not close the empty authentic carrier/scope inventory, the F03 semantic family/cut coverage, the historical replay requirements, or any later admission gate. Do not interpret an observed scope value as independently authenticated native scope.
