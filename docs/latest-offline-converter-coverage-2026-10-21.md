# Latest offline converter coverage audit



Parent index: [Documentation index](INDEX.md)



Status: source inventory and explicit converter treatment; six-source historical

acceptance is recorded separately after its migration run.



## Purpose



Close execution302–307 without absorbing missing owner work into an audit.



## Overview



The refreshed [durable inventory](../benchmarks/history-durable-family-current-2026-10-21.json)

classifies164 exact record identities in68 families. The original frozen baseline

is preserved. `history_writer_inventory.py --output PATH` writes an explicit

report and still rejects unknown or missing identities; its default now names the

current report. `CVAINSC0` is an encoder template whose last byte is replaced

before persistence, not a durable family. CVAXPLN1 remains outside REL/PHY.



Every family below has explicit treatment. The import-evidence audit identified an unfinished dependency: normalized CVAFRS06 settlements still require their legacy predecessor and temporary epoch binding. Standalone current proof conversion and completion-digest rebinding must be verified before corpus promotion and legacy-reader retirement.

The subsequent physical corpus run exposed the retired inert Entity Resolution

family; explicit308–314 cover its contract, distinct tag, literal carrier, replay,

transport, admission and parity. The source inventory alone was insufficient to

reserve that older physical identity. The already-numbered Project

correlation step306 normalizes genuine PRJCOR01, and307 adds explicit unknown

CVA/CVD/PRJ admission before staging. No runtime owner registry is introduced.

Concrete owner replay remains responsible for validity, hash/reference and proof

checks. Known unsupported V1 prune authority remains an explicit refusal, not a

claim of conversion support.



## Family crosswalk



| Family | Exact identities | Latest converter treatment |

| --- | --- | --- |

| `container_header` | `CVA\0\r\n\x1a\n` | Container identity/UUID retained; version becomes V2; source and target admission remain explicit. |

| `global_ticket` | `CVAVERS1`, `CVAVERS2` | Relocate physical bindings; preserve every literal global coordinate and transaction time. |

| `archive_publication` | `CVAAFMT2`, `CVAAREC1`, `CVAARP02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `publication_envelope` | `CVAPUB01` | Current-only input family rejected by explicit V1 owner admission; new envelopes emitted by concrete converters. |

| `archive_content` | `CVACONT1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `archive_node` | `CVANODE1`, `CVANODE2`, `CVANOD02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_branch` | `CVABRCH1`, `CVABRC02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_conversation` | `CVACONV1`, `CVACONV2`, `CVACONV3`, `CVACON04` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_terminal` | `CVACTMB1`, `CVACTM02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_fragment` | `CVAFRAG1`, `CVAFRA02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_episode` | `CVAEPIS1`, `CVAEPI02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_file` | `CVAFILE1`, `CVAFIL02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_turn_attachment` | `CVATURN1`, `CVATURN2`, `CVATUR03` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_file_memory_link` | `CVAFMEM1`, `CVAFME02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_import_receipt` | `CVAIMPT1`, `CVAIMP02` | Archive owner converts each accepted old publication to its exact native envelope at the old publication slot; ordered facts, repeats and dependencies preserved. |

| `archive_prune` | `CVAPRND1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `conversation_compaction` | `CVACMP1\0` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `interaction_stream` | `CVAISTR1`, `CVAISTR2` | Decode old checkpoints through the concrete stream owner; emit current Begin/suffix/interruption records at old slots. |

| `echo` | `CVAECHO1` | Retain exact current typed Echo carrier and event identity/order; dedicated latest-upgrade evidence test. |

| `project_correlation` | `PRJCOR01`, `PRJCOR02` | Normalize genuine PRJCOR01 into PRJCOR02 with its accepted repository-management default; retain every field/cut/sequence and current records. |

| `project_file_binding` | `PRJFILE1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `memory_format` | `CVAMEMF2` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `memory_body` | `CVAMBDY1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `memory_revision` | `CVAMEMR2`, `CVAMEMR3`, `CVAMEMR4`, `CVAMEMR5`, `CVAMEMR6`, `CVAMEMR7`, `CVAMEMR8`, `CVAMC001`, `CVAMD001`, `CVAMC002`, `CVAMD002`, `CVAMT002` | Memory owner reconstructs accepted literal histories and emits canonical creation/delta/terminal envelopes, preserving bodies/routing, versions and times. |

| `memory_publication` | `CVAMEMV1` | Memory owner reconstructs accepted literal histories and emits canonical creation/delta/terminal envelopes, preserving bodies/routing, versions and times. |

| `memory_terminal` | `CVAMEMT1` | Memory owner reconstructs accepted literal histories and emits canonical creation/delta/terminal envelopes, preserving bodies/routing, versions and times. |

| `memory_routing` | `CVAMRTE1`, `CVAMRTE2` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `memory_prune` | `CVAMPRN1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `processing_epoch` | `CVAPEP01` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `entity` | `CVAENTF1`, `CVAENTR1`, `CVAENTR2`, `CVAENTV1`, `CVAENTT1`, `CVAENTT2`, `CVAENC01`, `CVAEND01`, `CVAENC02`, `CVAEND02`, `CVAETD02`, `CVAETM02` | Entity owner converts creation/delta/direct terminal/merge terminal publications with exact ordered revisions and version coordinates. |

| `entity_prune` | `CVAEPRN1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `entity_resolution` | `CVAERS01`, `CVAERS02`, `CVAERS03`, `CVAERL01` | Materialize accepted CVAERS01 changes into current CVAERS03 deltas. Preserve frozen-inert CVAERS02 bytes in current checksummed CVAERL01 evidence without state effects. Exact admission follows actual physical format. |

| `relationship` | `CVARELF1`, `CVARELR1`, `CVARELV1`, `CVARELT1`, `CVARNC01`, `CVARND01`, `CVARNC02`, `CVARND02`, `CVARTD02` | Relationship owner converts ordered creation/delta/direct terminal history to native envelopes; Observation is not introduced. |

| `relationship_prune` | `CVARPRN1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `graph` | `CVAGFMT1`, `CVAGNODE`, `CVAGNOD2`, `CVAGMUT1`, `CVAGMUT2`, `CVAGBAT1`, `CVAGBAT2`, `CVAGVER1`, `CVAGCM01`, `CVAGCB01`, `CVAGCM02`, `CVAGCB02` | Graph owner converts exact ordered mutations/batches, transaction identity and repeats into native envelopes. |

| `graph_rewrite_baseline` | `CVAGBL01` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `community_membership` | `CVACOMM1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `community_name` | `CVACNAM1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `dream_pairs` | `CVADRP01`, `CVADRM01`, `CVADRP02` | DreamPairStore emits exact local Memory mappings and dense pairs; canonical membership and history remain unchanged. |

| `dream_freshness` | `CVDREAM1`, `CVDREAM2` | Concrete Begin converter preserves accepted Memory/birth prefix and proofs; later journal facts remain owned by the existing replay. |

| `legacy_dream_cooldown` | `CVADREM1`, `CVADREM2` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `freshness_policy` | `CVAFRP01` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `freshness_events` | `CVAFRS02` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `freshness_import_settlement` | `CVAFRS03`, `CVAFRS04`, `CVAFRS05`, `CVAFRS06` | Existing semantic proof normalization emits current settlement authority; no replacement proof is invented from current state. |

| `freshness_legacy_activation` | `CVAFSA01` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `freshness_rewrite_baseline` | `CVAFRB01` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `import_epoch` | `CVAIEPS1`, `CVAIEPC1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `import_legacy_epoch` | `CVAILEG1`, `CVAILEG2` | Temporary explicit legacy epoch/prefix binding rebases physical transport; removal remains separately gated after corpus promotion. |

| `insomnia_format` | `CVAINSF1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `insomnia_work` | `CVAINSW1`, `CVAINSA1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `insomnia_completion` | `CVAINSC1`, `CVAINSC2`, `CVAINSC3`, `CVAINSC4`, `CVAINSC5`, `CVAINSC6`, `CVAINSC7`, `CVAINSC8` | Concrete legacy completion translator preserves accepted grouped Memory/body/routing facts and emits canonical compact completion carriers. |

| `external_index` | `CVAXIDX1`, `CVAXIDX2` | Derived latest validated namespace snapshots plus required damage barrier become CVAXIDX2; superseded derived chains are discarded. |

| `external_publication` | `CVAXMEM1`, `CVAXMEM2` | Concrete external-publication codec emits CVAXMEM2 preserving accepted Memory, body, routing and birth evidence. |

| `ego_compact_history` | `CVAECR01`, `CVAEDL01`, `CVAETM01` | Ego owner materializes its independent literal sequence and writes current creation/delta/terminal carriers; no global ticket is allocated. |

| `ego_identity` | `CVAEIDN1`, `CVAEIDN2`, `CVAEIDA1` | Ego owner materializes its independent literal sequence and writes current creation/delta/terminal carriers; no global ticket is allocated. |

| `ego_personality` | `CVAEPER1` | Ego owner materializes its independent literal sequence and writes current creation/delta/terminal carriers; no global ticket is allocated. |

| `ego_anchor` | `CVAEANC1` | Ego owner materializes its independent literal sequence and writes current creation/delta/terminal carriers; no global ticket is allocated. |

| `ego_synthesis` | `CVAESYN1` | Ego owner materializes its independent literal sequence and writes current creation/delta/terminal carriers; no global ticket is allocated. |

| `packed_vectors` | `CVAPVFM1`, `CVAPVEC1`, `CVAPVRN1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `memory_vectors` | `CVAMVFM1`, `CVAMVEC1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `archive_vectors` | `CVAAVFM1`, `CVAAVEC1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `compatibility_profile` | `CVACPFM1`, `CVACPRO1`, `CVACPRT1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `vector_generation` | `CVAVGFM2`, `CVAVGEN2`, `CVAVGRC2` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `rel_metadata` | `CVARELM1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `phy_profile` | `CVAPHYP1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `owner_maintenance` | `CVAHPR01` | Explicit unsupported V1 CVAHPR01 authority: typed refusal, no output or invented replacement manifest. Not present in admitted six-source corpus; current V2 prune/reclaim remains supported. |

| `rewrite_lineage` | `CVARGN01` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |

| `legacy_workspace` | `CVAWKFM1`, `CVAWKSP1` | Retain accepted owner facts; existing concrete relocation handles any physical references. Owner replay validates the copied family; no new representation is needed for this phase. |



## Verification and boundaries



The dedicated latest-upgrade test compares every Echo field and ordering plus

both legacy/current project-correlation sequences and complete typed fields.

It checks source bytes, global neutrality and current-only output correlation

carriers. Unknown future CVA/CVD/PRJ signatures fail planning in both REL and PHY,

without source mutation, destination or partial staging. Both tests pass.

Existing prepruned-authority tests preserve the explicit refusal policy.



The six-source run uses only the latest complete converter and compares owner

history domains and independent clocks before preserving disposable outputs.

Per-family raw accounting includes framing and verifies total bytes against the

physical file length. Promotion and removal of all temporary compatibility remain

later gates; this audit neither promotes sources nor relaxes unsupported proof

admission. Later source drift requires a fresh exact inventory and explicit

owner-specific plan steps before implementation.



## Related docs



- [Linear execution plan](minimal-lossless-history-storage-linear-plan-2026-10-19.md)

- [Execution evidence](minimal-lossless-history-storage-execution-2026-10-19.md)

- [Storage format](storage-format.md)

- [Maintainer map](maintainer-map.md)



## Notes



A lexical writer inventory is not a complete Rust call graph. Direct append sites

are checked alongside concrete owner facades, import transport, in-place

compaction and maintenance paths. The private admission list grants no validity

to a recognized payload and is removed with the one-time upgrader.

