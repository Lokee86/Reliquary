# Storage continuation evidence — 2026-11-04

## Capability reassessment

The following assessment uses the individual acceptance statements and prerequisite edges in the canonical capability tracker. Earlier statements in this file that C02–C08 are all in progress are superseded by this assessment.

- **C02 accepted:** Archive tests `accepted_turns_branches_episodes_and_owner_identity_reopen_exactly` and `accepted_source_attachments_and_source_timestamp_survive_reopen` verify typed turns, activity and source identity, and retained history. `echo_is_separate_source_keyed_evidence_and_reopens_without_activity_effects` verifies separate Echo evidence. Owner/scope and conflicting-retry negatives pass.
- **C03 accepted:** `creation_revision_provenance_and_body_survive_reopen`, `retries_conflicts_immutable_body_and_terminal_deletion_are_explicit`, and `missing_memory_and_missing_body_backing_are_distinct` cover every stated read/reopen condition, including absent payload. Owner and malformed-record negatives pass.
- **C04 incomplete:** owner replay derives local Memory/Entity closure from durable stores and checks the physical prefix; Graph and Community reopen tests pass. Principal-association history still fails closed because authenticated historical principal proofs are unavailable. The approved typed-Graph decision forbids cross-owner Graph edges and traversal; cross-owner relational Observation evidence is a separate concern. Earlier references in this file to cross-owner Graph closure are superseded by this correction.
- **C05 accepted:** `entities_and_ambiguous_source_spans_survive_reopen` covers owner-bound entities, ambiguity, spans and revisions. Owner-session replay checks referenced Memory and available source spans and rejects references outside the physical prefix. The persistence store records supplied resolution facts and does not synthesize Observations.
- **C06 accepted:** `phy_private_state_and_revision_history_survive_reopen`, `rel_rejects_private_identity_and_personality_writes`, and `rel_replay_rejects_private_phy_records` cover scope, history and privacy. Supplied existing synthesis text is persisted; no future synthesis algorithm is added.
- **C07 accepted:** `f03_reference_identity_vectors_match` and `file_identity_matches_f03_reference_vector` compare all six captured F03 reference hashes. Vector-store tests reopen packed payloads, profiles, mappings and generations, and reject missing or inconsistent backing. Durable Archive-fragment publication and generation provenance remain later owner/publication work, not a claim made by these store tests.
- **C08 accepted:** `births_admissions_uses_and_settled_cuts_survive_reopen` checks accepted source, event proof, scores and historical cuts; `access_without_accepted_use_receipt_cannot_reinforce_freshness` rejects unreceipted retrieval reinforcement; unsupported FRS07 fails closed. This does not claim offline conversion or import settlement is complete.
- **C09 incomplete:** compact Dream Begin prefix proof supports grouped and versioned direct births, but external/import birth evidence and unversioned first-publication coordinates still fail closed. Those accepted record forms require authenticated physical-prefix and cohort proof before admission is complete.
- **C09 history-cut progress:** compact Begin now counts owner-local Memory revisions and tombstones at their physical prefix without treating them as births. A reopened direct-birth fixture with a prior tombstone retains the original population, and a forged history clock is rejected. Full Memory replay remains responsible for semantic validation.
- **C09 Freshness progress:** valid current-format Freshness journal records no longer block compact Begin replay or manufacture Dream birth proof. The journal decoder checks framing and owner; the Freshness store still validates its complete event history. Reopen with a real Freshness birth passes and a forged journal owner fails.
- **C10 incomplete:** depends on C04 and C09; RelSession rejects routed external Memory/routing metadata until it can authenticate owner-bound evidence. Supported historical cuts and ordered replay cannot be accepted across those missing proofs.
- **C11 incomplete:** depends on C04 and C09. PHY kind/privacy separation exists, but principal-association historical proof and supported Dream admissions remain unavailable, so principal-independent replay is not established for those histories.
- **C12 not yet dependency-ready:** grouped Insomnia commit, post-sync replay fencing, tombstones and absent history have focused tests; C10 and C11 must pass before accepting the grouped-commit/history capability. Cross-file atomicity is not claimed.
- **G1 not accepted:** depends on C12. Run the storage gate after its prerequisites are accepted; workspace tests alone do not establish this gate. G2 remains downstream of G1.

The focused Archive, Memory, Perception, Ego, Vector and Freshness tests and the 23-package/76-edge checker passed during this reassessment. F03's old-format corpus limitation is separate from current REL 2.1 runtime acceptance.

C02–C08 remain in progress; no product gate is accepted by this record.

- C02: Archive and Echo fresh projections replay before their first write, rejecting conflicting existing identities (`84aef57`, `06bb6af`).
- C03: Memory mutations check owner and file identity (`bdb0ff5`); initialization refuses existing records (`5ddee31`); metadata encoding precedes body append (`aacad40`).
- C04: Community validates against the same owner and container identity as its Graph (`e36ed3a`).
- C05: Entity writes check file kind and scope as well as owner ID (`a06acf4`).
- C06: fresh Ego projections replay before mutation and owner checks include container identity (`3e38c32`, `68406005`).
- C07: five vector format initializers reject duplicates before append; their stores reopen after rejected retries (`51c84ea`).
- C08: Freshness initialization rejects existing records before appending a policy (`70d552e`).

Focused package tests and the 23-package, 76-edge architecture checker passed for these changes. The complete workspace suite passed with loopback access through f0428f. Remaining capability acceptance includes source authorization, grouped publication and recovery, exact cross-owner closure, and the other requirements in the canonical tracker. The offline conversion corpus proof remains incomplete; normal runtime remains REL 2.1 only.

## Later storage work

- `4ee297a` accepts a grouped completion that names both existing and new owner Memories; replay retains both and rejects unknown IDs. `e95b3a2`, `daad009`, and `e92272a` fence uncertain owner writes and track the writer epoch.
- `12dca0e` creates initialized REL and PHY writers; `fe8a080` verifies caller Episode admissions, and `40d0637` reconstructs Episode work from Archive on reopen and supports live finalization and work admission.
- `2b2f92e`, `d2cd075`, `97ee32d`, and `06a4593` check owner Graph and Entity mention closure at replay. Principal-association history and authorized cross-owner Graph references remain unsupported without owner-authenticated proof; unavailable Memory payload is not synthesized.
- `52795d7`, `70d9a0f`, and `6fd0cc6` retain Entity/mention revisions, validate Entity identity and timestamps, and recover exact mutation retries.
- `80a204b` through `2f5ddad` validate Memory metadata and recheck temporal proofs when backing arrives, using a rebuilt body index.
- `d785e9c`, `eeb0e80`, and `4fba38f` preserve retired-profile and pruned-vector history while validating packed backing and rejecting new writes against unavailable payloads.
- `9205b07`, `1e00453`, `52bfcee`, and `cc96500` harden Freshness replay and retain accepted-use proof. `5e2e8e3` rejects partial Insomnia lease records; `c80804c` bounds malformed Archive attachment counts.

These are verified slices, not acceptance of C02–C12 or G1. Full-workspace tests passed with loopback access through `2b2f92e`; later changes require a fresh full-workspace run. PHY routed publication remains closed pending sealed candidate evidence; principal and cross-owner Graph proofs remain incomplete.

Continuation: `79a049e` implements deterministic Archive fragment identity and branch windows using the pinned reference hash and overlap algorithm. It does not persist fragments or assign Archive/global publication coordinates. Those require the owner publication history model; until then Archive-vector fragment provenance and generation cuts remain unverified. Dream compact Begin admissions likewise still require owner-derived physical-prefix proof. No acceptance gate is claimed.
The full locked offline workspace test suite passed with local loopback access at `79a049e`; workspace check, formatting, and the 23-package/76-edge architecture guard also passed. This is not product gate acceptance.

`4ee49d2` adds a validated, immutable Container publication-envelope decoder for future Archive and Dream replay. It exposes physical version, optional transaction time, reservation form, and opaque body; it does not establish Archive-local clocks or Dream prefix authority. Container tests, workspace compilation, formatting and the architecture guard passed after this change; the full suite passed immediately before it at `79a049e`.

`bdf90ec` exercises new REL creation through native turn, Episode, source-grounded Memory publication, reopen, and retry. `3463d1e` checks retries against the originally published Memory revision after later updates. `cac8a5f` and `1116b5f` reject Dream admissions naming absent owner records, future cuts, or identities created after a nonzero local cut; these are necessary checks, not physical-prefix, global-order, or birth-cohort authentication. Full locked offline workspace tests passed with loopback access at `1116b5f`; no capability or product gate is accepted by this evidence.

`0e69618` requires grouped completions to follow their source Episode physically; `86cb69c` also checks the Freshness birth turn against accepted Archive activity at the carrier's physical prefix. Tests rewrite valid files to place the Episode later or forge the activity cut and confirm owner open fails. Full locked offline workspace tests passed with loopback access at `86cb69c`; the production graph remains 23 packages and 76 edges. Dream physical birth/cohort prefix proof and durable Archive publication/fragment history remain incomplete.

`c1db511`, `b1d340b`, `3937a9e`, `3b01711`, `3ee33ae`, and `0a9300d` reject grouped Memory references, Graph endpoints, Entity mentions, Community cuts, and Memory-vector backing that appear only after the referring record. Negative tests move otherwise valid records across their source boundary; retained and pruned history remains separately readable. Full locked offline workspace tests passed with loopback access at `0a9300d`; these checks do not accept G1 or authenticate Dream birth/cohort proofs.

`4b6d7d3` authenticates compact Dream populations for supported grouped births at each physical Begin, including sibling-cohort exclusion and source/Graph prefix checks. `c548146` adds replay of versioned Memory publications; `e6f3319` uses their actual global coordinates and direct intent/consume evidence for compact populations, including reversed local/global order and missing-intent rejection. Unsupported external/import birth evidence and non-versioned first-publication coordinates still fail explicitly; this does not complete C09 or G1. Full locked offline workspace compilation and tests passed with local loopback access at `e6f3319`, along with formatting and the 23-package/76-edge architecture guard.
