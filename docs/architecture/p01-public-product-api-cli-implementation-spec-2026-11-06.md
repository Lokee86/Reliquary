# P01 — final Rel/Phylactery public API, consumer CLI and trusted host integration

**Status:** implementation specification; **not implemented or accepted**.
**Owner worktree:** `C:\!bin\workspace\Reliquary-rebuild` (`rebuild/clean-architecture`).
**Dependencies:** G3 and GM **accepted**; next gate G4.
**Authority:** [canonical tracker](implementation-rebuild-capabilities-2026-11-03.json), [selective-reuse implementation plan](../architecture-rebuild-implementation-plan-2026-11-03.md), [F02 selected behavior](../supported-behavior-inventory.md), [target dependency DAG](target-crate-dag-2026-11-03.json), and `Reliquary-rebuild/docs/gm-independent-reacceptance-2026-11-06.md`.

## Operational completion addendum — November 7, 2026

The [public host API and CLI completion specification](public-host-cli-completion-spec-2026-11-07.md) governs the newly identified operational exposure/composition gaps. Its audit distinguishes internal G4 integration and external inspection from a public-only producer creating and processing fresh owners. The addendum explicitly includes operational import, model configuration, processing, vectors, retrieval and necessary PHY CLI behavior; the earlier host-only command exclusions below no longer define the completed target. Original P01/G4 evidence is preserved, with separate `P01-O01..P01-O12` completion status.

## 1. Objective and completion contract

Deliver **one usable, narrow, supported public Rust product surface** over the accepted 2.1 owner/application/Gateway implementation, together with a **REL-oriented consumer CLI** and a **trusted host entry point**. P01 is primarily exposure and integration: *do not implement a second owner, copy the historical Root, or change accepted semantics to make a command easier to write.*

A new downstream consumer must be able to:
1. Create and independently reopen current-format REL and PHY owners, inspect bounded owner state, and distinguish unavailable/unauthorized history from corruption.
2. Authenticate a host principal, mount authorized owners, submit and observe native conversations, use existing source-grounded Memory/Dream/Entity/retrieval/stream workflows, and preserve the accepted-use boundary.
3. Use the **supported GM contracts** for planning and executing verified rewrite, terminal prune, profile repair, reclamation, same-owner REL reconciliation, and historical proofs without bypassing the owner admission or copy-and-reopen checks.
4. Perform the supported **REL operator subset** from a separate `reliquary` CLI using the same public library operations and safe dry-run/verified-copy behavior.

**Hard exclusions:** 2.0/FRS07 runtime readers and conversion (D01–D03); new Perception/Observation or Ego synthesis; arbitrary PHY/cross-owner reconciliation; arbitrary Dream/Insomnia/Community maintenance semantics beyond GM's accepted typed limits; direct public mutable stores; old `Cva` names or aliases; generic host transport, network daemon or unrestricted provider administration; production release/performance claims (G4/G5).

## 2. Verified starting inventory — implement against these seams

| Concern | Existing rebuild implementation | P01 gap |
| --- | --- | --- |
| Public crate | Root `Cargo.toml` already defines **`reliquary-memory`**; `src/lib.rs` currently says only “Public product boundary, under construction.” | Actual public types, controlled exports, error mapping and consumer documentation |
| Owner admission | `reliquary-owner-session::{RelSession,RelWriter,PhySession,PhyWriter,GraphAdmission}`; current `open_path` methods require validated Graph/Episode/Dream evidence | Safe public create/open and owner-proof/admission contract without exposing writable stores |
| Application | `reliquary-application/src/lib.rs` exports ingest, Dream, Entity, retrieval, history, import, rewrite, maintenance, reconciliation, vector and context workflows | Restricted product-facing entry points and policy/receipt types |
| Runtime | `reliquary-runtime-gateway::{Gateway,Instance,OwnerId,TrustedHostAdapter,...}`; `gateway_mounts.rs`, `instance_conversation.rs`, `instance_retrieval.rs`, `generation_stream.rs`, `owner_workers.rs` | Public authentication/mount/consumer binding and lifecycle ergonomics; preserve private writer and grant seams |
| CLI | No rebuilt CLI package/bin; pinned reference `Reliquary-reference/cli/src/{args,main,rel_cmd,phy_cmd,rewrite_cmd,repair_cmd,archive_cmd,import_cmd,vectors_cmd,insomnia_cmd}.rs` | Separate product-only command runner; select behavior, **do not port the old command router** |
| Host management | F02 cites `Reliquary-reference/tests/workspace_management.rs` as a retained **downstream consumer**; there is no `WorkspaceManagement` implementation in the rebuild | Bounded conversation/transcript paging, titles, Memory summaries/provenance, health and lifecycle through authorized owners |

The root `Cargo.toml` currently declares direct dependencies on lower crates. These are permitted by the current DAG but are **not** permission to export their entire modules or keep a parallel owner graph. Keep the package-count/edge ratchet unchanged unless an explicit, reviewed architecture decision justifies a change.

## 3. Public boundary design

### 3.1 Canonical public types

- Product names: `Rel`, `Phylactery`, `Gateway`, `Instance`, `OwnerId`, `ProductError`, and minimal explicitly named result/proof types. **No `Cva`, `CVA`, `Reliquary = Cva`, old Root fields, or catch-all `pub use crate::*`**.
- Keep two owner types so REL's Archive/turn/Freshness authority cannot appear on PHY. Prefer small owned, opaque handles and **immutable value snapshots**; no `Container`, `MemoryStore`, mutable `RelSession`/ `PhySession`, raw `RelWriter` or `PhyWriter` in consumer method signatures.
- Implement precise `Rel::create/open` and `Phylactery::create/open` operations that delegate to accepted owner-session admission. **Do not fabricate `GraphAdmission::for_replay(..., [], ...)` for nonempty files**, self-certify Dreams, or accept a caller-provided owner ID as authentication. If a full native owner needs retained Dream/Episode/Graph evidence, verify or derive it from existing durable owner evidence through one narrow owner-session seam; otherwise require a trusted, explicit admission object and fail typed. A basic “open works on an empty synthetic file” test is insufficient.
- Separate creation, read-only inspection and exclusive writer control. Reads from mounted writable owners use existing controlled Gateway operations, not a second mutable file handle. Source paths cannot be modified during `info`, `verify`, history, plan, or dry run.
- `ProductError`: preserve actionable distinctions (wrong format/kind, admission/owner mismatch, unauthorized/revoked principal, unavailable/pruned payload, unsupported operation, stale source/cut, conflict, current writer/lease, publication collision, I/O/verification). Never collapse all failures into “false”, a missing result or an unqualified string.
- Document stable owner/Memory/Graph IDs, result ordering, bounds, retries and the exact **2.1-only** runtime contract. Do not promise old Rust signature compatibility where `Cva` has intentionally been retired; preserve *required consumer behavior* and explicitly document the intentional rename.

### 3.2 Trusted host surface

- Export a single public way for a host to supply configured credentials, immutable REL/PHY grants and the existing Gateway host adapter. Authentication must produce an authorized `Instance`; user request parameters cannot mint or widen grants. Reuse `Gateway`'s principal, revocation, owner-kind and mounted-owner checks, including checking authorization **again at commit/handoff**.
- Preserve G3's bounded native queue, generation checkpoints, committed stream replay, cancellation/finalization races, host capability negotiation and `Held` unknown-state recovery. A CLI or host wrapper must never expose uncommitted output or invoke provider work under owner locks.
- **F02 management consumer**: provide a narrowly scoped `WorkspaceManagement` (or an explicitly documented equivalent facade) over authenticated `Instance`, including owner-scoped conversation selection, bounded conversations/transcript pages, title updates, direct Memory creation where the accepted owner workflow permits it, Memory summary/history/provenance, statistics/health, and quiesce/reconcile/reopen. Use `Reliquary-reference/tests/workspace_management.rs` as the executable behavior oracle. Retain the old contract's immutable result bounds and `MAX_MANAGEMENT_*` limits or explicitly prove an equal/stricter replacement. If an operation has **no accepted 2.1 owner/application workflow**, mark it as a concrete P01 dependency gap; add the smallest named workflow at the correct layer rather than silently dropping the consumer call or writing raw records from Gateway/Root.
- Management mutation requires a specific manage grant, not merely a view grant. Host credential revocation fences outstanding reads, writes and streaming; PHY data must never be visible through REL-only permissions.
- A host library API is sufficient: **no HTTP server, external MCP integration or new network protocol** is required for P01.

### 3.3 GM product operations (no bypass)

Expose one named, typed public operation per supported class, always through the existing Application/owner-session verified paths. Exact Rust signatures should follow the existing accepted workflow types rather than reviving the reference facade:

| Operation | Accepted implementation | Public guarantees |
| --- | --- | --- |
| Plan/rewrite REL or PHY | `application::{plan_rel_rewrite,plan_phy_rewrite,execute_memory_rewrite,verify_rewrite_source}` | Explicit policy; dry-run/plan writes nothing; selected closure and new/preserved owner generation; reject stale source; verified output and no-replace |
| Terminal Memory prune | `application::{prepare_tombstoned_memory_maintenance,prune_verified_memory_copy}` and owner-session | Only eligible terminal bodies; preserve tombstones, history and provenance; independently reopen |
| Archive/Entity/Relationship description prune | `owner_session::{prune_terminal_archive_copy,prune_terminal_entity_descriptions_copy,prune_terminal_relationship_descriptions_copy}` (already re-exported by Application) | REL vs PHY scope, source/reference reachability, typed unavailable history |
| Vector/profile repair | `application::{plan_profile_repair,execute_profile_repair,prune_retired_profile_vectors_copy}` | Verify active profile coverage, no inference/re-embedding; report retained/retired generations |
| Storage reclamation | `application::reclaim_storage_copy` | Proven unreachable backing only; second pass exact no-op; no source mutation |
| REL reconcile | `application::reconcile_rel_owner` and owner-session accepted M03 evidence | Same-owner native REL only; prefix vs semantic divergence; rebind destination Dream admissions/recipient Freshness authority; reject unsupported history, no silent loss |
| Owner history and proofs | `application::{read_owner_memory_history,read_owner_memory_history_at_cut}`; owner-session `HistoricalOwnerCut/Proof` | Bounded, owner/kind-checked revisions, tombstones/pruned/unavailable payloads and stable canonical proof |

**Independent integration check:** the existing `application::reconcile_rel_owner` reopening path must be tested with a **completed-import Dream**; M03's destination report can contain *new destination Dream admissions*. Fix this workflow if it still reopens with only original source admissions. P01 must not expose a facade that passes a simpler unit case while breaking the GM-supported branch.

Operator execution and host mutations must never bypass current leases, owner grants, durable provenance, the sync/reopen-before-publish requirement or same-volume atomic no-replace. Do not add “force” or automatic overwrite flags.

## 4. CLI contract and reference comparison

Create a **separate** `cli/` Cargo package with an independently invoked test/build, depending **only on public `reliquary-memory`**, not on Application, Gateway, owner-session, Container or service crates. Keep it outside the approved 23-package production DAG unless its addition is explicitly accepted and the checker updated. The CLI chooses options and formats results; all domain validation, owner opening and publication remain library-owned.

**Authoritative scope is a REL-oriented CLI** (P01 tracker), plus full REL/PHY Rust API. The pinned reference additionally exposes a `phy` CLI and `cva` alias. Those legacy commands are *not* automatic requirements to recreate; before closure, the agent must search documented downstream CLI usage, record whether any is a retained consumer contract, and request an explicit scope decision for any proven required PHY CLI use. **Do not claim “no consumer contract break” by ignoring that conflict.**

| Command family | Required P01 behavior |
| --- | --- |
| `rel create/info/verify` | Current 2.1 REL only, stable ID/scope/versions and bounded informative owner stats; verify performs real admission, not header-only success |
| `rel history` (subcommands as needed) | Current/selected-cut Memory, Archive and proof summaries with explicit unavailable/pruned status and bounds |
| `rel rewrite` | Policy file + bounded repeatable ID/profile overrides, `--dry-run`, exact resolved plan and no unexpected output |
| `rel prune` / `rel repair` / `rel reclaim` | Only GM-accepted terminal prune, profile and reclaim operations; deterministic reports, supported policy and reference-safe copy publication |
| `rel reconcile` | Same-owner prefix/divergent REL, explicit two sources and distinct destination; typed conflict and no source changes |
| `rel archive`, `rel import`, `rel vectors`, `rel insomnia` | Include only the **retained reference consumer behaviors** with an existing accepted 2.1 Application/Gateway workflow. Record each reference `archive_cmd.rs`, `import_cmd.rs`, `vectors_cmd.rs`, `insomnia_cmd.rs` operation as mapped, intentionally retired or blocked; no placeholder success and no porting reference-only internal/dev commands |
| General CLI controls | `--help`, `--version`, explicit file-kind errors, nonzero failure exit, stable machine-readable JSON for reports; secrets never echoed or placed on command line; no destructive default |

**Do not automatically copy** the pinned reference CLI's `dev`, `migrate-conversation-titles`, compatibility aliases, old `--config` transport and stale 2.0 policy schema. Where a supported config or credential operation is required by a real host/CLI consumer, implement via the existing Switchboard/public policy under an approved route, not a local duplicate. A `phy` binary/command is outside the initial CLI scope unless the compatibility check documents and resolves it.

## 5. Implementation sequence — small dependency-ordered slices

Each slice is a cohesive compile/test/commit unit, **not** permission to introduce placeholders. Stop on a concrete boundary conflict, document it and resolve it before altering the approved DAG. Suggested source folders are illustrative; split individual code files to keep coordinators small.

1. **P01-A — freeze consumer manifest.** Read F02 and the pinned reference tests `workspace_management.rs`, `native_import.rs`, `historical_payload_boundary.rs`, `freshness_retrieval_boundary.rs`, `multiplexing_*contract.rs`, and the reference CLI command/argument tests. Write `Reliquary-rebuild/docs/p01-consumer-contract-map.md` with *each retained behavior → new public method/CLI command → executable test → explicit intentional difference*. Confirm CLI/PHY legacy use, admissible owner-proof construction and available management primitives. No production implementation change.
2. **P01-B — public root and typed owners.** Replace root `src/lib.rs` placeholder with minimal organized exports and opaque `Rel`/`Phylactery` values. Implement true 2.1 create/open/info/verify, owner/kind validation and useful `ProductError`. Tests compile **only against `reliquary_memory`** and prove nonempty REL and PHY reopen, wrong kind/2.0 refusal, owner ID stability, and read-only immutability.
3. **P01-C — authenticated host and owner access.** Provide the public trusted host/mount/Instance acquisition API using the existing Gateway, immutable grants and revocation. Add compile-level consumer fixtures for REL vs PHY permissions, same-owner registry uniqueness and writer exclusion. No public `Gateway::instance` bypass for arbitrary principals.
4. **P01-D — management and operational product surface.** Implement the retained management caller operations via narrowly named Application/owner-session workflows and Gateway instance checks. Wire accepted ingest/retrieval/context/Dream/Entity/stream functions where the public host needs them. Verify bounded pages, title/memory/provenance, grant separation, state recovery and no provider work under locks. Retain G3 test coverage.
5. **P01-E — maintenance/history facade.** Expose all accepted GM methods and reports, with read-only policy planning and verified-copy writers. Add full consumer tests for REL/PHY rewrite, terminal prune, profile repair, reclaim/no-op, historical proof and supported M03 divergent import/Dream/Insomnia reconciliation; exercise destination admission and negative races. **Do not revive unsupported historical/PHY merge operations.**
6. **P01-F — standalone REL CLI.** Implement selected command grammar against the public crate. Test `--help`, expected JSON fields, dry-run source/output immutability, wrong-kind and 2.0 refusal, nonempty reopen, conflict/nonzero exit, no-replace collision, and actual result parity with equivalent API calls. Audit retained reference commands using the manifest from P01-A.
7. **P01-G — contract and docs closure.** Complete public Rustdoc, brief downstream host/CLI examples, public migration mapping (`Cva` deliberately removed), unsupported-scenario matrix and known upgrade boundary. Compile a standalone external consumer depending only on the Root crate; run all locked/offline checks; create `Reliquary-rebuild/docs/p01-public-product-acceptance-2026-11-06.md`. Commit and mark tracker `P01=complete` **only after** independent verification. G4 stays separately unaccepted.

## 6. Required adversarial tests (public layer, not internal unit proxies)

- **Authentication:** unknown/revoked principal, unauthorized REL, PHY-private access through REL grant, mismatched owner/kind, duplicate mounted writable owner and stale writer lease; no data or checkpoint leaks.
- **Workflow fidelity:** accepted turn → Episode → grounded REL/PHY Memory → authorized retrieval → confirmed external handoff and Freshness reinforcement **only after** confirmation. No re-award on retry; blocked/ambiguous host outcome remains Held and private.
- **Management:** downstream `WorkspaceManagement` create/select/page/title/Memory/provenance/health/quiesce/reopen flow; caps cannot be bypassed, mutation requires manage permission, tests do not reach raw owner-store state.
- **Maintenance:** plan/dry-run no writes, stale plan, wrong scope, source-preserving success and failure, referenced payload refusal, selected Entity/Relationship closure, proven reclaim and second exact no-op, separate owner IDs/generations.
- **Reconciliation:** identical/prefix/diverged same owner; same-import completed Dream with new destination admissions; source-preserved Dream population, exact Insomnia terminal/retry work history, Freshness settlement authority and byte-stable repeated merge; conflicting work/history fails with no output.
- **CLI:** independently executable parser, no `cva` command/alias, no implicit `phy` command absent approved exception, zero silent no-op commands, machine-readable bounded reports, bad-path exit, existing output preservation and no access to 2.0 source data.
- **Architecture:** external consumer sees public *types and values* only; package graph stays acyclic; Root does not own a mutable Container or semantic pipeline, CLI imports only Root, and no tests cheat by using private crate APIs as the exercised surface.

## 7. Verification and acceptance

Run in `C:\!bin\workspace\Reliquary-rebuild`, reusing **one** target directory on Windows:

```powershell
python scripts/check_architecture.py
cargo fmt --all --check
cargo check --workspace --locked --offline -j 1 --target-dir target-codex-check
cargo test --workspace --locked --offline -j 1 --target-dir target-codex-check -- --test-threads=1
# The separate CLI has its own manifest/lock and explicit tests:
cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1 --target-dir target-codex-check
cargo run --manifest-path cli/Cargo.toml --locked --offline -- --help
```

Also compile a small *external* consumer using only `reliquary-memory` imports; run CLI/API equivalent scenario comparisons. The accepted DAG baseline is **23 production packages / 76 approved production edges**; any change requires an explicit amended architectural decision, updated ratchet and explanation. Do not weaken checker/tests for convenience.

**P01 acceptance requires all of:** G3 and GM remain green; complete retained consumer manifest with no unapproved break; real public REL/PHY create/open and host/management/maintenance functions; real REL CLI behavior and negative exit semantics; independent 2.1 reopen and no source damage; authentication/PHY privacy/confirmed-use gates; passed workspace, CLI and external-consumer checks; accepted evidence document; clean committed worktree and canonical tracker update.

**Not part of P01:** G4 synthetic whole-product acceptance, D01–D03 archived 2.0 conversion/cutover and G5 performance/release activation. No claim that those gates pass merely because P01 tests are green.