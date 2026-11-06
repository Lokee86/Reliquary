# M01 Fresh-History Rewrite Acceptance — 2026-11-05

Status: **corrected and accepted** after the 2026-11-06 independent audit.
Memory-linked Relationship/Entity closure, complete retained revision replay and
no-replace publication are now covered by executable regressions. Final combined
review: `gm-independent-reacceptance-2026-11-06.md`.

Authority: `docs/architecture/maintenance-history-parity-phase-2026-11-05.md`
in the crate-first planning worktree. Implementation: `rebuild/clean-architecture`.

## Accepted surface

- Application owns a serializable, deny-unknown-fields rewrite policy and a
  read-only planner. The plan freezes owner ID, byte length, SHA-256 source
  proof, Memory cut, exact retained/dropped IDs, closure additions, revision
  and tombstone counts, warnings, output estimate, and its own digest.
- Memory policy supports include/exclude, lifecycle, archive, supersession and
  known-duplicate choices; tombstones; created/updated/source-time bounds;
  effective REL Freshness threshold; current/all/since-cut revisions; and zero
  or explicitly selected active embedding profiles. Contradictions, unknown
  IDs, unavailable profiles, and incomplete selected vector coverage stop.
- Closure expands stable parent/supersession references, active Graph Memory
  endpoints, and every Relationship whose local Memory evidence intersects the
  retained selection, including its Entity participants. Execution replays
  stable selected Memory IDs and chosen revisions, complete retained
  Entity/mention/Relationship histories and tombstones, verified
  Graph/Community/Ego state, selected Memory vectors, and REL Archive vectors
  for retained fragments. No inference, model call, or re-embedding occurs.
- REL provenance mode retains required source turns and ancestors, branches,
  Episodes, fragment definitions, attachments, and project references. REL
  may instead retain all Archive state or require it to be empty. PHY cannot
  acquire Archive or Freshness authority.
- Freshness supports reset, effective-current snapshot, or selected event/use
  replay. PHY-private Ego identities, personality, synthesis, and anchors are
  replayed only under the explicit verified-derived-state mode.
- NewOwner requires a distinct nonzero owner. PreserveOwner retains the owner
  but creates a new generation. Every output has authenticated rewrite lineage
  with source proof, plan proof, predecessor generation, and new generation.
  Prefix reconciliation refuses different history generations.
- Execution stages beside the destination, verifies source immutability again,
  syncs, independently reopens the candidate through REL/PHY admission, and
  publishes only after the selected Memory count matches. Existing destinations
  and stale plans do not produce or replace output.

## Explicit unsupported-state boundary

Dream, Insomnia, import-epoch, Echo, and external-source operational carriers
are not reconstructed or silently discarded. Their raw and grouped-publication
markers are detected before staging and returned as typed unsupported state.
Prior destructive-maintenance state also stops. This is the M01 fail-closed
boundary; operational semantic replay remains owned by later reconciliation
work where an exact evidence contract exists.

## Behavior evidence

| Contract | Rebuild proof | Negative proof |
| --- | --- | --- |
| Dry-run and closure | explicit child selection adds its parent; retained Memory evidence adds Relationship/Entity history without writes | contradictory, cross-owner and unknown IDs stop |
| NewOwner | stable selected IDs reopen under a distinct owner | stale source and destination collision leave no output |
| PreserveOwner | predecessor/new generation lineage survives reopen | cross-generation prefix reconciliation stops |
| REL provenance/Freshness | required turn, branch fragment, effective record reopen | PHY REL-only policy stops |
| Vectors | zero profiles and two-profile PHY coverage; REL Memory and Archive rows reopen | missing/retired/incomplete profile coverage stops |
| Filters | lifecycle, superseded duplicate, tombstone, and revision-cut fixtures | contradictory revision modes stop |
| Private derived state | PHY Ego reopens only under verified-derived mode | present state with omit mode stops |

The fixtures compare the same supported observable semantics as the reference
rewrite contracts: stable retained identities, selected current/history state,
provenance closure, profile coverage, owner identity mode, and generation
lineage. Raw 2.0 frames, offsets, and old API names are deliberately not copied.

## Verification

Run from `Reliquary-rebuild` with `CARGO_TARGET_DIR=target-codex-check` because
the default target link does not permit writes on this Windows filesystem.

- `python scripts/check_architecture.py` — passed; 23 production packages and
  76 expected edges.
- `cargo fmt --all --check` — passed.
- `cargo check --workspace --locked --offline -j 1` — passed.
- `cargo test -p reliquary-application rewrite_workflow::tests --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`
  — passed, 15/15 focused M01 cases.
- `cargo test --workspace --locked --offline -j 1 -- --test-threads=1` — passed,
  including doc tests. The final run allowed loopback networking solely for the
  switchboard's local mock HTTP servers; dependency resolution remained offline.

The full gate also exposed that this filesystem rejects hard links. Verified
copy and maintenance-candidate publication now use create-new verified copies.
Offline prune promotion retains the original through a hard link while holding
the owner lease; filesystems that reject hard links cannot perform promotion.

## Residual risks

- Operational Dream/import/Insomnia/Echo/external evidence is an intentional
  typed stop, not accepted rewrite coverage.
- M01 does not expose CLI/public gateway commands; P01 remains blocked on GM.
- M02–M04 and the combined GM decision remain separately evidenced in their
  own acceptance records; current GM authority is the post-correction review.
