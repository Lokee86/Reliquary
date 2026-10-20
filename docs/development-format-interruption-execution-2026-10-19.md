# Development-format interruption execution evidence (2026-10-19)

Parent: [Documentation index](INDEX.md).
Plan: [Format-policy amendment](minimal-lossless-history-storage-format-policy-amendment-2026-10-19.md).

## Purpose

Record I1–I12 without deleting readers before the remaining canonical owner converters exist. This is policy, inventory and acceptance design; it does not claim runtime retirement or fixture promotion.

## Overview

The frozen runtime is `39eea4a2fdce2549e161972422958f369d05e9f3`. Resumption began at clean source HEAD `9cf643e0ceb92c764e73e9d572a64eddbf3c3b7a`. The five intervening paths are documentation only; `src`, `cli`, Cargo manifests and locks have no difference from the freeze. The [inventory](../benchmarks/development-format-interruption-inventory-2026-10-19.json) records exact hashes, statuses, paths and header versions.

The fixture repository is at `fca1c1d0a1bbb19f56f29c51b080e33185e01899` with twelve preexisting modified paths: the six authoritative containers and their six metadata/checksum files. This interruption does not modify or commit them. All six current container hashes match the previously accepted authoritative baseline. Its README identifies these as the only deliberately retained canonical REL/PHY pairs. A recursive fixture-repository container inventory finds no additional REL/PHY/CVA files in calibration or experiments. Generated workspace test scratch is excluded from promotion. The old development-doc reference `target/insomnia-tuning-v1-check.cva` is absent, so it is not a present retained corpus dependency. Git/Syncthing historical states are archives, not parallel current corpora.

## Caller classification and deletion ownership

Each temporary row below is an I10 removal tag. Removal requires R1–R12 corpus verification/promotion first; existing runtime reachability is an observation, not an approved compatibility exception.

| Tag / owner | Exact source seam | Present classification | Retirement gate and retained tests |
| --- | --- | --- | --- |
| T1 / Container | `src/container/container_scan.rs`: `Container::open`, `open_scanned_version`; `src/container/container.rs`: `read_header_for_version_selection`, `LEGACY_VERSION` | Generic runtime open currently accepts V1/V2; V1 handles are read-only. Current creation emits V2. | R13: remove V1 selection and exceptions; keep V2 positive and V1/unknown typed-rejection tests in `container_format_boundary_tests`. |
| T2 / Facade | `src/facade/cva_lifecycle.rs`, `phylactery_lifecycle.rs`: generic `open`, public `open_v1_read_only`, V1 compaction/Entity-resolution projection branches | Runtime compatibility, reached by CLI info/verify and host mounts; V1 write admission is a separate barrier. | R13/R17: V2-only facade open, no V1 repair/mutation; replace V1-success expectations in `storage_format_boundary_tests`. |
| T3 / Freshness | `src/freshness_storage.rs`: settlement ingest; `src/freshness/freshness_import_settlement.rs`: `decode_legacy_frame`, V4/V5 decode, ingest; `freshness_import_compatibility.rs`: successor/predecessor/event absorption | Normal reopen decodes CVAFRS03/04/05 and uses compatibility proof/event hooks. CVAFRS06 is canonical. | R14: delete old decoders/hooks only after equivalent canonical event proof exists; retain CVAFRS06 reconstruction, exact same-cut events and malformed-current tests. |
| T4 / Historical import | `src/facade/cva_historical_import.rs`: `prepare_import_settlement_normalizations`, `normalize_import_settlements`; `historical_import_input.rs` | Explicit disposable-copy conversion, but public normalization and repair wiring remain reachable. | R14/R16: remove one-time conversion API and repair normalization; retain canonical settlement retry/effect tests. |
| T5 / Import epoch | `src/imports/legacy_epoch.rs`, `legacy_epoch_transport.rs`; `epoch_store.rs`: `ingest_at`, `validate_legacy_frames`; `cva_lifecycle.rs` open validation | Runtime CVAILEG1/2 readers authenticate nested original evidence, owner, marker position, prefix digest and unique CVAFRS03 settlement. | R15: replace legacy carrier dependency with authenticated canonical semantic epoch evidence, then remove modules/branches and legacy projection. Keep canonical CVAIEPS1/CVAIEPC1 epoch behavior. |
| T6 / Physical maintenance | `src/facade/storage_reclamation.rs`, `cva_repack.rs`, `cva_principal_repack.rs`, `cva_history_prune.rs`, `phylactery_history_prune.rs`; `historical_import_transform.rs` | Temporary `rebind_copy`/`original_payload` transport is currently reached by ordinary maintenance of owners carrying old proofs. | R15: remove rebinding/unwrapping after old frames are absent; retain current-only prune/repack/reclaim proof-equivalence tests. |
| T7 / Storage upgrade | `src/storage_upgrade/{mod,model,shell,memory,entity,relationship}.rs`: plan/stage functions including `stage_relationship_storage_upgrade`; feature-gated Memory/Entity/Relationship authoritative tests | Private offline/disposable conversion seam; no public promotion API or CLI `upgrade` command. Memory, Entity and Relationship converters compose explicitly; this does not make later owner payloads canonical or promote a corpus. | R16: remove shell, translators and obsolete successful-migration tests after fixture commit; preserve canonical-only negative/positive tests. |
| T8 / Identity bootstrap | `src/facade/migration.rs`: public `migrate_file`; `cli/src/{args,main,migration_cmd}.rs`; `docs/manual/migration-and-reconciliation.md` | Separate offline identityless-input conversion. It allocates a new UUID, rejects already identified owners and is not lossless identity-preserving V1 upgrade. No named compatibility exception is approved. | R16: retire API/CLI/manual/tests unless a separately justified consumer/sunset is recorded. |

Additional dependency callers: `src/imports/completion.rs` validates settlement digest before completion; `reconciliation.rs` admits CVAFRS05/06 intervals; `readiness.rs` consults settled state. Preserve their canonical semantic behavior while removing old-format branches. `cva_history_prune.rs` validates import proof and `storage_reclamation.rs` refuses unresolved physical settlements. `migrate-conversation-titles` is content backfill, not a format upgrader. Fresh-history rewrite is intentional history discard and cannot substitute for lossless migration; reclaim is not an upgrader and rejects V1 writes.

V2 creators can currently reproduce older 16/24-byte identity-header shapes through `create_with_legacy_identity`/`create_empty_like`. This is distinct from V1 version dispatch. Final parsed-corpus inventory must establish the accepted canonical header shape; do not silently conflate header identity bootstrap with container-version conversion.

## Remaining converter dependency matrix (I7)

| Owner / gap | Original steps | Required boundary before corpus promotion |
| --- | --- | --- |
| Memory | 87, 136–137 | Current Memory-only staging preserves history but still copies other legacy owners and retains old import proof. It is not final canonical corpus conversion. |
| Entity | 91–98, 138 | Creation/delta, both terminal forms and pruned wrappers; preserve each admitted optional global-ID revision. Reconciliation and reclamation adapters need explicit steps at owner entry. |
| Relationship / Observation | 99–107, 139 | Resolve one target, translate supported prior representations, preserve participant/evidence/terminal history. |
| Ego | 108–117, 140 | Identity, Anchor, Personality and synthesis owner-specific conversions. |
| Graph / Archive publication | 118–123, 141–143, 206 | Measurement-gated encoding/envelope choices require explicit per-owner conversion or justified canonical defer. No umbrella converter. |
| Freshness / imports | 124–132, 207 | Canonical event proof must replace CVAFRS03/04/05 physical dependencies, not merely append a CVAFRS06 successor. CVAILEG1 and CVAILEG2 each require explicit translation to authenticated canonical epoch evidence. Step 207 must insert owner-specific contract/codec/translator/validation items for any missing family before continuing; it remains an audit, not permission to hide implementation. |
| Dream first pass / pairs | 144–160 | Exact historical population cuts and exceptional explicit sets; pair-set equality. |
| Interaction / external derived index | 161–177 | Preserve checkpoint history; derived-index rebuild cannot discard external publication history. |
| Entity Resolution / Community | 178–193 | Decision/proof history; measurement-backed Community choice retains exact proof authority. |
| Vectors | 194–204 | Exact byte-distinct vectors, profiles and generations; profile compatibility is a live semantic object, not an obsolete-file compatibility exception. |
| Echo / project history / unknown owners | 205–212 | Explicit Echo and correlation conversion, complete family inventory, typed unknown-authority rejection, full owner validation. |

## Current-only runtime acceptance design (I9)

These are future test names/contracts, not newly implemented tests. No reader is removed during I1–I12.

| Proposed test | Boundary and acceptance |
| --- | --- |
| `canonical_v2_rel_phy_reopen_every_retained_cut` | Normal Container/Cva/Phylactery open accepts canonical V2 corpus with exact IDs, local/global clocks, timestamps, vectors, profiles, import proof and historical cuts. |
| `normal_open_rejects_v1_before_payload_dispatch` | Container and both facades return typed `UnsupportedVersion` before payload visitors, recovery, repair or mutation; source bytes/hash/mtime stay unchanged. CLI info/verify and host mounts expose the same boundary. |
| `normal_open_rejects_unknown_container_version` | Unknown major/minor versions fail closed before dispatch, with no identity fabrication or output creation. |
| `canonical_v2_rejects_obsolete_settlement_and_epoch_frames` | V2-header inputs containing CVAFRS03/04/05 or CVAILEG1/2 fail with a typed unsupported-payload error; checksum-valid obsolete frames are not silently ignored or treated as current. |
| `canonical_v2_malformed_payload_never_falls_back` | Corrupt current framing, unknown masks and contradictory proofs reject; no legacy fallback. |
| `temporary_converter_symbols_are_absent` | Post-R16 source/CLI/API search finds no removed converter, V1-open API, normalizer, rebinder or legacy decoder entry point. |

## Original-step mapping and final gates (I11)

Accepted original Steps 1–86 are immutable. Original numbering remains 1–266. Every later step is classified below; a range classifies each contained number without collapsing its existing action or gate.

| Original steps | Classification / controlling condition |
| --- | --- |
| 87 | Measurement of provisional Memory conversion; completed evidence is reusable, but I1–I12 precede acceptance. |
| 88–204 | Canonical owner contract/codec/replay/writer and verification work, except the explicit temporary converter steps listed next. Existing measurement-backed defer decisions remain required. |
| 96, 105, 115, 122, 130, 137, 150–152, 158, 168, 176, 184, 192, 202 | Temporary offline conversion/proof tasks; their old-format readers are removed at R13–R16 after corpus promotion. Conditional steps retain their original conditions. Step 152's exceptional semantic representation is not automatically obsolete; retain it only if canonical history genuinely needs it. |
| 205–207 | Inventory and converter-coverage audits, with explicit owner-family insertion on discovered gaps. |
| 208–210 | Temporary Echo/correlation conversion and unknown-authority rejection. |
| 211–212 | Upgraded REL/PHY owner-invariant validation. |
| 213–219 | Disposable canonical-corpus migration and accounting; no promotion before R1–R12. A V2 header alone does not satisfy canonical payload inventory. |
| 220–237 | Historical, maintenance and reconciliation equality; legacy inputs are temporary reference oracles, not permanent runtime formats. |
| 238–248 | Performance and disposable-checkpoint decisions; comparison inputs remain offline references. |
| 249–259 | Architecture/docs/ownership audits; the final ordinary runtime is V2-only, not a permanent open/migration compatibility boundary. |
| 260 | No leftover temporary converter/alias/fallback is accepted only after R13–R20, not before a capable converter exists. |
| 261–265 | Final commit, size, compression/defer and documentation evidence, including separate fixture/source commits from R12/R22. |
| 266 | Final acceptance additionally requires every R1–R22 gate. No partial corpus, old reader or unapproved compatibility exception may be hidden. |

Original 143/206/207 audits insert explicit missing owner steps before their next integration gate. R1–R12 migrate/verify/promote the whole inventoried corpus, then R13–R17 remove the tagged seams and add negatives, then R18–R22 rerun complete verification and record cleanup commits. None of these future gates is accepted by this policy interruption.

## Verification and resumed checkpoint (I12)

I1 inventory captures clean source start, exact freeze/resume hashes and preexisting fixture dirt. I2 hashes all six retained inputs without modification. I3–I6/I10 are the caller/removal maps above; I7 is the dependency matrix; I8 updates canonical architecture/development/storage/ADR policy; I9 defines runtime acceptance; I11 maps all remaining original steps. I12 requires both documentation-policy modes, link/index checks, unchanged runtime diff and unchanged fixture hashes after this documentation-only batch. The resumption commit is recorded in the amendment after these checks pass.

I12 passed: both documentation-policy modes returned success (`.test-tmp/format-interruption-docs-structural.log`, `.test-tmp/format-interruption-docs-impact.log`); index/related links resolve. Direct post-edit checks confirm all six source hashes unchanged, no runtime diff from the freeze, original rows exactly 1–266 and accepted records exactly 1–86. The amendment records I1–I12 as complete. Source resumption is based on `9cf643e`; the policy checkpoint is the isolated commit containing this record and its inventory.

## Related docs

- [Interruption specification](development-format-hard-cut-interruption-spec-2026-10-19.md)
- [Original execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md)
- [Storage format](storage-format.md)
- [Development workflow](development.md)

## Notes

After I12, discovery inserted nine steps in the live plan: one reproduced maintenance-proof rejection gate and eight Entity consumer adapters. Original 91–94 map to 92–95, original writer 95 maps to 102, and original 96–266 map to 105–275. New 91 rejects unsupported maintenance proof; new 96–101 own Entity reconciliation, import evidence/admission, reclamation and shared retention/proof readers; new 103/104 own REL/PHY pruning. I11 above is the preserved pre-refinement mapping. No original work or completed record was removed and no R-gate is implemented by this refinement.

The completed Memory six-file run is evidence for original Step 87 only; it is not canonical corpus promotion, reader retirement, full-domain parity or R1–R22 completion. Corpus originals remain untouched by this interruption.

Temporary Ego migration surfaces added during owner execution: `src/storage_upgrade/ego.rs`, its private `stage_ego_storage_upgrade` shell entry point, Ego legacy postimage decoders/encoders and migration-only tests. They extend the existing T7 removal scope; none is a permanent runtime compatibility promise or public promotion API.

Temporary Graph migration adds `src/storage_upgrade/graph.rs`, private `stage_graph_storage_upgrade`, and Graph synthetic/authoritative conversion tests to T7. The adapter rewrites only accepted marker-linked carriers without allocating semantic publications; it retains existing mappings and relocates original marker references. Remove this offline conversion surface after canonical corpus promotion. Graph's stable-ID import-evidence serializer is an ordinary semantic comparison boundary, not a persisted legacy writer, and must remain available (or receive an explicit canonical replacement) when old wire readers/encoders retire.
