# Offline history comparison

D02 independently opens the native files and compares their retained histories with the frozen F03 witness. It checks owner and clock correspondence, publication order, typed history, Archive backing cutoffs and vector relocation. It also exercises native CLI reads and the Root grant check. The F03 source reader lives in a separate checkout.

## Verify one original pair

Set `$Pair` to the reviewed fixture directory, `$Evidence` to its private source output root, and `$Custody` to its verified conversion inputs. Originals and copies are separate roots containing that fixture directory. The verifier binds both source hashes to the same pair, including when the 14-day and 28-day originals share owner IDs. [Source export and receipt layout](../d01-converter/README.md)

```powershell
$Shared = @('--pair', $Pair, '--source-tools', $SourceScripts, '--evidence', $Evidence, '--custody', $Custody, '--originals', $Originals, '--copies', $Copies, '--frozen', $Frozen)
python -B tools/d02-parity/verify_pair.py --freeze @Shared

cargo build --release --manifest-path tools/d02-parity/reader/Cargo.toml --locked --offline -j 1 --target-dir $Target
cargo test --test public_api --no-run --locked --offline -j 1
python -B tools/d02-parity/verify_pair.py @Shared --candidates $Candidates --cli $CurrentCli --reader $CurrentReader --writer $WriterUsed --grant-check $PublicApiTestExecutable
```

Build the CLI from `cli/Cargo.toml`. In each candidate directory, store the D01 outputs under `project.rel` and `user.phy` with `rel-compose.json`, `rel-writer.json`, `phy-compose.json`, and `phy-writer.json`. The successful comparison creates one `pair-verification.json` for that pair. Protect this directory and the frozen inputs because the candidates and transfers hold private history. The original `verify_first28d_pair.py` command defaults to `chatgpt-first28d`.

The focused checks are `python -B -m unittest discover -s tools/d02-parity/tests` and `cargo test --manifest-path tools/d02-parity/reader/Cargo.toml --locked --offline -j 1`. The comparator fixtures show that a changed intermediate value fails even when the head agrees.
