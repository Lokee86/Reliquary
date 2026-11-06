# M02 Step 2 — terminal REL Archive pruning

Status: **implemented, focused verified; M02 phase not accepted**.
Implementation: `Reliquary-rebuild`, `rebuild/clean-architecture`.
Reference contracts: `Reliquary-reference/src/facade/{cva_history_prune,cva_archive_history_prune}.rs`, current 2.1 semantics rather than 2.0 frame compatibility.

## Supported operation

- Only an explicitly selected, terminally tombstoned REL conversation is eligible. PHY Archive authority is refused.
- Requires a frozen, authenticated `VerifiedOwnerCopyPlan`, unique selected conversations, no previous destructive-maintenance manifest and a new destination. Original source is never modified.
- Removes selected turn text by substituting typed `RELTPR01` markers. Retains the accepted node/parent/role/principal/time/content identity, branch and Episode structure, fragment definitions, source attachment/file bytes and project references. **Attachment bodies are not deleted** as part of this operation; they remain referenced data.
- Preserves shared content safely by failing if an unselected turn or any retained file/attachment has the same content ID.
- Fails closed when any Memory history revision depends on the selected conversation, source node or Episode, including indirect `source_ref`; when import receipts still map it; when Archive vectors reference its fragments; or when any selected Episode has unfinished Insomnia work. Only completed/terminal Insomnia Episodes may be pruned.
- Terminal lifecycle fences subsequent turn, branch, Episode, import and fragment publications on write and replay.
- Manifest includes selected node IDs, content IDs, authenticated tombstone and structural digests, source owner and history cut. On reopen, the marker/manifest is checked for exhaustive matching, unique coverage and no unbound markers; incorrect digests reject replay.
- Physical rewriting relocates retained `RELVGRC1` generation-receipt offsets when shorter turn markers shift later `RELVGEN1` records.
- After a sync and independent owner reopen, postflight compares source order, Archive and global clocks, source revisions, activity, branches, Episodes, fragments, import receipts, Memory/Archive vector identities, attachment file bytes, retained text and explicit unavailable selected content. Only after source re-verification is the candidate published without replacement.

## Verified checks (2026-11-06)

- `cargo test -p reliquary-owner-session terminal_archive_prune --locked --offline -j 1 -- --test-threads=1`: **8 focused tests passed**, including multi-turn Episode+attachment preservation, shared text/file reference refusal, Memory direct/indirect/Episode provenance refusal, unfinished Episode refusal, tampered marker rejection, active vector-source refusal and generation-receipt relocation.
- `cargo test -p reliquary-owner-session --locked --offline -j 1 -- --test-threads=1`: **107 unit + 6 integration passed** (run before the final test-file split, same behaviors).
- `cargo test -p reliquary-archive --locked --offline -j 1 -- --test-threads=1`: **24 passed**.
- `python scripts/check_architecture.py`: **passed**, 23 production packages and 76 required edges.
- `cargo fmt --all --check`: **passed** before the last fixture split (formatting was subsequently applied).

## Boundaries

Step 2 is accepted only for proven **Archive turn text** pruning in REL. Do not claim independent deletion of attachment/file bodies, automatic cleanup of related Memories, incomplete Episode work or joint multi-domain historical transforms. M02 Step 3 (Entity/Relationship), Step 4 (reclamation), failure injection, full workspace gates and GM parity evidence remain outstanding. A prior workspace-wide check timed out while compiling the Lore dependency stack; it is not counted as passing.
