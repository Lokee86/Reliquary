# G4 — complete rebuilt product acceptance on generated REL/PHY 2.1 corpus

**Status:** implementation-ready specification; G4 **not started or accepted**.
**Execution repository:** `C:\!bin\workspace\Reliquary-rebuild` (`rebuild/clean-architecture`).
**Authority:** canonical [capability tracker](implementation-rebuild-capabilities-2026-11-03.json), [selective-reuse rebuild plan](../architecture-rebuild-implementation-plan-2026-11-03.md), [ADR 0049](../decisions/0049-parallel-architecture-rebuild-and-verified-cutover.md), [P01 public API specification](p01-public-product-api-cli-implementation-spec-2026-11-06.md), and accepted G1/G2/G3/GM/P01 evidence in the rebuild.
**Required predecessors:** P01 and GM complete (their own G3/G2/G1 dependencies remain authoritative).
**Companion contracts:** [G4 generated corpus and scenario matrix](g4-generated-2-1-corpus-acceptance-matrix-2026-11-06.md) and [G4 execution tickets](g4-product-acceptance-execution-plan-2026-11-06.md).

## 1. Problem statement

The rebuilt storage, semantic workflows, Gateway, maintenance, public Rust API and REL CLI passed their **individual** acceptance gates. They have not yet passed one independently generated, multi-owner, long-lived **product cohort** that exercises complete accepted behavior across those boundaries and persists/reopens the resulting state. A passing full workspace suite alone cannot establish that these independently accepted parts compose without data loss, privilege leaks, incorrect admission evidence, fake retries or incoherent history.

G4 proves the **native 2.1 replacement product works as a whole** before archived 2.0 files are converted. It is not an opportunity to copy the old facade, introduce compatibility readers or expand the product roadmap.

## 2. Target architecture and ownership

Keep the accepted call direction:

```text
External Rust consumer / REL CLI
          → public reliquary-memory (Rel, Phylactery, Gateway, Instance)
          → authenticated runtime Gateway / named Application workflows
          → RelSession / PhySession (single owner authority)
          → durable owners and REL/PHY 2.1 Container
```

Semantic evaluators (Insomnia, Dream, Entity Resolution, Retrieval, Freshness, Vectors, Context) operate on bounded owner-prepared evidence. Model/embedding/host and VCS work occurs outside owner mutation locks. Gateway owns principal/grant admission, revocation, host handoff, queues, streams and worker scheduling. The CLI depends **only on** the public product crate for domain operations. The test fixture harness is **not** a product subsystem.

### Test seams — exercise the final interfaces

1. **Consumer seam:** `reliquary_memory::{Rel,Phylactery,Gateway,TrustedHostAdmission,Instance,WorkspaceManagement}`, public maintenance/history operations, and `cli/` binary. Existing evidence: `tests/public_api.rs`, `cli/tests/cli.rs`, `tests/external-consumer`.
2. **Gateway integration seam:** `reliquary-runtime-gateway`'s authenticated instance and worker methods where P01 deliberately exposes no broad external provider/worker interface. Existing evidence: `src/{instance_conversation,instance_retrieval,instance_dream,instance_entity,instance_vectors,instance_context,owner_workers,workflow_workers}.rs` and their tests. **Do not turn private worker internals into public API just for G4.**
3. **Owner/Application proof seam:** current 2.1 `RelSession`/`PhySession`, selected Application workflows and independent owner reader/proof; existing GM/V01–V06 tests. Use owner reads for independent evidence only, never to inject records that a required product or Gateway scenario claims to have produced.
4. **Process seam:** a fresh CLI process and a standalone Rust consumer re-open the same test output without in-memory Gateway objects. The generator and verifier must not share mutable projections, fixtures fabricated after the fact, or source-owner leases.

**Structural rule:** the existing 23-package, 76-production-edge Cargo DAG remains canonical. No new permanent service, Root aggregate, store access path, empty forwarding crate, direct CLI-to-owner dependency, `Cva` aliases or 2.0 runtime parser. Amend the DAG only by an explicit architectural decision and independently rerun its checker.

## 3. Corpus contract

Generate a **new synthetic 2.1 corpus**, not a transformed historical fixture. The companion matrix fixes the seed, cohort, scenario content classes and expected effects. Baseline: **two REL owners and two PHY owners; 12 realistic multi-turn conversations across RELs** (eight/four), multiple forked branches, ≥40 accepted native turns, distinct principals, multiple bounded Episodes, project- and user-routed Memories, canonical/duplicate Dream candidates, Entity mentions, vector profiles, external project source evidence, retrieval receipts, host operations, tombstones and historical changes. All must be introduced through the **lowest supported final product/Gateway/Application admission**, never arbitrary raw record injection for a success case.

Prefer deterministic scripted external model, embedding and host test doubles **at existing real seams**. Freeze request/response data, timestamps, owner identity where the fixture constructor supports them, and scenario ordering. For the consumer `Rel::create`/PHY API that chooses random owner IDs, manifest checks compare the **captured identity** across reads/reopen rather than wrongly expecting a fixed hardcoded UUID. Deterministic identity golden vectors from F03 remain independent cross-checks, not fixture identities.

A versioned, source-controlled **scenario manifest** contains: seed, fixture schema version, conversation script IDs, branches/parentage, expected source references, allowed owner routing, expected active/inactive state, logical history cuts, retrieval acceptance cases, and adversarial disruptions. Generated owner files live in disposable test directories; no ignored `.test-tmp` artifacts become mandatory source files. Record manifest hash and optional output hashes in the G4 evidence report; do not commit potentially large mutable generated owner files or host secrets.

## 4. Implementation decisions

- **One canonical owner each:** native Archive user turns are distinct from host-only provisional transcript/checkpoint events. The same owner IDs and Memory body/source refs survive sync and independent reopen; PHY-private Memory never joins REL-only retrieval/context.
- **Actual workflow chaining:** accepted conversation → finalized Episode → validated Insomnia extraction → REL/PHY publication → Dream first-cycle and canonical/duplicate state → Entity resolution → vectors → bounded context/retrieval → confirmed external delivery/Freshness. No synthetic “pass” from directly pre-populating final store tables.
- **No model-dependent golden drift:** scripted fixed provider decisions may vary in *test inputs* (including rejection), not in the expected outcome. Verify exact selected IDs, citations, owner bindings, event receipts and history, not mere row counts.
- **Owner cut and history:** accepted-turn clocks, revision/body IDs, canonical revision stream and proof digests must be independently read from fresh sessions. Separate observable `missing`, `tombstoned`, `pruned/unavailable`, `unauthorized`, and `corrupt` cases.
- **Retained host semantics:** tested queue ordering, exclusive finalize/cancel win, submitter ownership, Held unknown host status, checkpoint-before-visibility, replay cursors, durable exact retry, revocation **at prepare and commit**, worker uniqueness/quiescence and no model call under owner locks.
- **GM on real G4 cohort:** snapshot the completed corpus, run permitted rewrite, terminal prune/profile repair/reclaim and same-owner REL reconcile on **copies** with physical no-replace publication, source immutability and independent reopen; compare preserved provenance, historical proofs, source IDs, Freshness and Dream/Insomnia work journals. Unsupported divergent state rejects with no output. Do not demand that every arbitrary divergent owner state is supported.
- **Provider, VCS and host boundaries:** use deterministic local mocks and ephemeral Git/Lore source repository only where already accepted workflows support them. Verify exact source bytes/revision, revocation and retries without requiring live external accounts or network providers.
- **Reference role:** the pinned 2.0 repository is an **oracle for retained semantic behavior**, not a linked fallback, production dependency or input file format. Native 2.1 whole-product acceptance is independent of D01–D03. Retain F03's six deterministic ID golden-vector assertions separately.
- **No 2.0 reader:** test that a 2.0-format negative file fails clearly without mutation; do not load or transform the six authoritative archived owners in G4.

## 5. Migration / integration strategy

This is **verification-led integration** of an already accepted clean rebuild; **no new architectural migration window** is authorized. Implement missing tests and narrowly fix genuine defects at their existing owner/workflow/Gateway/public boundary. Each correction requires a regression that demonstrates the original failure and proves the repaired externally observable behavior. Avoid wrappers, old-Root compatibility, duplicate owner projections and test-only production APIs.

Sequence: freeze expectations → generate corpus and reference-independent assertions → run full vertical flows → authorization/restart/adversarial tests → GM-on-corpus and CLI/process proof → independent acceptance. A regression of an accepted earlier gate blocks G4 and is corrected **without weakening the prior test or its acceptance contract**.

## 6. User/developer outcomes

A downstream consumer can create and reopen native REL/PHY owners, operate the supported authenticated product, receive only authorized/committed results, retrieve grounded Memory with confirmed-use accounting, and inspect maintenance and history after crash/reopen. Operators can inspect/verify and transform supported REL copies with the published CLI. Future engineering can use a small reproducible corpus and proof manifest as a **release gate**, without transporting old Root machinery.

## 7. Testing decisions and commands

Build on prior accepted suites rather than replacing them. Proposed test placement:
- `tests/g4_public_product.rs` for public-only owner, management and host flows; this integration test must import **only** `reliquary_memory`.
- `crates/reliquary-runtime-gateway/src/tests/` (or its existing test modules) for full authenticated worker/Insomnia/Dream/Entity/retrieval/host execution using already available test seams.
- `crates/reliquary-application`/owner-session tests only for any new correction with owner authority or GM integration.
- `cli/tests/g4_corpus.rs` for CLI invocation against a generated, independently reopened REL copy (not a no-op command smoke).
- `tests/external-consumer` for compilation and, where feasible, a separate process-based reopen/inspect.
- Minimal shared fixture functions under **test-only** modules with stable seed and expected manifest; no production fixture crate or permanent API.

Baseline command set from `Reliquary-rebuild` (Windows, serialized `target-codex-check` to limit disk pressure):

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo fmt --manifest-path cli/Cargo.toml --check
cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check
cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check
cargo check --manifest-path tests/external-consumer/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check
```

Run the **specific G4 scenario binaries** and process-reopen tests independently as described in the matrix. If a selected test needs loopback, provide only local loopback; no live provider request. Collect exit status and counts; treat ignored/filtered/replaced tests as **not executed** unless explicitly authorized by the scenario contract.

## 8. Completion gate

**G4 passes only if all are true:**

1. P01 and GM remain accepted; no regression or reclassification of their supported behaviors.
2. The versioned generated 2.1 manifest and its **independently authored assertions** cover every mandatory G4 matrix row; nontrivial state is created by accepted workflows rather than fixtures fabricated at the final store.
3. Canonical owner identities, exact current data, full historical proofs, accepted-use and routing receipts match after quiesce, process restart, independent open and deterministic repeat; no uncommitted host text becomes Archive.
4. REL/PHY and principal grant isolation, wrong-kind/formats, stale cut, body/provenance tampering, provider/host ambiguity, idempotent retry, destination collision and source immutability tests **fail closed** as specified.
5. The public crate, standalone CLI and independent consumer work on generated files; unsupported CLI commands fail nonzero.
6. All locked/offline workspace and CLI suites, external consumer compilation, formatting and production DAG checks pass. No permanent lower→Root dependency or legacy reader is introduced.
7. An **independent review** compares the actual product read results against the source-controlled scenario manifest and records both positive and negative outcomes in `Reliquary-rebuild/docs/g4-product-corpus-acceptance-2026-11-06.md`. Missing evidence is a blocker, not “deferred” by a green test count.
8. Tests and evidence are committed; working tree is clean at G4 acceptance; the canonical tracker is updated only at this point.

**G4 does not pass** merely because the 2.1 corpus is creatable, the workspace compiles, existing lower suites pass, or six old files can be opened with the historical FRS07 tool.

## 9. Explicit exclusions and handoff

Not in G4: importing/rewriting any of the six archived 2.0 fixtures; historical FRS07 source-fingerprint acceptance or converter implementation; activation/rollback of mixed generations; performance release thresholds; live provider quality/availability; proposed Observation generation/Ego inference; cross-owner or PHY semantic merge beyond accepted contracts; new public CLI `phy`/legacy `cva` commands; web daemon or cloud deployment.

**After G4:** D01 can start offline 2.0→2.1 conversion (with F03 source hash and historical-decoder proof), then D02 independent semantic parity, D03 safe activation preparation, and G5 performance/release acceptance. G4 itself never authorizes deployment or migration.
