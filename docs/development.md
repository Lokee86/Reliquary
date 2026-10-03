# Development

Parent index: [Documentation index](INDEX.md)

## Purpose

This document owns repository workflow, layout, verification commands, corpus smoke harnesses, and current measurements.

## Overview

Development uses deterministic local tests plus optional live provider execution. Model-switchboard routing, encrypted credential persistence, auth-header attachment, direct OpenAI-ready embedding/General HTTP transport, ChatGPT/Codex device-code acquisition, and provider-native Codex Responses execution are implemented. Codex OAuth token refresh remains outside the current slice. A separate repo-local CLI exposes current bring-up operations without installation.

## Repository boundary

```text
Cargo.toml / Cargo.lock              core library package + locked dependencies
cli/Cargo.toml / cli/Cargo.lock      detachable repo-local CLI package + lockfile
cli/src/*.rs                         interface parsing/prompting/rendering/dispatch over public API
src/container/                     physical REL/PHY container mechanics + global ordering
src/archive/                       Archive/source history, Episodes, files, Echo, repository correlation
src/memory/                        authoritative Memory records/revisions/provenance
src/retrieval/                     lexical/semantic retrieval, vectors, profiles, generations
src/semantic_graph/                Graph, Communities, typed semantic-node traversal
src/perception/                    durable Entity state + candidate/admission/resolution workflows
src/chronos/                       shared temporal semantics
src/ego/                           Identity/Personality/Anchor/synthesis persistence
src/runtime_inference/             Insomnia, Dream, providers, RuntimeHost, interaction orchestration
src/config_security/               local config, credentials, encryption, master-key persistence
src/facade/                        public Cva/Reliquary/Phylactery composition, migration, reconciliation
src/lib.rs                         crate-root module declarations + public exports
examples/archive_roundtrip.rs        prepared Archive corpus smoke
examples/vector_generation_smoke.rs  two-profile vector + retrieval smoke
examples/archive_open_profile.rs     open-time benchmark (allocator metrics unavailable)
examples/entity_calibration_harvest.rs deterministic frozen-corpus Entity calibration harvester
examples/freshness_calibration.rs      synthetic and verified-copy REL Graph Freshness workload profiler

docs/                                current docs, plans, limits, decisions
tools/pitlord/                       repository architecture ownership/policy
scripts/check_architecture.py        Lexicon/Arcana refresh + Pitlord enforcement
.github/workflows/architecture.yml   push/PR architecture gate
```

Do not copy large portions of previous Reliquary implementations as a migration shortcut. Reuse must preserve current ownership boundaries.

## Verification commands

```text
cargo fmt --check
cargo check
cargo test
cargo fmt --manifest-path cli/Cargo.toml -- --check
cargo check --manifest-path cli/Cargo.toml --locked
cargo test --manifest-path cli/Cargo.toml --locked
python ../engineering-standards/tools/docs_policy/check.py --repo .
python ../engineering-standards/tools/docs_policy/check.py --repo . --changed-from origin/main
python scripts/check_architecture.py --refresh
```

The ordinary `cargo test` suite contains only correctness/regression tests and should report zero ignored tests. Fourteen Community performance, routing-quality, and operating-point experiments live behind the opt-in `community-benchmarks` Cargo feature instead of registering as ignored unit tests. Use `cargo test --release --features community-benchmarks` when intentionally rerunning that benchmark/validation set. Feature-gated Community benchmarks must not become the only coverage for a correctness contract.

Test scratch paths must use collision-resistant identities such as `Uuid::new_v4()` rather than timestamp-only directory names; Rust tests execute concurrently and Windows can otherwise surface intermittent `AlreadyExists` failures.

Prepared Archive round trip:

```text
cargo run --release --example archive_roundtrip -- <graph.jsonl> <output.cva>
```

Two-profile simulated vector-generation smoke:

```text
cargo run --release --example vector_generation_smoke -- <source.cva> <output.cva>
```

The vector smoke copies the source CVA, creates two materially different deterministic simulated endpoints, establishes two compatibility profiles, builds one full Archive generation per profile, syncs, reopens, verifies both current generations, and runs the default hybrid retrieval path through one reopened generation.

Open benchmark:

```text
cargo run --release --example archive_open_profile -- <archive.cva> [runs]
```

The benchmark is standalone and reports median/p90 `Cva::open` time. Allocator-tracked retained and peak additional bytes are unavailable because Lore owns the global allocator; the harness does not install a conflicting allocator.

The test-only REL activity profiler avoids the standalone allocator conflict. Run `cargo test --lib archive_activity::measurement_tests -- --nocapture` with `REL_ACTIVITY_FIXTURES` set to semicolon-separated frozen REL paths (PowerShell: `$env:REL_ACTIVITY_FIXTURES='C:\path\first.rel;C:\path\second.rel'`). Without that variable it validates a disposable 512-turn mixed-role, mixed-format fixture. It verifies every version-linked cut and first identity position against an independent ledger and inspects hash-verified disposable copies, leaving original permissions and bytes unchanged. Both source and copy hashes are checked before/after. Hashing/copying warms the file cache before the three measured opens. Output reports three-run median total reopen time, nine-run median NodeIndex replay with/without canonical activity preflight/accounting, their signed per-node timing difference, and exact activity-vector used/capacity bytes plus struct size. Node cloning/destruction are outside replay timing. The replay difference estimates in-memory live accounting overhead; it excludes payload decoding, file I/O and sync. Timings are informational, have no pass/fail threshold, and must name the build profile and host conditions when reported. See [activity-clock validation](rel-activity-clock-implementation-plan.md).

Memory Freshness propagation profile:

```text
cargo run --release --example freshness_calibration
```

With no arguments, the harness builds a deterministic synthetic dense graph (`synthetic_dense_local_v1`) for reproducible algorithm/workload measurements. Real-fixture mode requires both `--fixture <REL path>` and `--sha256 <64-hex>`; run the following against the authoritative frozen files when available:

```text
cargo run --release --example freshness_calibration -- --fixture "C:\!bin\workspace\reliquary-fixtures\local\authoritative\chatgpt-first14d\project.prj.rel" --sha256 49751a62dd8644800d19976a0696a69366dc1e4f8d7c39bb4b69fbd7a8a5d18b
cargo run --release --example freshness_calibration -- --fixture "C:\!bin\workspace\reliquary-fixtures\local\authoritative\chatgpt-first28d\project.prj.rel" --sha256 8f4a2bf871d7f5368edde93954a2260a29c86131b28475b323a7b6c70f8c51e4
cargo run --release --example freshness_calibration -- --fixture "C:\!bin\workspace\reliquary-fixtures\local\authoritative\rhelm-david-r-ellis-pre-relationship\project.prj.rel" --sha256 3efd9c666aa992686abd8aac9b77cfc7dc2a0e15d6fb26ec01f8666db175e476
```

The harness checks the source digest against `--sha256`, copies the REL to a collision-resistant temporary path, verifies the copy digest, opens only the copy, and checks the source digest again before cleanup. It projects current owner-local active Memory-to-Memory topology and runs hypothetical +25/+50 propagation roots at 1/2/4/8 workers; a Community snapshot is used only when it matches the current Graph version, otherwise membership is conservatively unknown. Output includes p50/p95/p99 batch and per-event latency, touched Memories, examined edges, maximum frontier, and speedup relative to one worker. These are prospective workload measurements on present fixture topology. The fixtures do not contain time-positioned admission, Dream-link, or accepted-use histories, so the harness does not reconstruct historical score trajectories or calibrate Freshness policy. See [Freshness policy and implementation status](memory-staleness-plan.md), [implementation plan](generic-staleness-implementation-plan.md), and [current limitations](current-limitations.md).

Entity calibration harvest:

```text
cargo run --release --example entity_calibration_harvest -- <project.rel> <user.phy> <output-dir>
```

The harvester performs no inference and never mutates the source owners. It snapshots every current frozen REL/PHY Memory into a deterministic candidate inventory, binding each case to exact `MemoryId + MemoryBodyId`, and writes a fixed 256-case shadow sample ranked by SHA-256 over the v1 seed, owner ID, Memory ID, and body ID. The shadow selector reserves up to 32 cases per owner before filling the remainder globally by rank so PHY coverage cannot disappear through population imbalance. Human-reviewed gold and append-only regression labels remain corpus-repository data rather than implementation-repository fixtures.

Synthetic Insomnia mechanical stress:

```text
cargo run --release --example insomnia_stress -- [episodes] [workers] [dimensions] [delay_ms] [evidence_every]
```

The stress harness creates isolated import Episodes with materialized Archive fragments, emits one directly authorized Memory per Episode through a deterministic General endpoint, builds simulated Memory Vectors, syncs, reopens, and verifies Episode, Memory, and vector counts. `evidence_every=0` disables historical evidence; a positive value forces every Nth Episode through one lexical `archive_search` before final synthesis.

## Prepared-corpus measurements — 2026-08-14

The prepared corpus contains 12 conversations and `1,875` semantic Archive mutations: `1,559` nodes, `35` current branches, `281` fragments, `1,553` content objects, and `3,603` expanded fragment references.

### Current-format baseline

The required-marker CVA with no profiles/matrices/Archive-Vector sets/generations is `2,197,562` bytes.

Release warm-cache `Cva::open`, 50 runs:

```text
median:          25.253 ms
p90:             26.163 ms
retained heap:   757,051 bytes
peak additional: 888,730 bytes
```

The retained-heap increase from the prior slice includes one derived `u64` Archive creation version per fragment, used to verify VectorGeneration source coverage.

### Simulated vector-bearing smoke

The same corpus with two simulated profiles and two current generations is `2,289,410` bytes. Each profile uses 32-dimensional `f32` rows and covers all 281 fragments, for `562` total Archive-Vector rows. The current retrieval smoke reopens that CVA and exercises the original default 30-candidate / top-10 hybrid path through one selected profile/generation without changing file size.

Release warm-cache `Cva::open`, 50 runs:

```text
median:          25.147 ms
p90:             26.145 ms
retained heap:   760,578 bytes
peak additional: 910,413 bytes
```

These 32-dimensional simulated vectors verify machinery and ownership; they are not a performance proxy for eventual production embedding dimensions or scalar representation.

### Live OpenRouter embedding throughput — 2026-08-15

The current configured live route is `qwen/qwen3-embedding-8b` through OpenRouter, with 1024-dimensional L2-normalized output. Throughput was measured over the same 281 durable fragments using a disposable runner under `target/` that calls `OpenAiReadyEmbeddingEndpoint` directly. Cargo compilation, CVA publication, and compatibility probes were excluded from the timed region.

At fixed batch size 16:

```text
concurrency  wall time   fragments/s
1            36.149 s     7.773
4            16.397 s    17.138
8            10.453 s    26.882
16            8.733 s    32.176
```

At fixed concurrency 16:

```text
batch size   wall time   fragments/s
8             9.870 s    28.471
16            8.733 s    32.176
24           11.344 s    24.771
32           11.415 s    24.617
48           17.393 s    16.156
64           14.824 s    18.956
```

The best observed configuration was 16 inputs per request with 16 concurrent requests: `4.14x` the serial throughput and about `1.70x` the throughput of the former 64×16 default on this corpus/provider. The default live batching policy now uses 16×16.

The same live run exposed compatibility-policy calibration rather than an embedding transport failure. Eight repeated profile/re-probe comparisons on the same configured route produced minimum cosine values from `0.99988147` through `0.99993311`; the former policy-v1 threshold `0.99999` rejected every sample. Compatibility policy v2 therefore uses `0.9998`, with the policy version bumped so existing v1 profiles are not silently reinterpreted.

After applying those measured defaults and policy v2, a fresh copy of the zero-vector fixture completed the ordinary `vectors build` path with no batching overrides in `25.251` seconds. The resulting CVA is `3,374,366` bytes and reopens with one 1024-dimensional `f32` matrix containing all `281` fragment rows, one Archive-Vector binding, one compatibility profile, and one active generation at source Archive version `1875` / vector version `1`.

A live default hybrid search over that published generation for `compatibility profiles vector generations` returned the configured top 10 results in `12.828` seconds. All ten carried semantic scores from the active live generation, confirming endpoint re-verification, query embedding, exact semantic scan, row-to-fragment mapping, and hybrid result assembly after reopen.

### Live Codex General transport smoke — 2026-08-15

The configured ChatGPT/Codex route was set to `gpt-5.6-luna` with `low` reasoning and the existing device-code credential. A disposable one-conversation/two-turn CVA was imported and processed through the ordinary `insomnia run` path with one worker. The first backend probe exposed an invalid `reasoning.summary = none` request; the transport was corrected to `auto`. The fresh rerun completed one Episode in one claim with zero retries, zero terminal failures, zero Memories, and no embedding work, confirming live authentication, account routing, Luna model selection, low reasoning, structured Responses output, SSE parsing, and Insomnia provider dispatch.

### Live Dream Ox Alpha validation — 2026-08-25

The first representative Dream retrieval/inference validation used `stealth/ox-alpha` through OpenRouter against a disposable copy of an Insomnia-generated CVA containing 103 current Memories and one 1024-dimensional Memory-vector compatibility profile. The validation fixture contains nine unambiguous synthetic relation/direction cases, thirteen hand-selected related real-Memory pairs, and four unrelated negative pairs.

Initial `response_format` calls exposed provider-structure drift: Ox returned semantically plausible JSON but did not preserve the required evidence-array shape. The OpenAI-compatible Dream transport was therefore changed to force exactly one named function call with the Dream JSON schema as function parameters and to parse only that function's arguments. The temporary alternate-shape normalization used during diagnosis was removed. The transport also performs bounded retry/backoff for retryable connection/timeouts and HTTP 408/409/429/5xx responses because the OpenRouter shared Ox pool produced intermittent upstream 429s during bring-up.

The unchanged validation fixture then completed with:

```text
synthetic relation/direction: 9/9 exact
synthetic non-none verifier:   8/8 accept
real related-pair recall@12:  13/13
real related classification:  13/13 non-none
real related verifier:        13/13 accept
real unrelated negatives:      4/4 none
```

The related-pair candidate ranks were `2, 4, 4, 6, 1, 5, 1, 12, 4, 1, 6, 1, 4`. This is a small seam-validation fixture, not a population-level accuracy estimate. Because the broad verifier accepted every selected related proposal and corrected none, this run does not justify enabling factual/causal/recurrent verification by default.

A second write-enabled Ox validation used three disposable CVAs to exercise the semantic-state seams omitted by the first corpus run. The temporal-only case disabled semantic and lexical lanes and left the matching candidate unvectorized; `tomorrow` at an authoritative 2026-08-24 source timestamp matched an explicit `2026-08-25` candidate, Ox classified the pair `topical`, Graph publication succeeded, and the extracted source advanced to `knowledge`. The supersession case produced and independently verified `NEW --supersedes--> OLD`; Graph direction was correct, OLD became archived with `superseded_by = NEW`, and NEW advanced to active knowledge. The duplicate case classified and verified two identical observations introduced out of chronological order; Graph rewired them to `newest -> middle -> oldest`, archived the two redundant extracted observations, and left the oldest knowledge Memory active. All three fixtures also verified exact publication/lifecycle replay as a no-op and reopened with the same Graph/lifecycle state.

This controlled write fixture validates temporal-only retrieval, accepted Graph publication, duplicate-chain insertion/rewiring, supersession projection, lifecycle completion, replay idempotency, and reopen recovery with the live Dream model route. It is still not a population-level semantic-quality estimate or a canonical-promotion benchmark. No case in either Ox validation exposed a need for broad reverse/bounded reconsideration machinery; reconsideration remains deferred until a concrete failure demonstrates it.

### Same-owner REL/PHY Dream validation — 2026-08-27

The frozen `corpus/insomnia-tuning-v1.jsonl` fixture (11 isolated Episodes / 314 turns) was re-imported into a fresh Project REL and routed through production Insomnia with a fresh Phylactery attached. Import produced 314 nodes, 11 canonical branches, and 52 fragments. Routed Insomnia completed all 11 Episodes with zero retries/terminal failures, publishing 41 Project Memories and 8 User Memories; all 49 received Memory-vector bindings in their respective owners.

The stale configured Nous `stealth/ox-alpha` Dream route returned HTTP 404 because the model is no longer available. The machine-local Dream route was changed to the already-tuned Codex `gpt-5.6-luna` low-reasoning route and the untouched owner fixtures were rerun. Project REL Dream processed 40/41 sources on the first pass and persisted 121 active REL-local relations; one classifier response violated the strict relation/direction contract, correctly left that source `extracted`, and completed on the next retry, leaving zero extracted Project Memories. User PHY Dream processed all 8 sources with zero failures and reopened at five `knowledge`, three `canonical`, zero `extracted`, zero archived, and 16 active PHY-local relations.

This validates the production owner split rather than cross-owner semantics: candidate discovery, Graph publication, duplicate/supersession handling, and lifecycle projection remained inside the selected REL or PHY. PHY used persisted `source_time_ns` without Archive access. No REL↔PHY candidate comparison or cross-file Graph edge was created. See ADR 0024.

### Dream population tuning harness — 2026-08-25

`examples/dream_tune/` runs the production `DreamProcessor::process_memory` path against a disposable typed REL rather than emitting detached semantic artifacts. The current canonical source fixture is the legacy `target/insomnia-tuning-v1-check.cva`, containing 46 production Insomnia Memories and 46 Memory-vector bindings. When that legacy fixture is supplied, the harness now invokes the normal `migrate_file` seam first and writes `dream-run.prj.rel`; current typed REL inputs are copied with their `.org.rel`, `.prj.rel`, or `.con.rel` scope suffix preserved. Dream therefore never performs new experiments directly against a legacy CVA. The harness enumerates active `extracted` Memories through the public `Cva::memory_ids` read seam, captures the exact pre-call candidate set and lane ranks, records classifier/verifier/publication/lifecycle results, syncs, reopens the mutated REL, and serializes final Memory/Graph/vector state to `report.json`. The source fixture itself is never mutated.

The harness supports `--source <MemoryId>` for one-case tuning, `--candidate-limit <N>` for candidate-fusion experiments, `--verification default|broad`, and `--retrieval-only` for cost-free candidate-ranking work without classifier calls or Graph mutation. A generated `gold-template.json` provides the complete baseline Memory inventory for labeling. The tracked population contract is `corpus/dream-gold-v1-tuning-v1.json`; it currently contains eight high-confidence related pairs, six positive final-pair expectations, four unrelated final-pair negatives, and a full-run requirement that the extracted backlog drain to zero. Exact relation kind/direction remains owned by the existing synthetic Dream contract until independently labeled in the population fixture.

`tools/evaluate_dream_gold.py` scores retrieval, observed classification, verifier verdicts, publication outcomes, exact final relations, unordered final related-pair presence, lifecycle expectations, and full-run backlog completion separately. Partial `--limit`/`--source` runs are scoped to attempted Memories, and a retrieval miss is not double-counted as a classifier miss. Retrieval-only runs score only retrieval-stage contracts.

Smoke validation over the durable 46-Memory fixture confirmed that a one-Memory production Dream run mutates only the disposable CVA, survives reopen, advances the selected source out of `extracted`, and persists Graph state. A targeted repository-location case missed its paired `No active work on D:` Memory at candidate limit 4 but retrieved it at limit 12, demonstrating that the harness exposes candidate-ranking thresholds independently of classifier behavior.

The first complete population run used Codex `gpt-5.6-sol` at low reasoning, candidate limit 12, and 12-way pair-inference concurrency while preserving serial Graph publication. It processed all 46 source Memories in 4m12s with zero recoverable failures, drained the extracted backlog to zero, and reopened with 184 directed Graph relations representing 103 unique unordered related pairs. The original small population gold scored retrieval 8/8, observed related classification 7/7, final-pair behavior 9/10, and backlog completion 1/1. The sole labeled miss was `Troubleshooting pace` versus `Step-by-step guidance preference`: retrieval succeeded, the classifier proposed `duplicate_of`, and the verifier rejected that proposal, leaving no relation.

A manual audit then reviewed all 103 unique durable pairs using a stricter graph-utility standard: broad same-project/domain overlap is insufficient unless traversing between the two Memories is semantically useful. `tools/export_dream_web_pairs.py` deterministically exports the unique durable pair inventory from any `dream_tune` report; `tools/build_dream_web_gold.py` applies the reviewed v1 judgments. `corpus/dream-web-audit-v1.json` records every pair as `keep`, `remove`, or `review`. The audit found 87 clear keeps, 10 clear removals, and 6 borderline broad-context pairs intentionally withheld from benchmark truth. `corpus/dream-web-gold-v1.json` converts the 97 high-confidence audited pairs plus the known missing troubleshooting relation into a presence/absence contract. The classifier-v1 Sol-low run scores 87/98 = 88.8% on that contract, with all 10 clear false-positive edges still present plus the one missing relation. The six borderline pairs are excluded rather than forced into gold. Nine additional topical edges are flagged in the audit as strong candidates for a more specific factual relation, but exact population kind/direction remains unscored until those cases are independently labeled; the existing synthetic Dream contract remains authoritative for relation vocabulary/direction mechanics.

Classifier tuning exposed and fixed a reporting defect before further semantic conclusions were accepted: `dream_tune` previously serialized `classification.b` as `candidate_id`, which is wrong whenever the processed source is canonical pair B. Final durable Graph scoring and the 103-pair audit were unaffected because they read reopened Graph state directly, but per-call/source attribution was incorrect. The reporter now derives the candidate as the non-source side of the canonical pair, with tests covering both canonical orientations.

The first classifier-v2 prompt deliberately tightened graph utility and proved far too conservative at population scale. Targeted probes removed all 10 known false positives and changed the missing troubleshooting/step-by-step pair from rejected `duplicate_of` to a published `factual` relation, but the full run produced only 69/98 = 70.4% whole-web agreement and 91 directed relations; the final source then hit the Codex Plus usage cap and remained `extracted`. None of the 29 positive gold regressions involved that failed source: every one was actually evaluated by v2 and explicitly classified `none`. The experiment was therefore rejected on semantic grounds, not provider failure, and source was restored to the measured classifier-v1 baseline.

`corpus/dream-classifier-regressions-v1.json` now freezes the discovered decision boundary as 40 classifier cases: 30 related and 10 unrelated. The positive side groups same-workstream relations such as packet policy/implementation, multiplayer-flow state, workspace operations, ship runtime/identity, asteroid lifecycle, network protocol, status summary/detail, conceptual analogy, and general-to-scoped interaction preferences. The negative side groups cross-project history leakage, cross-subsystem same-project links, and lexical collisions across different entities. Seven weaker positive judgments are marked medium-confidence so future prompt work does not silently overfit them. `tools/build_dream_classifier_regressions.py` reconstructs the fixture from the durable v1/v2 reports and tracked web audit; `tools/evaluate_dream_classifier_regressions.py` scores future population reports by expected relatedness, confidence, and pattern. On this boundary fixture, classifier v1 scores 29/30 related but 0/10 unrelated (29/40 overall); the rejected v2 scores 1/30 related and 10/10 unrelated (11/40 overall). The next prompt must preserve same-workstream recall while blocking only cross-workstream/project-context leakage.

`examples/dream_classifier_tune/` is the cheap prompt-tuning gate for that fixture. It opens the durable baseline CVA read-only, obtains each Memory through `Cva::dream_memory_context` (the same context builder used by production candidate retrieval), and calls `DreamClassifier::classify_pair` directly for the 40 explicit pairs. It bypasses retrieval, Graph publication, verification, and lifecycle mutation, runs pairs with configurable bounded concurrency (default 12), and emits per-case relation/direction/evidence plus related/unrelated accuracy by pattern. The harness now accepts `--system-prompt-file PATH`; that path constructs `DreamClassifier::with_system_prompt` for the experiment while ordinary production call sites use `DreamClassifier::new` and the current production classifier contract. This reduces a prompt iteration from the 46-Memory population run's roughly 552 candidate classifications to exactly 40 classifier calls without temporarily promoting an unmeasured prompt.

```text
cargo run --example dream_classifier_tune -- <config> <baseline.cva> corpus/dream-classifier-regressions-v1.json <report.json> --concurrency 12 --system-prompt-file corpus/dream-classifier-v2-candidate.txt
```

The frozen v2f candidate uses semantic workstream/scope as the boundary, adds explicit bounded-session/lifecycle handling, and tightens verbatim evidence-copy instructions. Three independent live `gpt-5.6-luna` low-reasoning runs scored `37/40`, `39/40`, and `38/40`; related recall was `27/30`, `29/30`, and `28/30`, unrelated accuracy was `10/10` in every run, and all three had zero output errors. The sole persistent miss was one medium-confidence `ship_runtime_identity` judgment; remaining misses moved between runs. The candidate therefore clears the exact-pair advancement gate three times consecutively and is frozen in `corpus/dream-classifier-v2-candidate.txt`. The subsequent Luna-low population run completed 46/46 sources with zero failures/backlog, scored 96/98 = 98.0% whole-web agreement, removed all 10 audited v1 false positives, restored troubleshooting/step-by-step, and reopened with 97 unique unordered pairs / 144 directed relations. v2f is production classifier contract v2.

### Live Insomnia worker concurrency — 2026-08-15

The prepared 12-conversation ChatGPT corpus was converted to the current canonical graph-JSONL development format without adding a production importer. It produced 1,441 canonical text nodes and 66 deterministic import Episodes. Each concurrency point used a fresh copy of the same pre-Insomnia CVA, the configured `gpt-5.6-luna` Codex route at `low` reasoning, and the configured Qwen3 embedding route. Episode-processing time and Memory-Vector time were measured separately so embedding latency did not contaminate the worker scaling curve.

```text
workers   processing     episodes/s   speedup vs 1   retries   terminals
1         456.578 s       0.145         1.00x         0         0
2         240.614 s       0.274         1.90x         0         0
4         108.906 s       0.606         4.19x         0         0
8          79.076 s       0.835         5.77x         0         0
16         46.561 s       1.417         9.81x         0         0
32         29.078 s       2.270        15.70x         0         0
40         28.605 s       2.307        15.96x         0         0
44         30.530 s       2.162        14.95x         0         0
48         21.356 s       3.091        21.38x         0         0
48 repeat  21.162 s       3.119        21.58x         0         0
52         32.516 s       2.030        14.04x         0         0
56         23.527 s       2.805        19.41x         0         0
64         24.579 s       2.685        18.58x         0         0
```

The measured sweet spot was 48 workers for this provider/model/corpus under extractor contract `v2-2`, and the default was changed from 16 to 48 while retaining the 1–64 configurable range. Bracketing runs at 44 and 52 workers took 30.530 s and 32.516 s, while 56 and 64 workers also trailed the repeated 48-worker result. No point produced retries or terminal failures, indicating a latency/contention optimum rather than a hard rate-limit boundary. Contract `v2-2` was subsequently found to contain a shortened semantic extraction prompt rather than the tuned legacy contract; contract `v2-3` restores the legacy policy with only the structural v2 field/evidence mappings. The 48-worker default remains in place provisionally until a focused 32/48/64 recheck confirms that the longer restored prompt does not move the optimum.

The `v2-2` sweep produced 133–157 created Memories and 9–30 rejected candidates from the same 66 Episodes. Every run completed all Episodes, and none requested historical evidence. Those counts must not be treated as evidence about the stability or quality of the legacy extraction policy because the sweep used the shortened `v2-2` prompt. Memory-Vector filling remains a distinct derived phase and is not used in worker-concurrency timing.

Five fresh 48-worker runs under restored contract `v2-3` produced 111, 108, 124, 122, and 116 Memories respectively, with 8–16 rejected candidates, zero retries, zero terminal failures, and zero historical-evidence turns in every run. Audit of the stored Memory bodies found that consolidation and phrasing were generally strong when the same source was selected, but source-selection overlap remained only about 0.60–0.67 pairwise. The audit also exposed cases where vague/question turns were treated as authority without explicit assistant-content provenance, plus residual execution/progress memories and unstable category/type labels.

Extractor contract `v2-4` added a required structured `authority_kind` field with `direct`, `correction`, `adoption`, and `retention` values. Contract `v2-5` added the first deterministic authority-policy layer: direct authority cannot carry assistant content provenance, adoption requires it, bare/callback retention and low-information deictic authority cannot support detailed memories without provenance, unsupported interrogative authority is rejected while explicit asserted tag-questions remain admissible, and obvious prompt/phase/test/commit-style execution receipts are rejected. Contract `v2-6` additionally rejects any candidate that still carries numbered prompt/phase/step progress or checkpoint framing, even when the candidate also contains useful project state; the extractor must emit the durable state without the numbered progress reference. Contract `v2-7` splits contextual provenance into assistant-authored **authority sources** versus **grounding sources** used only to resolve referents in user-owned statements/corrections. The existing persisted `content_source_*` fields now mean authority provenance only; new persisted `grounding_source_*` fields carry referent grounding. These checks remain deliberately narrow and do not claim general semantic entailment.

### Insomnia semantic gold set — 2026-08-15

`corpus/insomnia-gold-v1.json` is retained as the historical 40-case contract used for the recorded v2-3/v2-5/v2-6 comparisons below. `corpus/insomnia-gold-v2.json` is retained as the provenance-split 40-case contract introduced with extractor v2-7. `corpus/insomnia-gold-v3.json` is the current contract. It keeps 32 durable-state cases and 8 omit cases, but separates the original benchmark anchor from the user-authoritative source or sources allowed to satisfy current semantic-state coverage. Each case now records `anchor_disposition`, `state_retain`, and `coverage_sources`; alternate sources may override provenance/metadata/guard expectations when their authority form differs from the original anchor.

Gold v3 corrects the stale player-color target: the earlier `modulate` decision is marked `superseded`, and current-state coverage comes from the later user-authoritative shader-hue decision in the same Episode. Packet migration and ShipStats also enumerate later valid user-authoritative sources that may satisfy state coverage without pretending they are the original benchmark anchor. The four v2 grounding distinctions remain intact: creatureServer mothballing, BrowserOS existing-MCP, ShipStats-now, and deleted assets use grounding only to resolve referents and never to authorize additional semantic detail.

`tools/evaluate_insomnia_gold.py` now defaults to gold v3 and reports exact **anchor fidelity**, durable **state coverage**, **omit cleanliness**, assistant-authority provenance, grounding provenance, metadata, and content/receipt guards separately. Coverage is source-based and may use only explicitly enumerated valid authority nodes; where one user turn emits multiple unrelated Memories, a narrow coverage selector prevents the sibling Memory from satisfying the wrong case. Provenance or metadata failure does not erase state coverage: for example, a BrowserOS Memory may cover the intended state while still failing required grounding. Semantic equivalence to each case's `semantic_target` remains a human/model judgment rather than lexical overlap. Historical percentages below remain gold-v1 measurements and are not silently recomputed under v2 or v3.

Retrospective scoring of the existing five-run audits gives:

```text
contract   selection   provenance   metadata   content/receipt guards
v2-3         64.5%       95.5%       82.1%             92.9%
v2-5         67.0%       98.0%       92.2%             96.1%
```

The `v2-5` per-run selection scores were 57.5%, 70.0%, 67.5%, 62.5%, and 77.5%. Selection remains the dominant measured failure mode. The high provenance percentage is conditional on a gold-retain source actually being selected; it does not mean semantic support is generally validated. Gold v1 treated `ok, so we do want to add it now` as adoption; the later account/corpus audit showed that judgment was wrong and gold v2 corrects it to user authority plus referent grounding.

A controlled `v2-6` comparison used the same clean zero-Memory base CVA, 66 Episodes, 48 workers, embedding route, batching, and contract for three fresh Luna-low runs and three Luna-medium runs. Reasoning effort was the intended variable, and the configured route was restored to low afterward.

```text
reasoning   wall times (s)          avg wall   Memories       rejected       evidence turns   selection   provenance   metadata   guards   source Jaccard   strict semantic target
low         37.766 49.234 39.373     42.12     102 110 110     16 10 13       22 0 8           63.3%       98.2%        87.5%      100%     0.536            60.0%
medium      73.377 74.949 65.444     71.26     129 132 126     17 15 22       36 8 22          65.0%       98.4%        83.6%      100%     0.656            60.0%
```

`source Jaccard` is the mean pairwise overlap of all source nodes selected within the three runs at that reasoning level. `strict semantic target` is an explicit manual/model review against the 40 tracked semantic targets, counting omissions, incomplete targets, unsupported authority/provenance, and false-positive Memories as failures; it is intentionally stricter than the mechanical source-selection metric.

Medium reasoning made source selection more repeatable and increased raw Memory production, but did not improve aggregate semantic-target agreement. It gained only 1.7 mechanical selection points, reduced acceptable metadata agreement, took about 69% longer wall-clock time, and produced about 20% more Memories. Stable failures were largely unchanged: future ship variants, player-color identity, the narrow current custom-room-code fact, narrow server-authoritative collision state, the contextual deleted-assets correction, and calibrated prompt sizing were missed in all six runs, while the Godot-first question was incorrectly converted into a decision in all six. ShipStats was selected once at each reasoning level but still lacked the required assistant-content provenance. Medium also introduced one Prompt-41 receipt-derived Memory and one `fix it?` execution-request Memory that low omitted correctly.

The result does not support spending more reasoning budget on Insomnia extraction. Luna-low remains the selected bring-up default. The remaining quality problem is semantic selection/support policy: additional reasoning stabilizes a broader choice of sources, but does not make that choice materially more correct. Contract `v2-6` did raise the tracked explicit forbidden-content/numbered-receipt guard score to 100% in all six runs, but general semantic authority, entailment, and non-numbered execution-state handling still require a stronger validation/review mechanism rather than more lexical heuristics. The comparison is only three runs per level, so small numeric differences should not be treated as precise population estimates.

### Gold-set account-context audit — 2026-08-15

`corpus/insomnia-gold-account-context-audit-v1.json` audits all 40 gold cases against the account-level context retrieval surface available to the assistant and against the original local conversation neighborhoods in the prepared corpus. This is not a dump of platform-internal memory objects; it distinguishes surfaced user facts/preferences/constraints from recoverable historical conversation/file context.

Of the 32 retained cases, 19 have strong independently recoverable account support, 9 have only partial/related account support, and 4 have no independently recovered copy of the exact target. Those counts do **not** imply that account-wide retrieval should participate in authority. Several strong external matches contain broader or later state that would incorrectly expand the current user's proposition if treated as semantic authority.

The original audit focused on the 9 gold-v1 cases marked as requiring assistant provenance and found nearby conversation context for all of them. Re-review under the provenance split identified four cases where context is **grounding rather than authority**: creatureServer mothballing, BrowserOS existing-MCP, ShipStats-now, and deleted assets. The corpus still does not demonstrate a need for vector search as the first semantic-support mechanism. Deterministic current-Episode/nearby ancestry should be exhausted first; vector retrieval remains a fallback for genuinely distant unresolved references.

Contract `v2-7` implements that correction. Candidate structured output now has separate `authority_source_*` and `grounding_source_*` tuples. Authority sources must be earlier assistant turns explicitly adopted/retained by the user. Grounding sources may be earlier user or assistant turns but can only identify a referent; they cannot authorize semantic details absent from the user source turn. Both source kinds must precede the user authority and must be inside the authoritative Episode or returned by the bounded evidence round. That work originally bumped Memory records to `CVAMEMR2` for grounding provenance. The later canonical-policy milestone writes `CVAMEMR3` to persist `authority_kind`; R2 records remain readable and reopen with `authority_kind = unknown` rather than inferred authority.

The negative cases reinforce the same rule. Related history exists for several rejected questions/requests, but retrieval must not convert `so... fix it?`, the local-spawner question, or the Godot-first question into durable decisions. Likewise, account-wide collision and custom-room-code context can contain broader/later project state; similarity alone must never make that state authoritative for the current turn.

### Candidate-level semantic-review experiment — 2026-08-16

A disposable second-pass semantic reviewer was implemented and live-tested, then removed rather than retained in the runtime. The reviewer received only each extracted candidate plus its authoritative user turn and permitted authority/grounding source. An initial `supported` / `narrow` / `unsupported` contract over-narrowed 57 of 113 reviewed candidates, so a second contract separated `authority_valid`, `durable`, and `full` / `partial` / `none` semantic support and prohibited stylistic compression.

The second contract was compared over three fresh Luna-low / 48-worker runs against three fresh v2-7 controls on the same current-format 66-Episode fixture:

```text
mode       Memories/run       avg Memories   retain-source recall   omit-source clean   avg mechanical selection
control    103 / 97 / 97         99.0             65.6%                79.2%                  68.3%
review     86 / 74 / 79          79.7             57.3%                87.5%                  63.3%
```

The reviewer improved precision on the tracked omit cases: the Godot-first interrogative false decision disappeared in all review runs and the Prompt-41 receipt false positive seen in one control run also disappeared. However, `phase-renumbering` remained a false positive in all three control and all three review runs. More importantly, the blanket review removed too much useful state: retain-source recall fell by 8.3 points and average mechanical gold selection fell by 5.0 points.

The provenance breakdown rules out using the same reviewer only because contextual provenance exists. Gold cases requiring assistant authority were selected at 81.0% in both control and review runs. Grounding-required cases fell from 33.3% to 25.0%, and ordinary retained cases fell from 66.7% to 55.6%. A candidate-only reviewer also cannot recover a durable source turn the extractor never selected.

A direct Episode-boundary inspection confirmed that the required grounding material for all four gold-v2 grounding cases is already present inside the same authoritative Episode: creatureServer, BrowserOS, ShipStats, and deleted assets do not require broader ancestry or vector retrieval to resolve their tracked referents. The remaining measured problem is therefore **initial semantic source selection inside an already-sufficient Episode**, followed by faithful synthesis from selected authority—not missing retrieval context.

The experimental reviewer code and calibration-only CLI hooks were removed after measurement. The next quality experiment should separate **per-user-turn retain/omit source selection** from Memory synthesis so every potential authority turn receives an explicit disposition before consolidation. Broader retrieval, including vectors, remains a fallback for future cases that actually lack sufficient Episode-local grounding.

### Sol-low semantic-review follow-up — 2026-08-16

The same v2-7 66-Episode / 48-worker fixture and second-pass reviewer contract were then retested with `gpt-5.6-sol` at low reasoning in a detached disposable worktree. Three fresh Sol controls were compared with three fresh Sol runs using the reviewer; the main runtime and configured Luna route were not changed.

```text
mode       Memories/run        avg Memories   retain-source recall   omit-source clean   avg mechanical selection   source Jaccard
control    154 / 148 / 158        153.3             85.4%                62.5%                  80.8%               0.695
review     122 / 121 / 126        123.0             71.9%                87.5%                  75.0%               0.638
```

Sol materially changed the first-pass extraction result. Compared with the earlier Luna-low v2-7 controls, Sol controls raised retain-source recall from `65.6%` to `85.4%` and average mechanical selection from `68.3%` to `80.8%`, while producing substantially more raw Memories. A direct body audit of the first Sol pair also recovered several targets that were stable Luna misses: future ship variants, the narrow custom-room-code current fact, deleted-assets grounding, and calibrated prompt sizing. Sol still consistently missed player-color identity, the narrow collision-authority target, and the mixed Prompt-72 state in the observed runs.

The blanket reviewer still did not justify itself. It removed the Godot-first false decision and Prompt-41 receipt in all three review runs and improved omit cleanliness from `62.5%` to `87.5%`, but it reduced retain recall by 13.5 points, lowered average mechanical selection by 5.8 points, and made source selection less repeatable. It also still admitted phase-renumbering in two runs and one finish-job request in one run. The three Sol controls took `50.846`, `71.347`, and `63.431` seconds end to end; review runs took `73.842`, `88.115`, and `66.900` seconds, about 23% slower on average.

The useful conclusion is therefore **model choice before architectural expansion**. Sol-low is now the stronger extractor candidate and should receive a full strict semantic-target review before replacing Luna-low as the configured default. If a second pass is retained at all, the next experiment should gate semantic review only to structurally ambiguous/high-risk authority candidates rather than reviewing every Memory. This may preserve Sol's high recall while filtering question/request/receipt false positives. No result here establishes a need for vector retrieval.

### Strict Sol-low semantic-target audit — 2026-08-16

The three-run Sol-low comparison was regenerated in a detached audit worktree so the actual Memory bodies could be judged rather than relying on source-node selection alone. The fresh controls created `147`, `150`, and `149` Memories from the same 66-Episode v2-7 fixture, with zero retries or terminal failures. Exact-anchor mechanical selection was `72.5%`, `75.0%`, and `85.0%` (77.5% mean); the difference from the earlier 80.8% three-run sample confirms that source selection remains stochastic even with Sol.

`corpus/insomnia-sol-low-semantic-audit-v1.csv` records the 40-case body-level audit. A run counts as semantically acceptable there only when the target is faithfully retained, correctly omitted, or legitimately represented by a different/later user-authoritative Memory; incomplete targets, unsupported expansion, provenance misuse, and false durable state fail. Under that stricter rule the three runs were `30/40`, `29/40`, and `32/40` acceptable (`75.8%` mean). This is not directly comparable to the historical v2-6 60% strict score because the current audit uses corrected gold-v2 semantics and additionally recognizes valid alternate/superseding authority.

The audit also exposed an evaluator/gold distinction that the exact-anchor metric cannot represent. Packet migration and ShipStats were mechanically missing in some runs while the same durable state was retained from another valid later user-authoritative turn. More importantly, the player-color gold target freezes the earlier `modulate` implementation even though a later user turn in the same Episode explicitly supersedes it with shader hue values while preserving the durable local-default/remote-distinct identity rule. R1/R2 correctly preferred that later state. Future gold should therefore separate **authority-anchor fidelity** from **current semantic-state coverage** and allow explicit supersession/alternate-authority relationships rather than treating every non-gold anchor as semantic loss.

The remaining stable failures are much more diagnostic than the headline score:

- `adopt-c-family-adapter` is missed in all three runs despite an explicit `Fuckit, add that` immediately after the shared C/C++ adapter proposal. This is a true adoption-selection failure, not missing context.
- `collision-authority-narrow` is missed in all three runs. The user asserts that collision has to stay server-authoritative and adds a confirmation tag; the extractor's strong anti-question instruction is suppressing an asserted proposition before the deterministic tag-question policy can see it.
- `prompt-41-receipt` is falsely retained in all three runs. `completed through 41` is being combined with the contextual contents of Prompt 41 to manufacture an implementation fact. The current receipt wording permits retaining a resulting state after completion and is therefore too permissive about *implied* results.
- `prompt-72-mixed-status` is missed in all three runs. The same anti-receipt policy suppresses the whole turn instead of discarding only the numbered checkpoint clause and retaining the independently asserted current state (`generally working`, with visual bugs/oddities remaining).
- Provenance remains semantically leaky in a smaller set of cases: BrowserOS can be named without grounding, and prompt-sizing Memories sometimes use assistant context to add packet-migration-specific framing that the user's correction did not need.

These failures point to task decomposition rather than retrieval. The necessary evidence is already Episode-local, and the 64-candidate ceiling is not binding. The current single pass asks one model simultaneously to select durable user propositions, classify authority, distinguish questions/receipts, resolve referents, preserve lifecycle/modality, consolidate related claims, write standalone Memories, and attach provenance. Sol handles that bundle better than Luna, but the stable boundary failures remain.

The next quality experiment should therefore replace free-form `Episode -> Memories` selection with an explicit **clause-level authority/disposition ledger** before Memory synthesis. Every user turn should receive an explicit disposition; mixed turns may produce both omitted and retained clauses. The selector should record exact source span, retain/omit, authority kind, lifecycle/modality, and whether assistant authority or referent grounding is required. Deterministic validation can then require complete turn accounting and prevent a receipt from authorizing the contents of a completed task. A second synthesis step should consolidate only already-authorized propositions while preserving modality and provenance. Sol-low is the current best candidate for the hard selection pass; a smaller model can be tested for synthesis afterward because synthesis quality has generally been strong once the correct source is selected.

Gold/evaluator v3 now implements that correction. On the same three preserved Sol-low controls, exact anchor fidelity is `75.0%`, `77.5%`, and `87.5%` (`80.0%` aggregate), while explicitly allowed state-source coverage is `78.1%`, `81.2%`, and `84.4%` (`81.2%` aggregate). Omit cleanliness is `87.5%` in all three because Prompt 41 remains the one stable false-positive omit case. The split now makes packet-migration/ShipStats alternate authority and the superseding player-color hue decision visible without erasing provenance defects such as BrowserOS R2's missing grounding. These are mechanical source/contract metrics only; strict semantic-target equivalence remains separately judged. No result in this audit supports adding vector retrieval; vectors cannot fix propositions that are already present in the Episode but misclassified or skipped.

### Insomnia routine tuning corpus — 2026-08-16

Routine semantic tuning no longer uses the full 12-conversation / 66-Episode fixture. Gold v3 spans 28 distinct source Episodes, which is still too expensive for repeated frontier-model iteration. `corpus/insomnia-tuning-v1.jsonl` is the deliberately adversarial core fixture: 11 isolated Episodes, 314 turns, and 19 gold-v3 cases (`15` retained-state cases and `4` omit cases). `corpus/insomnia-gold-v3-tuning-v1.json` is the matching evaluator subset. Source-node IDs and source content are preserved verbatim; fixture-local conversation/Episode IDs isolate each selected path cleanly.

The selected cases cover the failure modes that have actually driven Insomnia changes: short/deictic adoption, correction plus grounding, unsupported contextual elaboration, asserted tag questions versus pure questions, pure execution receipts, mixed receipt/current-state turns, transient phase metadata, future modality, supersession, over-atomization, long governing architecture with trailing implementation commentary, and simple direct-state sanity. One redundant C-family precision-adoption case was intentionally dropped because equivalent adoption behavior is already exercised by two C-family cases plus the separate `alright, that works` case.

Historically, for the two-pass extractor this reduced the nominal no-evidence model-call floor from `66 * 2 = 132` calls to `11 * 2 = 22`, about an `83%` reduction. The final three-model-pass tuning harness has a nominal no-evidence floor of 33 calls on the same fixture. Routine tuning is now frozen: keep the 11-Episode corpus as a regression fixture, and use full gold v3 / the 66-Episode corpus only for milestone confirmation rather than renewed benchmark-specific optimization.

### Atomic self-cleaning Insomnia completion — 2026-08-16

Successful Episode processing no longer publishes local REL Memory revisions or their backing objects one at a time. The processor validates extraction and prepares new content-addressed local Memory bodies, Memory records, body-bound routing metadata, and contiguous REL-global version assignments entirely in memory, then writes one current `CVAINSC6` completion transaction containing the new local bodies, every newly visible local Memory record, the owned global-version range, owner-qualified external Memory references, the corresponding new Project-Memory routing metadata, the accepted Freshness birth turn/policy/exact first-revision cohort, and the compact successful Episode receipt. The same chunk is consumed by REL Memory/routing rebuild and Insomnia completion state. `CVAINSC1` through `CVAINSC5` remain readable for compatibility; V4 carries transaction time but predates embedded routing metadata.

Container reopen treats an incomplete final length-prefixed chunk as an interrupted append: it truncates the file back to the start of that trailing chunk and continues from the last complete boundary. The existing deterministic truncation suite continues to cover the atomic local Memory transaction through the version-compatible completion decoder, while current writes are `CVAINSC6`. Routed ownership adds a second crash boundary: a PHY User Memory and its routing attachment are synced before the REL receipt, and deterministic mutation/body identity reuses that already-durable PHY state rather than duplicating or rewriting it. A separate regression runs 1,000 synthetic processing failures before success and verifies exactly zero REL growth before success and exactly one new completion chunk.

Operational retention is deliberately narrow. Pending queue state, claims, leases, renewals, retry counters, retry delays, and retryable failures exist only in memory and emit no `CVAINSW1` chunks. On reopen, every Episode without a final success or terminal outcome is derived as Pending with a fresh runtime attempt count. Successful processing retains one compact `CVAINSC6` REL transaction containing the producing model/contract, timestamps, local Memory IDs, owner-qualified external Memory references, rejected count, newly required local Memory bodies/records, body-grounded routing metadata, and their REL-global version range. Terminal outcomes still use one durable final `CVAINSW1` record during this compatibility slice. Detailed extraction responses, evidence bundles, classification/wording/enrichment intermediates, and retry traces are not made durable.

This removes normal Insomnia garbage at the source rather than depending on vacuum. Queue/claim/renew/retry transitions add zero CVA bytes, failed extraction or synthesis adds zero CVA bytes, and once an Episode already exists a successful processing pass adds only its single completion transaction. Older development CVAs can still contain superseded `CVAINSW1` records and legacy staged-body/`CVAINSC1` layouts, which remain readable as compatibility history. Current Insomnia processing has no pre-completion persistent staging seam and should not require a general CVA vacuum for normal operation.

### Synthetic Insomnia mechanical scaling — 2026-08-17

`examples/insomnia_stress.rs` isolates storage, scheduling, application, and Memory-Vector publication from model latency. Each synthetic conversation contains one durable user fact, one materialized Archive fragment, one finalized import Episode, one directly authorized Memory, and one simulated 1024-dimensional Memory Vector. The normal `drain_insomnia_backlog` path is used, followed by sync, reopen, and count validation.

Release-mode results on the current development machine:

```text
episodes  workers  evidence rate  drain time   episodes/s   CVA bytes      reopen
5,000     1        0%              2.320 s      2,155.278    26,701,304     224 ms
5,000     48       0%              2.285 s      2,188.239    26,701,304     221 ms
20,000    48       0%              9.660 s      2,070.450   106,941,304   1,260 ms
5,000     48       1%              4.739 s      1,055.014    26,701,304     228 ms
5,000     48       10%            24.618 s        203.107    26,701,304     227 ms
```

The zero-latency endpoint intentionally exposes the serialized mutation ceiling: adding workers does not improve the approximately 2.1K Episode/s mechanical rate because model latency has been removed. Scaling from 5,000 to 20,000 Episodes with fragments and vectors remains close to linear, while the measured live `gpt-5.6-luna` 48-worker run is only about 3.1 Episodes/s. Normal Insomnia mechanics therefore have more than two orders of magnitude of headroom over current live inference.

The pre-index evidence cases exposed a scale-sensitive path: `archive_search` performed a full lexical fragment scan while holding the Container lock. At 5,000 fragments, forcing 1% of Episodes through that path roughly halved synthetic throughput; forcing 10% reduced it to about 203 Episodes/s. Those measurements are retained above as the baseline that motivated the shared lexical index.

#### Shared lexical-index rerun — 2026-08-19

Insomnia evidence now uses the same disposable derived lexical index as normal lexical/hybrid retrieval. The finite drain builds the index once before spawning workers; exact-turn/range planning and lexical ranking then occur against immutable Archive/index state without the Container lock, and only the bounded winning turn bodies are hydrated through the serialized Container handle. Lexical tokenization, scoring, and tie-breaking are unchanged, and the index remains non-persistent derived state with no CVA format change.

```text
episodes  workers  evidence rate  drain time   episodes/s
5,000     48       0%              2.399 s      2,083.786
5,000     48       1%              2.473 s      2,021.939
5,000     48       10%             2.441 s      2,048.242
20,000    48       10%            11.053 s      1,809.476
```

At 5,000 fragments and 10% historical-evidence usage, throughput improved from `203.107` to `2,048.242` Episodes/s, about a tenfold increase. At 20,000 fragments the same 10% evidence rate still sustained `1,809.476` Episodes/s. The former repeated full-Archive scan is therefore no longer the scaling bottleneck; the remaining evidence cost is bounded index-query work plus hydration of selected turn bodies.

### Insomnia semantic tuning closure — 2026-08-24

Routine semantic tuning is frozen on the 11-Episode adversarial fixture. The active experimental architecture is `Sol-low semantic ledger -> deterministic fixed groups -> Luna-low metadata -> Sol-low wording`. The separate metadata pass is classification-only and cannot alter semantic groups. In the controlled comparison it preserved all 51 frozen groups while raising metadata agreement from `71.4%` to `93.3%`.

The evaluator now scopes gold-v3 automatically to source turns actually present in the sibling `run.json`. This corrected a denominator error in earlier ad-hoc scoring: the trimmed fixture contains 19 evaluable anchor cases, 15 retained-state cases, and 4 omit cases, not all 40 gold-v3 sources. Correctly scoped controlled v7 results are `89.5%` Anchor / `93.3%` State / `100%` Omit. Final pass-1 changes add clause-level extraction for mixed question/request turns, preserve durable future/possible clauses, strip checkpoint framing from retained propositions, distinguish corroboration from supersession, and supersede an older mixed clause wholesale when a material mechanism/value/requirement is actually replaced.

Two fresh full runs around the finalized prompt produced `94.7%` Anchor, `93.3–100%` State, and `100%` Omit. Targeted reruns of every remaining edge (`future-ship-variants`, phase renumbering, Prompt-72 mixed status, ShipStats corroboration, genuine player-colour supersession, and workspace-repository location) all reached their expected semantic behavior. Remaining full-run misses move between cases and reproduce correctly in isolation, so further prompt squeezing is treated as benchmark overfitting against model sampling variance rather than useful tuning.

The hard selector ranking from this 2026-08-24 tuning closure remains `Sol-low > Ox Alpha-low >> Luna-low`. Since that measurement, authoritative extractor contract `v3-4` has ported the fixed-group metadata seam, added the separate bounded `user | project` durable-owner classifier before wording, and added a post-wording Entity-mention enrichment pass whose output is constrained to verbatim final-Memory text. The briefly implemented model-selected lexical-term output was removed after split-task calibration showed no architectural need for probabilistic lexical routing; REL/PHY Memories now use the same deterministic lexical indexing/scoring machinery as Archive search. These later routing/enrichment changes do not revise the historical semantic-selection model-quality measurements above. Further semantic reliability work should require a materially different capability boundary such as stronger/fine-tuned models, voting/verifier architecture, deterministic semantic preprocessing, or another separately justified structural stage.

Full experimental history, provider-specific compatibility work, caveats, and exact comparison tables are recorded in [Insomnia semantic validation — 2026-08-24](insomnia-semantic-validation-2026-08-24.md).

### Luna medium/high Insomnia follow-up — 2026-09-07

A focused all-Luna comparison reran the frozen 11-Episode / 314-turn tuning fixture with four workers, using Luna for semantic ledger, metadata, and wording at one consistent reasoning level per run. The measured `medium` run produced 63 Memories and scored `84.2%` Anchor / `86.7%` State / `75.0%` Omit / `100.0%` Authority / `81.8%` Grounding / `85.7%` Metadata / `100.0%` Guards. The measured `high` run produced 70 Memories and scored `89.5%` Anchor / `93.3%` State / `75.0%` Omit / `100.0%` Authority / `96.0%` Grounding / `81.2%` Metadata / `100.0%` Guards.

High recovered the medium run's `custom-room-code-current-fact` miss and substantially improved grounding, but both levels missed `adopt-c-calibration-corpus` and retained `phase-renumbering`. Observed wall times were approximately `208.5 s` medium and `485.6 s` high, but the runs overlapped and are not a clean isolated throughput benchmark. The historical low result is not a strict same-contract baseline; the direct result is that Luna-high outperformed Luna-medium for semantic selection/grounding on this fixture while costing substantially more latency and leaving omit cleanliness at `75%`.

The active `codex` credential was expired at test time, so the detached benchmark worktrees used the existing valid `codex-alt` credential without changing repository configuration. The comparison retained the historical deterministic missing-user-turn `omit` repair; the measured high run required it once. A resume attempt also exposed a benchmark-only defect: rebuilding frozen groups from already metadata-mutated saved ledgers can change group cardinality. The scored medium/high runs were restarted fresh with no resume state. Full failure lists and caveats are recorded in the semantic-validation document above.

### Benchmark baseline — 2026-08-15

This repository state is the benchmark baseline for subsequent retrieval-quality changes. The baseline restores the retrieval behavior used by the existing Long Memory Evaluation machinery: 8-turn / 2-overlap fragments, `qwen/qwen3-embedding-8b` at 1024 dimensions, exact cosine semantic retrieval, the original lexical scoring formula, 30-candidate lexical/semantic fusion with `0.45/0.55` weights, duplicate-range removal, overlap diversification, and top-10 output.

The historical LME score has not been rerun against this v2 implementation. The baseline claim is therefore that the benchmark-required machinery and retrieval semantics are restored and live-tested, not that v2 has independently reproduced the prior judged score. Changes after this point that can affect retrieval quality should be compared against this baseline explicitly.

A current-format cold-cache sample has not yet been recorded. Larger-population realistic-dimension vector runs require repeated cache eviction plus one open per sample before conclusions about cold scaling, segmentation, or checkpoints.

### Chronos detector calibration — 2026-09-16

The first indication-detector calibration used production `chronos::assess` over frozen real Memory corpora plus LongMemEval labelled temporal-reasoning answer sessions. The primary 28-day REL contains 1,396 Memories with authoritative source time. Before contextual guards, 244 Memories were marked temporal and 239 were unresolved; inspection showed dominant false-positive boundary/fuzzy traffic such as `through the audioflow seam`, `before the other changes`, `Match -> march`, `early -> yearly`, and uncontextualized four-digit dimensions. After calibration, the same REL has 24 temporal Memories, 22 unresolved Memories, zero fuzzy corrections, and unresolved residue consisting of 24 duration indications plus one calendar indication. The paired 28-day PHY has 124 Memories, 9 temporal Memories, 8 unresolved Memories, zero corrections, and only duration residue. The frozen 14-day REL/PHY control showed the same duration-dominated pattern, so the precision improvement was not specific to the later project vocabulary.

LongMemEval recall was checked only on the labelled temporal-reasoning population rather than all distractor turns: 133 cases, 288 answer-bearing source sessions, and 3,324 unique answer-session turns. Chronos detected temporal material in 283/288 answer sessions (98.3%). Tightening ambiguous calendar terms removed 105 unresolved calendar indications while preserving the same 283/288 session recall. The five sessions with no detected content-time material were cases whose temporal reasoning depends on source/session chronology plus approximate wording such as `recently`, `just completed`, `just helped`, or `just got back`; those are recorded as an uncertainty/approximate-valid-time coverage gap rather than forcing unverifiable detector output.

The calibration also exposed deterministic coverage gaps rather than detector-threshold problems: standalone clock times, approximate/compound durations, partial conversational calendar forms, and event-relative offsets. Fuzzy normalization remains deliberately bounded; numbered-list punctuation is now an adjacency barrier so `3. Weeds ...` cannot become `3 weeks`, while true adjacent typos such as `three mnoths` remain correctable. These measurements justify the current precision guards, not arbitrary grammar expansion.

## Failure modes

- `cargo fmt --check` catches Rust formatting drift.
- `cargo check` catches type/compile errors.
- Core `cargo test` owns focused library behavior, including config replacement, encrypted credential round trips/tamper rejection, switchboard auth binding, and master-key generation/reload/redaction.
- The separate CLI fmt/check/test commands verify that the detachable package compiles only against the public library surface.
- Command-level CLI smoke tests exercise CVA create/info/verify, graph import/archive inspection, simulated profile/vector/search execution, and config show/verify without installing the binary. Live `vectors probe/build` requires an external endpoint and credential and is not part of deterministic CI.
- The documentation checker validates repository documentation policy but not semantic correctness.
- `archive_roundtrip` fails on Archive reconstruction/count/content disagreement.
- `vector_generation_smoke` fails if profile separation, full-fragment generation building, current-generation reconstruction, row counts, or reopened default hybrid retrieval disagree.

## Related docs

- [Architecture](architecture.md)
- [Maintainer map](maintainer-map.md)
- [Documentation coverage](documentation-coverage.md)
- [Behavioral contracts](behavioral-contracts.md)
- [Repo-local CLI](cli.md)
- [Insomnia semantic validation — 2026-08-24](insomnia-semantic-validation-2026-08-24.md)
- [ADR 0007](decisions/0007-compatibility-profiles-and-vector-generations.md)
- [ADR 0009](decisions/0009-expandable-model-switchboard.md)
- [ADR 0010](decisions/0010-encrypted-credential-objects.md)
- [ADR 0011](decisions/0011-detachable-repo-local-cli.md)

## Notes

The current library has a synchronous per-turn ingestion primitive, transport-neutral interaction seam, derived conversation/leaf inventory, and exact-leaf transcript reads. Warlock v2 now consumes those APIs directly for workspace lifecycle and durable user-turn chat, but no long-lived inference/background host loop, full workspace/CVA management surface, ACP adapter, or production provider/session importer exists yet. Additional provider-native/local adapters and Codex OAuth token refresh are also absent; future sequencing is owned by [Roadmap](roadmap.md).