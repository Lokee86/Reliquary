# F03 pre-conversion physical-only census

## Purpose

This is a distinct, feature-gated one-shot tool. It accepts only the six V01 path/hash/byte-length copy pins and their custody provenance. It does not read or generate a PG-C01 full-custody document, expected owner UUID/kind/scope, target hash, or source-to-target mapping. The existing strict f03_source_witness command remains unchanged and continues to require full PG-C01 custody.

## Overview

The command verifies the exact V01 copy layout, each copy hash/length, one-link physical identity and source-tree inventory. It parses headers from a clone of the same held file handle and scans physical framing with the bounded historical cursor. Owner kind, owner UUID and native scope are output only as *_observed_from_header; a missing header/scope remains not present/unknown. These observations never become independent expectations or authenticated identity. Copy IDs are opaque, assigned by the fixed V01 table's byte-ordered path order; source paths are not included in outputs.

Frame batches contain only offsets, lengths, clocks, payload SHA-256 values and cryptographic fingerprints of at most the first 16 payload bytes. Raw frame bytes and payload prefixes are never emitted. Batch payload hashes form a predecessor chain. The one-shot mode refuses existing output, source/output overlap, extra source-tree entries, aliases, reparse points, changed files, invalid framing and bounded output overflow. It re-enumerates source membership after final source hashes and keeps source, output, batch and report guards through finalization. Output access control itself is not authenticated by the report: use a fresh disjoint output leaf under an independently verified restricted owner/SYSTEM/Administrators-only ACL.

Every report is explicitly INCOMPLETE; authentic admission, identity/scope authentication, semantic history, family coverage and cut completeness are false or unknown. External sidecars are not discovered. No family is inferred from paths, headers or frame bytes, and no zero-row or unavailable-state claims are made. A successful physical scan still returns exit code 2 because semantic source evidence is not established.

Run only after a separate preflight confirms the six disposable copies against V01 custody and verifies the protected output parent:

cargo run --example f03_preconversion_physical_census --features history-oracle-fixtures --locked --offline -- --source-root <six-copy-root> --output <fresh-protected-output>

Outputs are preconversion-physical-coverage.json, preconversion-physical-manifest.json, and copy-####-batch-######.json files. There is no resume mode. An interrupted run may leave partial artifacts without a manifest; discard that output leaf only after preserving its diagnostic status. No output is an authentic PG-C01 source witness or admission receipt.

## Related docs

- [f03 source witness v02](f03-source-witness-v02.md)
- [f03 identity observation v1](f03-identity-observation-v1.md)

## Notes

The coverage and manifest are diagnostic outputs, not a successful-run receipt. Final integrity checks happen after the reports are written, so a failed finalization can leave them present. Treat report presence alone as insufficient; an independently audited, protected outer receipt must capture the exact invocation outcome and bind the reviewed producer/helper/example sources, Cargo.lock, built executable, active toolchain, command, complete logs, and full output artifact set. The tool itself does not authenticate that receipt or the output ACL.
