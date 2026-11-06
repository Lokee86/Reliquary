# M03 independent completed-import reconciliation re-verification — 2026-11-06

**Current verdict: corrected implementation verified; GM not yet accepted.** This is a new evidence record after the earlier GM independent audit reopened M03. The reference oracle is the pinned `Reliquary-reference` at `f62b1fb4`; rebuilding is native REL 2.1 only.

## Confirmed corrected behavior

- Completed right-only native-import epochs are replayed through **destination-native** owner operations and receive new destination watermarks; identical completed import authority is deduplicated, conflicting evidence is refused, and missing/open authority is a typed stop.
- Multiple epochs, intervening live Archive activity and grouped Insomnia Memory completions preserve stable identifiers, source links, accepted episode receipts and Freshness births. The imported grouped-Memory replay test independently merges the output with the same right source again and asserts zero additional Memory revisions or Freshness credits and byte-exact merged output.
- Candidate publication uses same-volume, atomic no-replace hard linking. A separate collision regression constructs an intervening destination and proves it remains byte-for-byte unchanged.
- **Additional independently found and repaired defect:** an accepted live (non-import) Insomnia work journal on the right could be allowed by the physical tail filter but discarded by the import-only replay mechanism. `unrelated_live_insomnia_work_must_not_disappear_during_import_merge` initially **failed**: reconciliation succeeded despite dropping that work. M03 now returns a typed `UnsupportedState("divergent live Insomnia journal")` if right live work has no exactly matching left work. The regression now passes and checks unchanged source files and absent destination.

## Focused executable evidence

- `cargo test -p reliquary-owner-session semantic_reconciliation --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1`: **11 passed**, including independent import epoch, grouped import, duplicate replay, conflict, collision, and live-Insomnia negative cases.
- Source-level review of replay: importer `semantic_reconciliation/replay/imports.rs` validates shared and new completed epochs; routes Archive, grouped Insomnia/Memory and Freshness effects through destination typed owners and post-write proof verification; live operational work is now explicitly fenced.
- The GM-wide locked/offline workspace, format and architecture checks are required again after integrating M03/M04 and are tracked in the final GM record.

## Boundary

M03 does not claim support for arbitrary divergent live Insomnia, Dream, Community, Ego, external source, pruned payload or incompatible owner histories. These are typed stops rather than dropped semantics. This record supersedes the **unsupported completed native-import** paragraph in the original `docs/m03-divergent-rel-reconciliation-acceptance-2026-11-06.md`; that initial sign-off is preserved as historical, not authoritative, evidence.
