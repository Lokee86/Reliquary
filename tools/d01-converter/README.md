# Offline 2.1 history writer

The D01 composer and native writer run in separate processes. The composer reads a hash-pinned original copy and the F03 family transfers. The writer accepts an ordered, hash-pinned transfer and writes a new native 2.1 owner. It syncs, reopens through ordinary owner admission, and publishes through a same-directory no-replace link.

## Reviewed original pairs

Use the isolated F03 source checkout at `de7cec88` on `migration/offline-named-source-pairs`. Run its `f03_source_witness` example with `--fixture $Pair`, the protected copy root, and the original custody receipt. The reviewed fixture directories are `chatgpt-first14d`, `chatgpt-first28d`, and `rhelm-david-r-ellis-pre-relationship`. Use one witness and one output directory for each pair. [Original execution plan](../../docs/planning/owner-history-migration-plan-2026-11-18.md)

Use the F03 Memory, Graph, Entity Resolution, Community and Freshness exporters. The source checkout's `f03_export_first28d_converter_transfers.py` runs the Archive, vector, work and native Freshness adapters in this directory. Prepare a separate output directory and freeze [the D02 expectations](../d02-parity/README.md) before composing either file.

```powershell
cargo build --manifest-path tools/d01-converter/target-writer/Cargo.toml --locked --offline -j 1 --target-dir $Target
python -B tools/d01-converter/compose_authentic.py --source-tools $SourceScripts --source-coverage $Coverage --source-copies $Copies --transfer-root $Transfers --fixture "$Pair/project.prj.rel" --output $RelTransfer
& $Writer --authentic-frames $RelTransfer $RelTransferSha $NewRel
```

Use the composer's `transfer_sha256` in the writer invocation. Repeat with `"$Pair/user.phy"` and a distinct transfer and new owner path. Capture the composer and writer receipts for each file. The writer receipt establishes current-format reopen and binds the candidate hash. The independent reader establishes parity.

The focused fixtures exercise Archive allocation and reservation binding, retained PHY revisions, output collision and unresolved vector backing. Run `cargo test --manifest-path tools/d01-converter/target-writer/Cargo.toml --locked --offline -j 1`. Use a local scratch directory for files and receipts containing original data.
