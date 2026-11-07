# G4 complete-product corpus acceptance evidence (2026-11-06)

## Decision

**BLOCKED — not accepted.**

This branch now contains substantial executable G4 evidence: a validated authored manifest; a 12-conversation, 48-turn, three-fork public replay; joined Gateway semantic, retrieval, delivery, queue, host, worker and recovery scenarios; maintenance copy tests; fresh-process CLI proof; and a standalone root-crate consumer. The G4-specific suites pass.

The governing contract nevertheless requires one completed authored cohort to carry the same owner identities and expected aliases through semantic/import, vectors/context/retrieval/use, host recovery, maintenance copies, CLI, and the external consumer, followed by an independent expected/actual/reopened comparison for every mandatory row. The current tests replay the authored cohort in several lanes but perform some deeper workflows on additional synthetic conversations or independently generated owners. The independent Luna audit therefore found that no mandatory row can be marked accepted from the current evidence.

Source commit before the uncommitted G4 work: `70c0a4a6044158a58a5e5b4185ab3a945b431a20`.

Manifest: `tests/fixtures/g4-native-rel21-v1.json`.

Manifest SHA-256: `C525A579BFA395AEBBF1C30C7D9F235CCCBBC60CC53FDFBA6DD962A68ADE9CAE`.

Fixture seed: `g4-rel21-v1`.

## Executed evidence

| Command / target | Result |
| --- | --- |
| `cargo test --test g4_public_product -- --nocapture` | PASS — 2 passed |
| `cargo test --test g4_manifest -- --nocapture` | PASS — 3 passed |
| `cargo test --test g4_maintenance -- --nocapture` | PASS — 4 passed |
| `cargo test -p reliquary-runtime-gateway g4_ -- --nocapture` | PASS — 5 passed |
| `cargo test --manifest-path cli/Cargo.toml --test g4_corpus -- --nocapture` | PASS — 1 passed |
| `cargo test --manifest-path tests/external-consumer/Cargo.toml --test process_reopen -- --nocapture` | PASS — 1 fresh-process external-consumer test |
| `cargo fmt --all --check` | PASS |
| `cargo fmt --manifest-path cli/Cargo.toml --check` | PASS |
| `cargo fmt --manifest-path tests/external-consumer/Cargo.toml --check` | PASS |
| `python scripts/check_architecture.py` | PASS — 23 packages, 76 production edges, 14 test-only edges |
| `git diff --check` | PASS |
| `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check` | PASS |
| `cargo test -p reliquary-application --lib --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1` | PASS on retry — 96 passed |
| `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1` | NOT COMPLETE — first run built but Windows could not launch the first test executable (`os error 2`); the isolated executable rebuilt and passed; the second full run was interrupted while linking |

The required full locked/offline CLI suite and external-consumer check were not rerun as final release gates after the interrupted workspace run. The linker repeatedly emits existing `LNK4098` default-library warnings. Existing unused-import/dead-code warnings in Dream, owner-session, Application, and Gateway also remain. No warning was treated as acceptance evidence.

## Row evidence and blockers

| Case | Current executable evidence | Result | Acceptance blocker |
| --- | --- | --- | --- |
| G4-01 | Public creation/reopen of two REL and two PHY owners; kind/identity/wrong-kind checks; fresh external process opens four populated owners. | BLOCKED | No fresh-process comparison of all four authored manifest owners/aliases and physical hash inventory; malformed and 2.0 cases are split across lanes. |
| G4-02 | Authored public replay checks 12 conversations, 48 accepted turns, 8/4 allocation, three forks, exact branch transcript bodies/node IDs/roles, quiesce and reopen counts. | BLOCKED | Exact bodies, parentage and branch heads are not all re-observed from a fresh process after reopen. |
| G4-03 | Accepted turn → Episode → Insomnia → REL/PHY publication with provenance, privacy, forged-owner, unauthorized-PHY and stale-claim rejection. | BLOCKED | The semantic publication flow uses additional hand-built conversations rather than every authored source/routing case on the completed manifest cohort. |
| G4-04 | Dream frontier, deterministic duplicate decision, publication retry and pair readback, plus supporting accepted supersession/zero-link workspace tests. | BLOCKED | No completed authored-cohort inventory of first-pass/canonical/duplicate/supersession/zero-link state with independent graph/history comparison. |
| G4-05 | Joined new, known and ambiguous Entity outcomes reopen with exact mention records; unauthorized owner rejects. | BLOCKED | Outcomes are on the semantic side fixture, not the authored L-05 cohort; stale-body and foreign-target negatives are not joined into the manifest oracle. |
| G4-06 | Memory vector profile/publication, wrong dimensions/profile, exact selected revision/body, repeat reuse and reopen. | BLOCKED | Separate workflow fixture; no G4 record of the six F03 golden checks or complete authored profile/generation-cut witness. |
| G4-07 | Pinned Git bytes survive working-tree edits; REL/PHY context, revocation, foreign owner and budget rejection. | BLOCKED | Not compared against the authored source/reference/grant oracle after independent reopen. |
| G4-08 | Passive lexical/vector and agentic retrieval with exact selected source and authorization rejection. | BLOCKED | Missing authored multi-candidate/community ordering and retired/pruned/inactive-branch exclusions. |
| G4-09 | No credit before delivery; confirmed-use receipt; retry/reopen idempotence; rejection, mismatch, unknown host and revocation before/during commit. | BLOCKED | No independent authored-cohort receipt/history comparison. |
| G4-10 | Native import stage, incomplete-completion rejection, exact retry, conflict rejection, scoped settlement, completion-once, deduplication and reopen. | BLOCKED | Import runs on a separate Cascade owner; supported-profile and partial-settlement negatives are not observed on the completed authored owner. |
| G4-11 | Queue fork, finalize/cancel race, checkpoints, replay, Held host state and torn-tail recovery; authored manifest replay is present in the same test module. | BLOCKED | Recovery witnesses are not tied to the complete authored result/cursor oracle. |
| G4-12 | Worker uniqueness, quiescence/restart, credential revocation, outsider denial and unknown-host Held state. | BLOCKED | Exact authorized pending-work resume and the complete grant/wrong-kind matrix are not proven on one completed cohort. |
| G4-13 | REL NewOwner/PreserveOwner rewrite, collision refusal and source/proof preservation; PHY rewrite/reopen. | BLOCKED | REL copies are Memory-only and PHY success is empty; no full completed-cohort evidence closure. |
| G4-14 | Active-conversation refusal followed by terminal conversation prune; transformed copy retains node/tombstone/history, makes payload unavailable and leaves source bytes unchanged. | BLOCKED | The terminal case is not a copy of the completed authored cohort and does not cover terminal Memory/Entity/Relationship plus profile repair together. |
| G4-15 | Supported populated REL reclaim and exact byte-identical second no-op. | BLOCKED | The owner contains no proven unreachable frames (`removed_frames == 0`); no completed-copy live-mapping comparison after actual reclamation. |
| G4-16 | Prefix and divergent REL reconciliation, collision refusal, independent reopen and byte-identical retry. | BLOCKED | Memory-only fixture; import/Dream/Insomnia/Freshness authorities from the completed cohort are not preserved and compared. |
| G4-17 | REL/PHY historical proofs plus typed wrong-owner, future, tampered and unavailable results; pruned-history proof retains generation and marks unavailable payloads. | BLOCKED | Logical cuts are split across fixtures rather than the authored REL/PHY owners before and after required mutations. |
| G4-18 | Fresh CLI processes run info/verify/history/proof/dry-run/reconcile and fail closed on collision, wrong kind, legacy aliases, unknown/duplicate flags and synthetic 2.0 input. | BLOCKED | CLI reads a separately generated manifest-shaped REL rather than the completed semantic/retrieval/host cohort; output is not cross-compared with the public API oracle. |
| G4-19 | Standalone package imports only `reliquary_memory`; fresh process reopens four owners with 48 generated turns and two REL Memories, checking bounded results, provenance/history, viewer-write denial, revocation and PHY privacy. | BLOCKED | The process cohort is populated but not the authored manifest/branches; PHYs are empty and source-hash/lost-queue witnesses are missing. |
| G4-20 | G4-specific tests, formatting, architecture and locked/offline workspace check pass; isolated Application suite passes. | BLOCKED | G4-01–19 remain blocked and the final full locked/offline workspace/CLI/consumer gate was not completed. |

## Independent review

The independent Luna audit found no new production API or production dependency edge: the Gateway additions are test-only modules and the architecture guard remains at 23 packages / 76 production edges. The external consumer depends only on the root crate, and no production 2.0 reader or `Cva` path was introduced. It independently confirmed the central specification blocker: the authored cohort is not carried as one completed artifact through all semantic, recovery, maintenance, CLI and external-process observations.

The audit also corrected a stale statement from the earlier report: the external-consumer fixture is no longer empty. It is populated and provides meaningful fresh-process evidence, but it is not the authored manifest cohort and therefore still cannot close G4-19.

## Required continuation

1. Make the manifest builder return one reusable completed cohort artifact and witness bundle, rather than replaying authored turns beside deeper side fixtures.
2. Run Insomnia, Dream, Entity, vectors, context, retrieval/use, import and Gateway recovery on those same four owner IDs and record expected/actual/reopened aliases.
3. Run rewrite, terminal prune/profile repair, actual unreachable-frame reclaim and authority-preserving reconciliation on verified copies of that completed cohort.
4. Launch the CLI and standalone consumer against those outputs and compare their observations to the same manifest witness.
5. Re-run the independent G4-01..20 audit and the complete locked/offline workspace, CLI and consumer gates without interruption.
6. Commit the accepted evidence, verify a clean worktree, then and only then set canonical `G4=complete` and hand off to D01.

No tracker or governing specification state was changed by this blocked run.
