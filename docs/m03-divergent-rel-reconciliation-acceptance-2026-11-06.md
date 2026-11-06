# M03 — supported divergent REL reconciliation acceptance

Status: **corrected and accepted** for the supported native REL 2.1 boundary
after independent-audit remediation. Consolidated evidence:
`m03-final-correction-handoff-2026-11-06.md`.

Authority: `maintenance-history-parity-phase-2026-11-05.md` and capability
`M03` in the canonical rebuild tracker. This is same-owner REL 2.1
reconciliation, not a general merge facility or PHY reconciliation.

## Accepted behavior

- `compare_rel_histories` independently freezes both owners, checks REL kind,
  owner and scope, history generation, native-import completion fencing and
  the accepted semantic payload prefix, then reports `Identical`,
  `LeftExtendsRight`, `RightExtendsLeft` or `Diverged`.
- The Application workflow retains byte-exact verified-copy fast paths for
  identical and strict-extension histories. Only `Diverged` enters semantic
  replay.
- Divergence starts from a staged verified copy and replays the other branch
  through Archive, Memory, Perception, Graph, Freshness and Echo owners.
  Stable Archive, Memory, Entity, Relationship and Episode identities are
  checked. Source and grounding provenance remain attached to their accepted
  revisions.
- Exact mutation/event/use identities are idempotent. Repeated reconciliation
  is byte-stable and creates no duplicate Memory revision, Graph mutation or
  Freshness credit.
- Graph transaction clocks and owner Memory/Freshness clocks are rebased only
  by ordinary store admission. The report declares vector rebuild required
  when semantic replay changed an owner that carried derived vector state;
  vectors are never silently treated as current after a merge.
- The candidate is independently reopened with the union Graph closure,
  preserved Archive/Memory/Freshness histories and native-import settlement
  authority are checked, and both source cuts are reverified before the staged
  file is published.
- Completed right-only native-import epochs replay through destination-native
  Archive, Insomnia, grouped Memory, Freshness and settlement operations.
  Authenticated initial Dream settlements retain their source population, and
  every revision in the supported imported Insomnia terminal/retry journal is
  replayed through the destination work-state APIs. Other work transitions
  remain typed stops.
- Shared completed epochs compare frozen Freshness settlement authority and the
  full retained Insomnia work-history chain. Replayed settlements bind their
  authenticated source authority separately from the destination's actual
  merged Freshness snapshot, so repeat reconciliation is byte-stable without
  hiding a divergent source authority.

## Typed refusals and supported boundary

- Wrong kind, owner, REL scope or history generation; active writers; open
  import epochs; conflicting Archive source identities, Memory mutations,
  Entity/mention/Relationship revisions, Graph mutations or Freshness
  evidence; and an existing destination all fail without publishing output.
- Completed native-import history is supported when its accepted Archive,
  grouped Memory, Freshness, Dream and Insomnia evidence can be replayed through
  destination owners. Open epochs, conflicting settlement authority, divergent
  imported work journals and unsupported Dream semantics fail closed.
- Divergent live non-import Insomnia, Community, Ego, external-source,
  maintenance, historical-snapshot, pruned-payload or unknown carrier state
  fails closed. Shared prefix state remains exact in the staged base. PHY
  divergence remains outside M03.
- Failure after staging removes the private candidate. The sources and an
  existing destination remain byte-for-byte untouched.

## Acceptance coverage

The focused tests cover all four comparison relations; both prefix directions;
independent divergent Archive, Memory, Graph and Freshness changes; completed
empty/grouped imports; multiple epochs and intervening live Archive activity;
authenticated initial Dream replay; exact terminal/retry Insomnia history;
conflicting work-journal and Freshness authority; source-authority-preserving
byte-stable repeat; no-replace collision; wrong owner/scope/generation;
independent reopen; and source and destination immutability on failure.

## Verification

- `cargo test -p reliquary-owner-session semantic_reconciliation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **16 passed**.
- `python scripts/check_architecture.py`: **passed**, 23 production packages and 76 approved edges.
- `cargo fmt --all --check`: **passed**.
- `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check`: **passed**.
- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **passed**, including 154 owner-session unit tests, 6 owner-session integration tests and all workspace/documentation suites.

Existing compiler and Windows linker warnings remain outside M03. No package,
manifest or approved production dependency edge changed.

## Acceptance boundary

M03 is complete. M04 and the combined GM decision remain separately evidenced
in their own acceptance records. P01 public facade/CLI exposure and D01–D03
conversion/cutover remain separate work.
