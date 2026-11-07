# G4 complete-product corpus acceptance evidence (2026-11-06)

## Decision

**Accepted — G4 complete. All 20 mandatory cases PASS.**

The completed authored cohort passes the joined product scenario, including terminal Memory/Archive/Entity/Relationship maintenance. Two independent regenerations produced identical semantic witnesses. The actual CLI/root-consumer process run and every required locked/offline product gate pass on source `31e7adb2`: 629 workspace tests, 8 CLI tests and the separately enabled consumer integration test. This evidence is committed before the canonical G4 status update. D01-D03 and G5 remain separate and unstarted.

Governing specification, corpus matrix and execution plan are the three `g4-*-2026-11-06.md` documents in the adjacent planning worktree's `docs/architecture` directory. Previously accepted GM exclusions remain in force.

## Reproducible corpus and observations

- Manifest: `tests/fixtures/g4-native-rel21-v1.json`; seed `g4-rel21-v1`.
- SHA-256: `FF996170B657EDEC07CD96DD6EE65D8FB6D6DA6643360D160DB0687AC267E2CD`.
- Base inputs: two REL owners, two private PHY owners, 12 conversations, 48 accepted authored turns, three forks, exact Unicode/multiline source text and eight primary source/routing expectations.
- L03's two replies have assistant roles. The six authored fork children enter through G4-T02's existing authenticated owner-session fixture boundary (`accept_turn_and_advance_branch`); all other authored turns use Gateway submission/finalization. This is accepted native fixture publication, not a claim of live host assistant-output publication. No physical store injection creates successful semantic state.
- The public queue smoke intentionally submits script text as user messages. Exact authored roles, parent paths and canonical flags are verified by the joined fixture and separate consumer.
- Supported fixture creation fixes the four owner IDs to bytes 211/213 for Lantern/Cascade and 212/214 for private A/B. Every later stage reopens those same owners or a verified maintenance copy. No old-format corpus is input.
- Completed sources contain REL Memory populations 9/2 and PHY populations 2/1. Additive native import and accepted lifecycle turns extend the 48-turn base. The finalize/cancel race is checked against its actual single winner.
- `scripts/run_g4_acceptance.ps1` generates two fresh cohorts, compares the whole semantic witness, builds and launches the real CLI and root-only consumer, verifies negative commands, compares owner hashes and writes `witness.json`, `process-witness.json`, and `cli-witness.json`.

Current generated bundle: `.test-tmp/g4-completed-776c0bd2d3a54d2aa0b55bfdab33db9e` and its `-repeat` sibling. Generated owner binaries are disposable outputs and are not committed.

## Mandatory matrix

The common executable is `reliquary-runtime-gateway::g4_retrieval_acceptance::g4_t04_joined_vectors_context_retrieval_and_confirmed_use`. Its test-only helpers share the same completed owner paths. `g4_export` independently reopens all four owners before export; maintenance helpers reopen each transformed output. Process-dependent rows passed against the actual CLI and consumer processes.

| Case | Authored positive and independent observation | Negative evidence | Status |
| --- | --- | --- | --- |
| G4-01 | Four distinct identities/kinds/scopes; exact 48-turn base; populated REL/PHY owners; export hashes | Wrong-kind, malformed and 2.0 input rejection without mutation | PASS |
| G4-02 | Exact accepted bodies, roles, timestamps, parent paths and three canonical/alternate forks; historical cuts reopen | Rejected/provisional/cancelled/Held content has no accepted Archive publication | PASS |
| G4-03 | Exact source clauses publish to declared REL/PHY through Episode/Insomnia; terminal work receipts and private provenance reopen | Invalid Episode, forged quote, unauthorized routing and stale claims; no rejected Memory birth | PASS |
| G4-04 | First-pass duplicate canonicalization, supersession, one edge, zero-link pair, stable retry; fresh Dream/Graph inventory | No repeated publication or phantom zero-link edge; stale prepared work rejects | PASS |
| G4-05 | New/known/ambiguous Mira outcomes and private PHY Entity tied to exact source cuts; reopened mention IDs | Foreign Entity target, foreign-owner access and stale-body publication reject without owner-cut change | PASS |
| G4-06 | Source-bound Memory/Archive profiles, rows and generation payloads; reopen/retry makes no redundant embedding | Dimension/profile/source-cut failures preserve prior state; F03 six identity goldens required in final suite | PASS |
| G4-07 | Authorized REL/PHY context and exact pinned Git bytes survive working-copy edits and fresh repository read | Forged revision/hash, revoked project source, foreign PHY and budget overflow reject | PASS |
| G4-08 | Exact authored lexical and multi-candidate vector order; documented missing-community global fallback; canonical filtering | Foreign access denied; archived, superseded, terminal and pruned Memory excluded | PASS |
| G4-09 | No credit before external consumption; exact host manifest and one accepted-use receipt; recovery/retry stable | Rejected/unknown host, manifest mismatch, revocation and stale prepared delivery give no credit | PASS |
| G4-10 | Additive native Cascade input staged, scoped claim settled, completed once, deduplicated and reopened | Incomplete settlement/completion, changed admitted profile and conflicting retry reject unchanged | PASS |
| G4-11 | Durable queue/checkpoint/replay, single finalize/cancel winner and accepted Archive receipt | Foreign submitter, corrupt tail, cancellation and unknown host retain correct private/Held state | PASS |
| G4-12 | Unique workers drain/restart; exact pending operation resumes under a new valid credential without a second handoff | Revoked/foreign credentials, wrong owner/kind and unsupported host contracts reject or remain Held | PASS |
| G4-13 | Populated PHY NewOwner/PreserveOwner rewrites reopen with retained Memory/provenance and new history generation; verified copies preserve all four owners | Stale plans/collisions reject; completed REL operational rewrite is the accepted GM typed stop, with no output | PASS |
| G4-14 | Separate completed-cohort copies remove one terminal Memory body, Archive turn payload, Entity description and Relationship description; tombstones/history reopen; unused profile retired | Vector/live references refuse; source bytes unchanged; missing/tampered prune evidence fails owner admission | PASS |
| G4-15 | An actual unreachable backing frame is removed; live authority/vector generations compare independently; second reclaim is byte-identical with zero removals | Unknown/unproved references fail closed; no unverified candidate publication | PASS |
| G4-16 | Prefix/divergent Lantern and imported Cascade copies retain accepted branches, historical proofs, Insomnia/Dream/Freshness/import authority; retry byte-identical | Wrong owner and conflicting accepted bodies produce no output and preserve both sources | PASS |
| G4-17 | Three authored REL cuts, PHY histories, maintenance proofs and typed absent terminal payloads observed from fresh readers | Foreign/future cut, unknown Memory and corrupt revision distinguished; corrupt bytes unchanged | PASS |
| G4-18 | Real REL CLI create/info/verify/history/proof/dry-run/copy compared with public API JSON | Legacy aliases, PHY command, duplicate recognized flag, unknown flag, 2.0/wrong kind and collisions must return nonzero JSON errors | PASS |
| G4-19 | Separate package imports only root product crate; exact source transcripts, title/Memory/provenance, private owner histories and quiesce/reopen | Viewer writes and revoked/foreign reads deny; queue receipt must agree with Archive; cancelled/Held output absent | PASS |
| G4-20 | Required workspace/CLI/consumer/format/DAG gates and independent review | No skipped mandatory scenario or changed production dependency graph | PASS |

G4-13 does not require broadening accepted GM behavior. `docs/gm-maintenance-history-parity-gate-2026-11-06.md`, M01 row, explicitly excludes operational Dream/Insomnia/import/Echo/external carriers from fresh-history reconstruction. The G4 matrix permits those prior exclusions when their no-output stop is proven. Populated PHY rewrite supplies both supported positive policies.

## Corrections and review

### Executable and independent-evidence index

Every row below uses manifest seed `g4-rel21-v1` and SHA-256 `FF996170B657EDEC07CD96DD6EE65D8FB6D6DA6643360D160DB0687AC267E2CD`. Positive/negative assertions and results are the corresponding mandatory-matrix row above. `J` means the `reliquary-runtime-gateway` test binary, function `g4_retrieval_acceptance::g4_t04_joined_vectors_context_retrieval_and_confirmed_use`; named helpers below execute inside J. `W`, `P`, and `C` mean the generated bundle's `witness.json`, `process-witness.json`, and `cli-witness.json`. W is exported only after `g4_export::verify_semantic_reopen` and its independent owner readers succeed. P comes from the separately launched root-only consumer; C records actual CLI arguments, exit codes and parsed JSON. These are output evidence, not alternative scenario oracles.

| Case | Executable/helper | Independent evidence | Implementation/correction commits |
| --- | --- | --- | --- |
| G4-01 | J; root `g4_manifest` schema tests; consumer `inspect`; process script | W `owners`, P owner IDs/counts, C info/verify and format/kind denials | `e87a32c1`, `02d6e279` |
| G4-02 | J → `g4_test_cohort::replay_authored_manifest`; consumer `inspect` | W `historical_cuts`; P `authored_turns_checked=48`; manifest transcript comparison after new-process open | `42aa2c9c`, `02d6e279` |
| G4-03 | J → `complete_authored_semantics`, `verify_semantic_negatives` | W `semantics.sources`, `episodes`, `negative_and_private_entity`; fresh Memory/provenance reads | `30f6e69d`, `8aeb6d3f` |
| G4-04 | J → `complete_authored_semantics`; export `verify_dream_and_entity_reopen` | W `semantics.dream`; reopened canonical/duplicate/supersession and Graph observations | `91ad46b9`, `b5198085` |
| G4-05 | J → `complete_authored_semantics`, `verify_semantic_negatives` | W `semantics.entity`, `negative_and_private_entity`; independent mentions/ownership reads | `1daeea4e`, `2190a8f9`, `a4bbb6ae` |
| G4-06 | J → `complete_vector_coverage`; vector-store `tests::f03_reference_identity_vectors_match` | W `retrieval`, maintenance `retained_authority` independent vector payloads; final suite's six-golden test | `ea587760`, `90de9fa2` |
| G4-07 | J context/VCS assertions | Reopened pinned repository bytes and historical owner cut; forged revision/hash leaves cut unchanged | `b5198085`, `a4bbb6ae` |
| G4-08 | J authored route; `g4_terminal_acceptance::assert_terminal_retrieval` | W `retrieval`, `terminal_maintenance`; fresh read-only Gateway excludes retired/pruned Memory | `a4bbb6ae`, `b5198085` |
| G4-09 | J host-confirmed delivery and receipt assertions | W `retrieval`; reopened host manifest/Freshness receipt before and after retry | `ea587760`, `6c5719c7` |
| G4-10 | J → `complete_authored_native_import` | W `semantics.native_import`, `maintenance.cascade_import_reconcile_history`, retained import receipt/settlement | `30f6e69d`, `90de9fa2` |
| G4-11 | J → `g4_gateway_acceptance::complete_cohort_lifecycle`; consumer process integration test | W `lifecycle`; P `gateway_queue`, accepted Archive transcript, absent cancelled/Held text | `76af0ef5`, `02d6e279` |
| G4-12 | Same lifecycle helper; `g4_joined_gateway_queue_host_stream_reopen_and_worker_lifecycle` | W `lifecycle`; worker restart and fresh authenticated recovery observations; P revoked access checks | `6c5719c7`, `76af0ef5` |
| G4-13 | J → `g4_completed_maintenance::verify_completed_cohort` | W `maintenance.owners`; independently reopened PHY new/preserved identities and all verified copies; REL typed no-output stop | `7c381302`, `448af6c3` |
| G4-14 | J → `verify_terminal_semantics`, `verify_completed_cohort`; owner-session prune regression suite | W `terminal_maintenance`, maintenance Archive/profile operations; independent tombstone/absent-body reads | `cc91d35b`, `4f62cf94`, `95b9981d`, `42aa2c9c`, `31e7adb2` |
| G4-15 | J → `verify_completed_cohort`, `compare_preserved_authority` | W maintenance reclaim operation and `retained_authority`; separate source/destination readers, source hashes | `7959a713`, `90de9fa2` |
| G4-16 | Same completed-maintenance helpers; owner-session native Dream reconciliation regression | W maintenance reconciliation flags, `retained_authority`, conflict rejection and exact retry | `17fc1ec1`, `15edf96b`, `4e2afd47` |
| G4-17 | J → export, completed-maintenance and terminal helpers; root `g4_maintenance` historical-cut test | W `historical_cuts`, corruption operation, terminal payload observations; P histories and C proofs | `90de9fa2`, `b5198085` |
| G4-18 | `scripts/run_g4_acceptance.ps1` → real `reliquary.exe` | C command/exit/JSON snapshots compared with P; source SHA-256 preserved | `02d6e279`, `4e2afd47` |
| G4-19 | Consumer `inspect`; `process_reopen::external_root_crate_consumer_reopens_the_exported_authored_cohort` | P exact transcript/provenance/privacy/queue/reopen assertions and byte-identical sources | `02d6e279`, `84fefb44`, `a3dc8fa2` |
| G4-20 | Required final commands below; independent Luna review | Final gate logs/result JSON, source commit, fixture digest and 23/76/14 DAG report | Final acceptance commit records evidence only |

- Reclamation now retains validated operational journals and Archive retention markers instead of rejecting the completed owner.
- Reconciliation reuses the owner-admitted Graph projection; reopening a standalone Graph store had rejected retained grouped Dream history. The targeted divergent Dream regression passes.
- Named, source-bound lifecycle commands support authorized retirement/mention withdrawal/Relationship creation without exporting writable stores. Four lifecycle regressions pass, including complete-name matching and stale/forged source refusal.
- Terminal pruning removes selected body entries from admitted grouped Insomnia completions, preserves creation/lifecycle/receipt/work evidence and untargeted bodies, relocates vector receipts and accounts for grouped publication clocks. Existing v1 proof domains remain unchanged. The four focused prune tests pass, including missing/altered manifest rejection.
- Independent Luna review assessed architecture and contract separately. It found no remaining actionable defect in the final grouped-prune change. It confirmed that lower MemoryStore history intentionally represents unavailable payloads while owner-session validates grouped-payload pruning authority before returning a session; an additional replay API is unnecessary. It also confirmed the GM rewrite exclusion and the accepted fixture seam for authored reply roles.
- Closing independent review: Luna agent `g4_lifecycle_review` inspected the successful initial bundle (00544e09dad64f8998cbcf900a8786e6)'s W/P/C against the fixed manifest and found no discrepancy in identities, exact authored transcripts, semantic/retrieval/terminal observations, queue states, private routing, history/count comparisons, command errors or unchanged source evidence. This closes T08; the successful final integration results below close T09. The review did not substitute prior component success for joined-cohort evidence.
- The Windows consumer byte-comparison check initially retained its own mounted-owner leases. `84fefb44` releases those leases before the independent file read; the process integration test then passed. `a3dc8fa2` includes Cascade's already-verified history digest in consumer JSON so both REL proofs can be compared with the CLI. Neither correction changes product semantics.
- The first full workspace run exposed five existing owner-session regressions: the grouped-prune proof check had also rejected previously supported standalone Memory metadata without local body bytes. `31e7adb2` records grouped body IDs during existing owner replay and scopes the new proof requirement to those bodies, preserving the standalone contract. No test expectations changed. Independent Luna review confirmed this distinction; the complete owner-session suite then passed **165 unit + 6 integration tests**, including all five regressions and missing/tampered grouped-prune evidence refusal. Red evidence is `.test-tmp/g4-final-workspace-test-before-metadata-fix.log`; green owner evidence is `.test-tmp/g4-owner-session-final.log`.

### Captured process and maintenance values

The successful process run is `.test-tmp/g4-process-final.log`, exit 0. Its separate consumer integration test ran explicitly with `--ignored`: **1 passed, 0 failed, 0 ignored**. The two independent composed runs each passed their one selected test and their complete semantic witnesses matched. The process snapshot contains **48 exact authored turns**, Lantern **35 accepted sources / 9 Memories / 9 conversations**, Cascade **20 accepted sources / 2 Memories / 5 conversations**, and private PHY Memory counts **2 / 1**. The cancellation branch won this run's finalize/cancel race; the Archive and queue agree. Private checkpoint replay resumed at cursor 1, and Held/private/cancelled text remained absent from accepted Archive.

The process script captured **25 invocations: 17 successful (15 CLI and two consumer inspections) and eight expected CLI refusals**, all with parsed JSON. The failures cover existing output, `cva`, `phy`, duplicate scope, wrong owner kind, synthetic 2.0 header, unknown flag, and colliding reconciliation output. Both REL proof digests, bounded Archive pages, Memory history and rewrite dry-run plan matched the root consumer. All four source owner hashes remained unchanged. The console's final compact JSON summary warns about depth truncation; the actual `process-witness.json` and `cli-witness.json` use depth 16/32 and retain the nested observations.

Independent maintenance readers observed one removed terminal Memory payload, one Archive payload, one Entity description, one Relationship description, two withdrawn mentions and one retired unused profile. Reclamation removed one actual orphan frame, then removed zero on a byte-identical second pass. Five independent retained-authority comparisons preserve Memory histories, vector payloads/generations and applicable operational evidence. Both reconciliation retries are byte-identical; conflicting source bodies refuse without output. Referenced Memory pruning refuses without creating a destination.

## Final integration evidence

Production code/fixture/consumer checkpoint: `31e7adb2`. The final acceptance commit changes this report only. All commands below completed successfully on that code; the final fresh-process run followed the full suite and used the same checkpoint. Earlier interrupted or failed attempts are retained as diagnostic evidence and are not counted as final passes.

| Exact command (PowerShell, from the rebuild root) | Exit | Result |
| --- | --- | --- |
| `python scripts/check_architecture.py` | 0 | 23 packages; 76 production edges; 14 test-only edges |
| `cargo fmt --all --check` | 0 | PASS |
| `cargo fmt --manifest-path cli/Cargo.toml --check` | 0 | PASS |
| `cargo fmt --manifest-path tests/external-consumer/Cargo.toml --check` | 0 | PASS |
| `git diff --check` | 0 | PASS |
| `cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check` | 0 | PASS |
| `cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1` | 0 | 629 passed; 0 failed; 0 ignored, including all workspace doctest targets |
| `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1` | 0 | 8 passed; 0 failed; 0 ignored |
| `cargo check --manifest-path tests/external-consumer/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check` | 0 | PASS |
| `& ./scripts/run_g4_acceptance.ps1` | 0 | Two joined runs; deterministic semantics; 1 separate consumer test passed; 25 real command observations; unchanged sources |

Exact per-command logs, durations and exits are in `.test-tmp/g4-final-gates.json` and `.test-tmp/g4-final-*.log`. The process log is `.test-tmp/g4-process-final.log`. The full suite explicitly reports `tests::f03_reference_identity_vectors_match ... ok`, which contains all six fixed identity goldens. The root consumer's process test is explicitly enabled by the process script and finishes with **0 ignored**; its normal `#[ignore]` fixture requirement is not used as a pass. No mandatory scenario is skipped.

Required commands: architecture guard; workspace/CLI/consumer formatting; locked/offline workspace check and full test; full CLI test; external-consumer check; fresh-process script. Cargo operations are serialized against `target-codex-check`, using repository-local temporary outputs. Windows hard-link operations require ordinary host filesystem access; restricted runs that denied those operations are not counted as product failures.

Compiler warnings are recorded separately from pass/fail: unused imports (`MemoryBodyId`, `MemoryStore`, `PathBuf`, `std::io::Read`, `super::*` and test host types), existing unused helper methods/functions, an unnecessary test `mut`, and Windows linker `LNK4098`. These diagnostics remain in the complete logs; none is used as acceptance evidence.

G4 closes with the accepted evidence committed and a clean rebuild worktree, followed by the scoped planning tracker/execution-plan commit. Only this task's three integrated worker worktrees were removed; their branches and residual patches remain available. No source release, archived-corpus conversion or activation is part of this acceptance.

## Documentation impact

- Inspected: the three governing G4 documents, capability tracker, accepted GM/P01 scope and local agent instructions.
- Updated: this acceptance record and committed fixture/process checks; the canonical G4 tracker/execution-plan status follows the clean acceptance commit.
- Not affected: D01-D03/G5, archived 2.0 corpus, release/activation instructions and the pinned reference implementation.
- Compliance check: final formatting and diff checks; planning documentation policy was run before status edits to establish its existing baseline.
- Known documentation gaps: before any planning status edits, `python ../engineering-standards/tools/docs_policy/check.py --repo .` exited 1 with 117 pre-existing findings (missing standard sections and index links, including the supplied G4 planning documents). `ddocs check --indexes --links` also exited 1, naming the existing `docs/INDEX.md` and `docs/architecture/INDEX.md`. Logs are `.test-tmp/g4-planning-doc-policy.log` and `.test-tmp/g4-planning-ddocs.log`. These planning-document failures are recorded separately from native G4 executable acceptance; unrelated documentation is preserved.

## Architecture impact

- Standards added or changed: none; 23 production packages, 76 production edges and 14 test-only edges remain the approved graph.
- Ownership or boundary impact: concrete source-checked lifecycle commands remain at owner-session/Application/Gateway; pruning/reconciliation/reclamation fixes remain with existing durable owners. No writable lower store is exported and no corpus service, generic owner facade or 2.0 runtime path is introduced.
- Pitlord enforcement impact: no ownership-map change; the planning update changes status/evidence only.
- Other verification impact: the joined acceptance fixture and actual standalone CLI/consumer checks now cover the completed native cohort, plus the required full-suite regression gates.
- Known architectural gaps: none found in the bounded independent review; the already accepted GM fresh-history rewrite exclusions are explicitly preserved.
