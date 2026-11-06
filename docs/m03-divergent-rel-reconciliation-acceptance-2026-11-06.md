# M03 — supported divergent REL reconciliation acceptance

Status: **accepted** on `rebuild/clean-architecture` (2026-11-06).

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

## Typed refusals and supported boundary

- Wrong kind, owner, REL scope or history generation; active writers; open
  import epochs; conflicting Archive source identities, Memory mutations,
  Entity/mention/Relationship revisions, Graph mutations or Freshness
  evidence; and an existing destination all fail without publishing output.
- A completed native-import history is supported when it is shared accepted
  prefix authority. Divergent native-import epochs or settlement authority are
  explicitly unsupported because their physical prefix digests cannot be
  rebased without fabricating evidence.
- Divergent Community, Ego, Dream/Insomnia, external-source, maintenance,
  historical-snapshot, pruned-payload or unknown carrier state fails closed.
  Shared prefix state remains exact in the staged base. PHY divergence remains
  outside M03.
- Failure after staging removes the private candidate. The sources and an
  existing destination remain byte-for-byte untouched.

## Acceptance coverage

The focused tests cover all four comparison relations; both prefix directions
through the retained exact-copy workflow; independent divergent Archive,
Memory, Graph and Freshness changes; an overlapping exact Memory transaction;
repeat reconciliation without duplicate credit; conflicting source identity;
wrong owner/scope; preserve-owner generation divergence; independent reopen;
and source/destination immutability on failure.

## Verification

- `cargo test -p reliquary-owner-session semantic_reconciliation --target-dir target-codex-check`: **4 passed**.
- `cargo test -p reliquary-application verified_copy_workflow --target-dir target-codex-check`: **3 passed**; the dedicated `rel_reconciliation_workflow` exact-copy router test also passed.
- `python scripts/check_architecture.py`: **passed**, 23 production packages and 76 approved edges.
- `cargo fmt --all --check`: **passed**.
- `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check`: **passed**.
- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **passed**, including 122 owner-session unit tests, 6 owner-session integration tests and all workspace/documentation suites.

Existing compiler and Windows linker warnings remain outside M03. No package,
manifest or approved production dependency edge changed.

## Acceptance boundary

M03 is complete. M04 historical read/proof parity, GM combined maintenance
acceptance, P01 public facade/CLI exposure and D01–D03 conversion/cutover remain
separate gates.
