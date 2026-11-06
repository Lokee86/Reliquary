# M04 — historical read/proof parity acceptance

Status: **accepted** on `rebuild/clean-architecture` (2026-11-06).

Authority: `maintenance-history-parity-phase-2026-11-05.md` and capability
`M04` in the canonical rebuild tracker. This gate establishes bounded,
owner-scoped historical proof inside the typed owner-session boundary. Public
facade and CLI exposure remain P01 work.

## Accepted behavior

- REL callers select an explicit cut across Archive, Memory, Perception,
  Graph, Ego, Freshness and vector-generation clocks. PHY callers select the
  owner-local Memory, Perception, Graph and private Ego clocks; PHY correctly
  rejects Archive, Freshness and vector authority it does not own.
- Store-owned historical projections retain Archive branch revisions,
  Episodes, Fragments, import chronology and timestamps; Memory revisions and
  tombstones; Entity, mention and Relationship revisions; Graph relations and
  Community snapshots; private Ego revisions; Freshness events; and retained
  vector-generation receipts.
- Each proof binds the owner identity, REL/PHY file kind, history generation,
  selected cut and deterministic SHA-256 semantic digest. It reports bounded
  retained-record and unavailable-payload counts without exposing physical
  offsets as semantic authority.
- Terminal pruning remains observable at old cuts: structure, order,
  tombstones and prune evidence remain, while unavailable payload bytes are
  not fabricated. Retired vector generations remain part of historical proof
  after their packed backing is pruned.
- Projection checks cross-domain consistency. Graph endpoints must resolve to
  retained Memory or Entity identity, Ego cannot claim a future Memory cut,
  Freshness cannot reference absent Memory, and vector generation receipts
  cannot name a future Archive cut.
- Freshness cuts name real semantic events and cannot split the synthetic
  events carried by one atomic accepted-use receipt. Grouped Insomnia Memory
  publication interiors are likewise not queryable cuts.
- Imported chronology, restart/reopen behavior and deterministic repeated
  proof generation are covered. The proof is capped at 8,192 retained records
  and fails closed rather than returning a partial or ambiguous result.

## Typed refusals and supported boundary

- Wrong owner or file kind, future/unsupported clocks, non-authoritative
  Freshness cuts, grouped publication interiors, cross-domain inconsistencies
  and proof-size overflow return typed historical-proof errors.
- Missing or pruned payloads are represented only as unavailable counts over
  retained semantic structure. Missing provenance, retired generation backing
  and ambiguous authority are never reconstructed from inference.
- These proofs describe accepted semantic history. They do not claim byte
  restoration, generalized time travel over unknown carriers, conversion from
  retired formats or a public query surface.

## Maintenance transformation parity

- M01 preserve-owner rewrite retains the selected semantic digest while
  advancing the explicit history generation.
- M02 terminal pruning changes the digest and unavailable-payload count while
  retaining the same history generation and structural record count.
- M02 retired-profile vector pruning preserves both early and current
  generation proofs after backing reclamation.
- M03 reconciliation produces a stable proof for the merged semantic history;
  repeated byte-identical reconciliation produces the same proof.

## Acceptance coverage

Focused tests cover early, middle and terminal REL cuts; pre/post tombstone and
pruned Archive payloads; imported chronology; grouped Memory and Freshness
boundaries; Entity/Relationship/Graph consistency; retained and retired vector
generations; private PHY Ego revisions; wrong/cross owner; future clocks;
restart parity; and M01/M02/M03 transformed-history parity.

## Verification

- `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **passed**, including 125 owner-session unit tests, 6 owner-session integration tests and all workspace/documentation suites.
- `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check`: **passed**.
- `python scripts/check_architecture.py`: **passed**, 23 production packages and 76 approved edges.
- `cargo fmt --all --check`: **passed**.

Existing compiler and Windows incremental-cache warnings remain outside M04.
No package, manifest or approved production dependency edge changed.

## Acceptance boundary

M04 is complete. GM combined maintenance acceptance, P01 public facade/CLI
exposure and D01–D03 conversion/cutover remain separate gates.
