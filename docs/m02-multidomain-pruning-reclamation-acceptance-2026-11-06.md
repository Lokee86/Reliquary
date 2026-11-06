# M02 — multi-domain pruning, repair and reclamation acceptance

Status: **accepted** on `rebuild/clean-architecture` (2026-11-06).

Authority: `maintenance-history-parity-phase-2026-11-05.md` and capability
`M02` in the canonical rebuild tracker. Normal runtime remains native REL/PHY
2.1; no 2.0 reader or converter is introduced.

## Accepted behavior

| Domain | Accepted 2.1 operation and proof |
| --- | --- |
| Memory | Explicit terminal REL/PHY Memory-body pruning retains complete revision/tombstone history, refuses active/shared/vector-referenced bodies, persists typed retention evidence and independently reopens the transformed owner. |
| Compatibility profiles | Owner-level repair planning freezes the exact retained active set, validates current Memory and REL Archive coverage and appends auditable retirement evidence without inference or re-embedding. |
| Packed vectors | Retired-profile pruning removes only exclusively owned backing, retains mapping/generation history, relocates generation receipts and refuses active, shared, unowned or ambiguous backing. |
| Archive | REL-only terminal conversation pruning replaces selected turn text with typed markers after lifecycle, provenance, Episode, file and vector closure. PHY has no Archive authority. Attachment/file bodies remain when referenced. |
| Entity | REL/PHY terminal Entity description pruning covers every revision, preserves stable identity and clocks and refuses active mentions, Graph edges or Relationship participants. |
| Relationship | REL/PHY native 2.1 Relationship descriptions can be pruned only after terminal proof; participant, evidence and revision structure remain. This is not Graph-edge deletion. |
| Reclamation | A separate verified-copy operation removes unreachable standalone Memory bodies, unmapped packed matrices and unreferenced REL Archive file bodies. Retained vector-generation receipts are relocated and validated after reopen. |

Graph and Ego payload destruction are not claimed: the reference inventory did
not establish a supported independent destruction contract for them. Archive
attachments are not implicitly removed by conversation pruning. Unknown frame
families fail closed.

## Reclamation and failure safety

- REL coverage proves reclaimed orphan packed/file backing, preserved Memory
  history and active vector rows, generation-receipt relocation, an exact
  second-pass no-op copy and refusal of an unknown carrier.
- A referenced Archive file body produces an exact no-op copy and its bytes are
  still available after independent REL reopen.
- PHY coverage removes an unmapped packed matrix, leaves the source exact and
  independently reopens the candidate.
- A physically present vector generation without an activation receipt is a
  typed recovery stop. It is not guessed unreachable or silently discarded.
- Deterministic copy, sync and reopen fault injection proves no destination is
  published, the source remains byte-for-byte exact and the source still
  passes owner admission after each failure.
- Publication uses a staged, synchronized, independently reopened candidate
  and a no-replace same-volume hard link. Destination collisions, stale source
  proof and admission failures retain the source.

## Verification

The final verification used the repository-local `target-codex-check` cache
and serialized locked/offline execution. Windows hard-link access was enabled
for verified-copy publication tests.

- `cargo test -p reliquary-owner-session storage_reclamation --locked --offline -j 1 -- --test-threads=1`: **4 passed**.
- `cargo test -p reliquary-owner-session --locked --offline -j 1 -- --test-threads=1`: **118 unit + 6 integration passed** after the final reclamation additions.
- `cargo test -p reliquary-application storage_reclamation --locked --offline -j 1 -- --test-threads=1`: **2 passed**.
- `python scripts/check_architecture.py`: **passed**, 23 production packages and 76 approved edges.
- `cargo fmt --all --check`: **passed**.
- `cargo check --workspace --locked --offline -j 1`: **passed**.
- `cargo test --workspace --locked --offline -j 1 -- --test-threads=1`: **passed**, including all crate, integration and documentation tests. One initial run saw the unrelated local-Git fixture fail to create its temporary commit; that fixture passed alone immediately afterward and the complete rerun passed without a code change.

Existing unused-code warnings and the existing Windows `LNK4098` warning remain
outside M02. No new package or production dependency edge was added.

## Acceptance boundary

M02 is complete. M03 divergent REL semantic reconciliation, M04 broader
historical-query parity, GM combined parity evidence, P01 public facade/CLI
exposure and D01–D03 conversion/cutover remain separate gates.
