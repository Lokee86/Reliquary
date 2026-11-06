# M02 Step 3 — terminal Entity and Relationship description pruning

Status: **implemented and owner-session verified; M02 phase remains open**.
Repository: `Reliquary-rebuild` (`rebuild/clean-architecture`).
Reference: `Reliquary-reference/src/facade/cva_description_prune.rs`,
`src/facade/relationship_compact_prune_tests.rs`, and
`src/perception/relationship_lifecycle_tests.rs`.

## 2.1 implementation

- Existing 2.1 Entity revisions retain their original stable IDs, canonical names,
  aliases, kind, mutation ID and timestamps. A new terminal tombstone and per-revision
  description marker permit replacing only a tombstoned Entity's summary.
- The rebuild previously had **no first-class Relationship-description records**.
  The graph's `Relation` edges have no summary and cannot stand in for the
  reference's Relationship domain. This step introduces native 2.1 Relationship
  description revisions in the existing Perception owner, with ordered
  owner-qualified Entity participants, Memory-evidence references, revision
  history, independent Relationship clock, mutation idempotency, and a terminal
  tombstone. It is not a Graph-edge deletion API. Relationship IDs use a new
  2.1 identity domain; no 2.0 runtime decoding or ID equivalence is claimed.
- REL and PHY now accept an explicit bounded selection of terminal Entity IDs
  or terminal Relationship IDs. Both operations require a frozen verified owner
  source, reject duplicate targets, active targets, stale plans and existing
  outputs, and refuse an already destructively pruned owner.
- The transformed copy replaces only selected per-revision summary text and
  appends typed, content-digested, manifest-bound markers. All original
  structural revisions and owner semantic clocks remain intact. Entity pruning
  also refuses current Memory mentions, active Graph edges and active
  Relationship participants pointing at the selected Entity.
- Retention descriptors bind **every** revision, retained fields, summary
  digest, terminal revision and terminal clock. Owner-session independently
  recomputes them after replay, rejects missing, duplicate, forged or unbound
  prune markers and compares retained semantic state. A transformed copy
  relocates vector-generation receipts if physical offsets shift. The original
  source remains unchanged; publishing is a no-replace hard link only after
  staged synchronization and independent REL/PHY owner admission.

## Verification

- `cargo test -p reliquary-owner-session --locked --offline -j 1 -- --test-threads=1`:
  **114 unit + 6 integration tests passed**. Coverage includes Entity and
  Relationship revisions, both owner kinds, verified pruning, source unchanged,
  nonterminal and shared-reference refusal, missing/forged/duplicate markers,
  stale source, destination collision, and post-tombstone mutation refusal.
- `python scripts/check_architecture.py`: **passed** (23 packages, 76 edges).
- `cargo check --workspace --locked --offline -j 1`: **passed**, with existing unused-code warnings.
- `cargo fmt --all --check`: **passed**.
- `cargo test --workspace --locked --offline -j 1 -- --test-threads=1`: **passed**, all crate and documentation tests on extended retry (9m 51s compilation + test). The initial six-minute attempt timed out during Lore compilation, before running tests. The succeeding retry compiled dependencies and exercised the entire workspace.

## Intentional boundaries

Only **descriptions** are removable. Relationship and Entity identities,
participant references, Memory evidence, lifecycle records, graph history and
other semantic payloads remain. There is no automatic cascading Graph deletion,
no independent Graph relation-description store, no 2.0 runtime compatibility,
and no arbitrary second destructive pass without a new lineage contract.
The new Relationship-description authoring API exists in the Perception owner;
P01 remains responsible for public facade/CLI exposure. M02 reclamation
coverage and fault injection remain separate and still block overall M02 acceptance.
