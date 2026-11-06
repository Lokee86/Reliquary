# M03 final correction handoff — 2026-11-06

**Verdict: corrected and verified for the supported native REL 2.1 boundary.**
This record consolidates the completed-import, Dream, Freshness-authority and
Insomnia-journal corrections that followed the independent GM audit. The
contract remains the M03 section of
`maintenance-history-parity-phase-2026-11-05.md`; the reference oracle remains
`Reliquary-reference` at `f62b1fb4`.

## Corrected behavior

- Completed right-only native-import epochs replay through destination-native
  Archive, Insomnia, grouped Memory, Freshness and import-settlement APIs.
  Destination clocks and watermarks are regenerated while stable semantic
  identities, receipts and source evidence are preserved.
- Completed initial Dream settlements replay from an authenticated physical
  Begin and retain the source pass's exact historical preexisting population.
  A destination-only Memory birth is not silently inserted into that Dream
  population.
- Every revision in the supported imported terminal/retry Insomnia journal is
  decoded in owner order and replayed through the destination work-state APIs
  before grouped completion. That history is preserved exactly. A shared
  completed epoch with a different work-journal chain is a typed semantic
  conflict with no output; unimplemented renewal/checkpoint/held transition
  histories fail closed rather than being summarized or copied opaquely.
- Shared completed epochs compare their frozen import Freshness authority:
  `freshness_state_digest` plus `freshness_memory_count` from the durable
  settlement. The comparison does not reconstruct historical authority from a
  branch's later journal. Reconciliation persists that authenticated source
  authority separately from the destination's actual merged settlement. A
  repeat with the same source is therefore byte-stable with zero replayed
  Memory revisions, even when the destination has additional recipients.
- Divergent live, non-import Insomnia remains a typed unsupported state. Source
  owners and an intervening destination remain byte-for-byte unchanged on
  every covered conflict and publication collision.

## Regression evidence

Focused M03 execution:

```text
cargo test -p reliquary-owner-session semantic_reconciliation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
16 passed; 0 failed
```

The focused cases include completed empty and grouped imports, multiple epochs,
intervening live Archive activity, authenticated initial Dream replay, retained
Dream populations, terminal/retry Insomnia history, conflicting Insomnia
journals, source-authenticated Freshness authority with byte-stable repeat,
live-Insomnia refusal, and no-replace publication collision.

Unrestricted gate verification:

```text
python scripts/check_architecture.py
Production DAG: 23 packages, 76 edges; expected 76.

cargo fmt --all --check
passed

cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check
passed

cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
passed, including all crate, integration and documentation suites
```

Existing compiler, incremental-cache and Windows linker warnings remain outside
M03. No manifest or approved production dependency edge changed.

## Supported boundary

M03 is same-owner REL reconciliation, not arbitrary history merging. Open import
epochs, cross-owner or PHY reconciliation, incompatible generations, conflicting
Freshness or work-journal authority, arbitrary Dream relations, positive-root
Dream propagation, divergent Community/Ego/external-source/maintenance/pruned
state, and unknown carriers remain typed stops. The corrected implementation
does not claim old-format runtime support or corpus conversion.
