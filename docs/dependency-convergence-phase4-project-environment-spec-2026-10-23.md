# Phase 4 — Project Environment and Repository Boundary Convergence

**Status:** Specified for review; **not implemented**.  
**Date:** 2026-10-23.  
**Repository:** Reliquary.  
**Campaign:** `docs/dependency-convergence-architecture-refactor-2026-10-21.md`, Phase 4.  
**Execution prerequisite:** Phase 3 A3 handoff accepted, canonical source/format interfaces stabilized, and the Phase 1–3 closure audit C-01–C-04 source-conformance obligations resolved. Planning/source inspection may proceed; do not begin hard-cut implementation while those graph proofs are unresolved.  
**Scope:** Correction-only architectural convergence. No new repository features, external sync, semantic features, or new storage format.

## Overview

Correction-only Project Environment architectural convergence on the accepted Phase 3 main baseline. The phase concerns repository authority, accepted project subtree scope and separation of public/runtime consumers from the durable owner boundaries.

## Purpose

Establish one validated repository/project authorization boundary without changing user-visible project history or the stored REL/PHY format. Phase completion requires exact source-level dependency evidence, not only local passing tests or a hand-authored target dependency diagram.

## 1. Problem statement and verified source baseline

Reliquary already implements managed Lore, manually managed Lore, and Git Project Environment operations. The current code correctly separates project VCS bytes from REL/PHY semantic storage in important ways, but repository selection, scope checking, path resolution, upload preparation, historical reads, and semantic publication are split across backend helpers, Project Environment and Gateway/Cva construction. Repository identity checks alone do not prove authority over an arbitrary project subtree of that same repository.

This makes a caller-selected local folder capable of being treated as a valid repository merely because it resolves to the expected repository ID. A project belonging to a nested subtree must not implicitly authorize sibling or parent tree content. An exact historical file reference is also not, by itself, authorization to access the referenced repository path. A selected path and accepted REL association are distinct inputs.

### Source observations (2026-10-23; no Phase 4 mutation)

| Source | Verified behavior / reason to address |
| --- | --- |
| `src/project_environment/file.rs`; `git_file.rs`; `lore_file.rs` | Historical reads dispatch by reference kind and verify repository ID and optional content digest, but do not bind the requested reference to the REL's independently accepted project association. |
| `src/project_environment/repository.rs` | `current_revision_for` and `repository_root` compare durable repository identity; a project-path mismatch is not consistently rejected here. Reserved ignore/exclude maintenance may also run during current-revision resolution. |
| `src/project_environment/git_upload.rs`; `attachment.rs`; `lore_upload.rs` | Git upload compares expected repository identity but not the full association; Lore upload is selected using the correlation's mode yet does not independently verify its expected ID/project path before mutating Lore. |
| `src/project_environment/lore_repository.rs`; `git.rs`; `upload_paths.rs` | Backend discovery and path preparation differ. Git canonicalizes repository roots and project directories; Lore uses ancestor discovery and lexical relative paths; candidate enumeration and upload paths need one checked project scope. |
| `src/runtime_inference/runtime_host_gateway_ambient_project.rs` | Gateway grants `ViewRel` for file bytes and `ManageProject` for project mutations; it delivers/revalidates authorization after external I/O. Historical reads provide a caller-local `project_dir` and a file binding but do not also convey the accepted repository correlation. |
| `src/runtime_inference/runtime_host_gateway_project_lifecycle.rs` | Validation may append a new project-path correlation when the discovered path differs but the repository ID matches. Move/rename support must not become silent reassociation to an arbitrary sibling folder. |
| `src/project_environment/lifecycle.rs` and `repository.rs` | Project Environment currently constructs/opens and directly mutates `Cva`. Final owner-lifecycle publication and project repository planning need a downward boundary, not a new facade callback. |
| `src/archive/project_history_model.rs`, `project_history_store.rs`, `project_file_binding_store.rs` | These hold historical **correlations and references** (`ProjectRevisionCorrelation`, `ProjectFileRef`), not repository tree/blob ancestry. Preserve this accepted semantic evidence. |
| `src/archive/file_store.rs`; `project_file_attachment_tests.rs` | Project-file identities and source attachment bindings are part of Archive/semantic history; do not confuse them with duplicate VCS content storage. Ordinary Archive-owned file content also remains legitimate. |
| `src/project_environment_git_tests.rs`, `project_environment_lore_tests.rs`, `project_environment_tests.rs`, `runtime_host_gateway_project_tests.rs` | Existing end-to-end tests cover Git branch/index preservation, Lore exact historical reads, nested uploads, missing identities, grants, and post-I/O authorization invalidation. They are the primary preservation seams. |

**Important negative finding:** The source inspection does **not** establish that `ProjectHistoryStore` or `ProjectFileStore` is an unnecessary second VCS. Do not delete, rename away, or rewrite them just to satisfy the broad campaign description. Any purported duplicate file ancestry/merge/restore implementation must be identified by precise source, consumers and evidence before a deletion is scheduled. The absence of an identified duplicate is an acceptable outcome of the inventory gate.

The existing Arcana manifest is known to reject reads with a `wrong field count` error. Use direct source search and tests for this specification; do not refresh Arcana before the Phase 3 D3 checkpoint, or launch repeated whole-repository scans for Phase 4 planning.

## 2. Controlling decisions and invariants

- ADR 0027: Lore/Git alone owns project-file bytes, tree/revisions, rename ancestry, branches and project-file merge/history. REL/PHY retain independent semantic history and can correlate exact project revisions.
- ADR 0028: the selected project folder and repository root may differ; nested projects, complete-folder moves and `.warlock/` exclusion matter. Git never receives an automatic commit from project bootstrap; manual Lore has explicit checkpoint policy.
- ADR 0037 and ADR 0038: Reliquary owns Project Environment implementation. Gateway/Instance owns mounted owner authorization and coordination; owner-local data and durable publication remain below it. No upward callback into Warlock or the public facade.
- Phase 3 H3/M3/I3 and A3: consume completed read-only History/owner-lifecycle/store boundaries. Do not reimplement replay, Import completion, Maintenance/rewrite/reconciliation, source copying, or physical writer machinery.
- The 34-component/163-permitted-edge target DAG places `project-environment` at tier 4 over `archive-store` and `owner-lifecycle`. It must not import `runtime-gateway`, facade `Cva`, runtime host presentation, or higher semantic services.
- Preserve current `ProjectRepositoryMode` choices (`ManagedLore`, `ManualLore`, `Git`), durable `ProjectRepositoryRef`/`ProjectRevisionRef`/`ProjectFileRef`/`ProjectRevisionCorrelation` semantics, Git snapshot behavior, Lore checkpoint rules, REL/PHY privacy/semantic data and historical cut evidence.
- Preserve existing durable/named external markers such as `warlock.repositoryId`, `.warlock/`, `warlock/uploads` and `refs/warlock/snapshots` where already used by persisted references or backend state. A Warlock-origin name is not, by itself, an architectural dependency and is not permission to break on-disk references.
- No inference, network access, or repeated model calls are required for Phase 4 acceptance.

## 3. Target architecture and call direction

### 3.1 Canonical Project Environment authority

Introduce **one private, validated project authority** (working name `ValidatedProjectScope`) inside `project_environment`. This is a deep module, not a generic new VCS framework or an external public API.

It is built from **both** (a) a selected local project folder/backend discovery result and (b) the independently accepted Project REL association, including durable historical correlations when historical references are resolved. It encapsulates:

- actual backend kind and durable repository identity;
- canonical machine-local repository root and selected project directory; these are ephemeral, never used as durable repository identity;
- normalized repository-relative selected project path and current management policy;
- accepted historical project-path associations from that REL, rather than paths asserted solely by a supplied `ProjectFileRef`;
- backend access bound to that validated repository; direct callers never pass an unvalidated root into lower Git/Lore read or mutation helpers.

**Creation and adoption exception:** a not-yet-associated REL has no accepted project correlation. Construct its initial candidate scope from the explicitly selected creation/adoption project directory and mode, under existing lifecycle admission; finalize durable correlation exactly at the original initialization checkpoint. This is not a general way to bypass validation on an already-associated REL.

The validated scope offers a small set of operations: current revision/root resolution; authorization of an existing historical `ProjectFileRef`; exact historical bytes; upload preparation/materialization; and existing explicit Lore checkpoint. These may be separate focused internal methods, not one exposed mutable repository object with every backend operation.

**Authority is not an instance grant.** Gateway must continue checking `ViewRel`/`ManageProject` and must reauthorize at delivery/publication after I/O. Project Environment validates the repository/project scope and historical file path. Neither check substitutes for the other.

### 3.2 Path and reference invariants

1. Verify backend kind **and** actual durable repository ID against the REL's accepted association. Matching ID alone must not silently authorize any path.
2. Normalize repository-relative paths structurally. Reject empty/absolute, drive-qualified, backslash/mixed-separator bypasses, `.`/`..` path components, malformed relative roots, invalid project-subtree prefix and reserved semantic-state file paths. Do not use string prefix matching to authorize filesystem containment.
3. Canonicalize existing host paths before physical access and check containment against the **selected project subtree**, not merely the repository root. Address symlink/junction traversal, root/checkout aliases, Windows case/volume considerations, and changing paths between validation and filesystem operations with an appropriate fail-closed per-operation check. Do not invent a global filesystem transaction/lock manager.
4. Repository tree paths used for historical reads are logical repository-relative paths. Check their syntax and accepted project scope before dispatch, then let Lore/Git resolve the exact retained historical revision; do not require the old file to still exist in the working tree.
5. An accepted historical `ProjectFileRef` may reference an earlier enrolled project path after legitimate relocation. Verify that (kind, ID, historical `project_path`) occurs in the owning REL's accepted correlation history and the file lies within that historical subtree. The reference alone cannot enroll a new path or silently rebind the current project.
6. New uploads/checkpoints/root operations always require the **current authorized project path**. A historical path grants exact historical read authority only, never present-day write authority.
7. Preserve content-hash verification where present and valid historical references with no stored hash where already supported; do not invent a new wire-format requirement for old references.
8. Wrong-project/sibling path, repository replacement, missing/changed identity, invalid file path, inaccessible old revision and mismatched content all fail before semantic publication. Keep existing specific refusal/error behavior when possible and document any necessary tightening.

### 3.3 Relocation and durable identity

A whole project can move on disk without changing the durable repository identity. **Machine-local root is a mount fact; (repository kind, repository ID) is durable identity; the enrolled project-relative subtree and its accepted changes govern path scope.**

The existing validation/reassociation entrypoint may record a genuine selected-project relocation, but may **not** treat any other directory under the same repository as that project solely because the repo ID matches. Use the existing project owner/mount correlation and applicable explicit management authorization to justify any path update. Do not require byte-preserving absolute checkout paths. Earlier enrolled paths remain valid for historical references at their earlier revisions.

During implementation, test the exact live relocation mechanism against ADR 0028. If the current host supplies insufficient evidence to distinguish a moved project from a sibling project, fail closed and isolate the missing proof at the existing validation boundary; do **not** silently widen authority or implement a new project-selection/product workflow as part of Phase 4. Record that constraint and the required bounded caller contract before accepting the relocation gate.

### 3.4 Ownership map

| Responsibility | Final owner | Not owned here |
| --- | --- | --- |
| Project backend discovery, project-scope validation, Git/Lore calls and project file authorization | `project_environment` | Warlock callbacks, runtime presentation, another VCS |
| Lore and Git tree/file/history/branch/commit authority | Lore/Git backend | REL/PHY |
| REL project-correlation and exact file-binding durable records, sequence/error invariants | Archive/owner store | Project Environment reimplementation of Archive storage |
| REL lifecycle construction, storage write admission, correlation publication and physical durability | accepted `owner-lifecycle` / Archive publication seam | `project_environment` constructing/mutating `Cva` |
| Mounted grant checks, lifecycle claim, pre-I/O and post-I/O authorization, long-I/O scheduling | Gateway/Instance | repository path proof |
| REL semantic history, semantic cuts, Memory/Graph/Ego and retrieval state | existing respective owners / History | Lore/Git |
| Public gateway/CLI convenience compatibility | upper public adapters, scheduled for final facade cleanup in Phase 6 | lower owner dependencies on facades |

Call direction: **Gateway / supported public composition -> Project Environment -> backend Git/Lore** for repository operations; Project Environment consumes lower immutable Archive correlation DTOs and accepted lifecycle capability when unavoidable. **Gateway -> owner-lifecycle/Archive** for correlation/attachment publication. No owner store directly calls Project Environment to resolve an ambient path, and Project Environment does not own a persistent mutable REL/PHY.

### 3.5 Interfaces and ownership migration

- Consolidate `git::validate_reference_repository`, `lore_file::validate_repository`, `repository::repository_root`, `repository::current_revision_for` and related checks behind the same scope construction/validation contract. Keep backend-specific details private.
- Replace `file::read_project_file(project_dir, reference)`'s ambient authorization with a call consuming validated association and accepted historical correlation evidence.
- Replace `attachment::prepare_upload(correlation, project_dir, source)` and direct backend preparations with a validated **current** scope, preserving source-byte read, dedupe, snapshot, publication and retry order.
- Route `checkpoint_manual_lore`, project root and current revision through the current validated scope; only approved mode may checkpoint.
- Move direct `Cva` construction/correlation effects in `project_environment/lifecycle.rs` and `repository.rs` to the already-approved owner-lifecycle or upper lifecycle coordinator at the **final target**, rather than inserting a new facade-forwarding shim. Phase 4 must not reopen Phase 3's accepted lifecycle implementation.
- Keep public `GatewayRuntime` and current Cva convenience signatures functional; migrate internal logic downward. Wider public reexport/renaming cleanup belongs to Phase 6.
- Preserve present Git index/HEAD invariants and `refs/warlock/snapshots` retention for captured uploaded bytes. Preserve Lore managed/manual initialization and checkpoint behavior.
- Never invoke repo mutation as an incidental side effect of validation except documented currently required exclusion maintenance at the deliberate existing checkpoint; move that maintenance behind scope authorization before changing files.

## 4. Developer and user outcomes

- Historical files cannot escape the selected Project REL's accepted repository subtree even if another folder shares the same repository.
- The same project can be opened by its intended current mount and can resolve older, durably enrolled historical paths after a legitimate relocation.
- Git snapshots do not mutate HEAD, index or user branch. Lore state is initialized and checkpointed only according to existing management mode.
- Reliquary stores semantic correlations and exact references, not duplicate project-file ancestry or blobs.
- Repository policy, permission proof and backend-specific behavior have single, reviewable interfaces; lower semantic owners do not depend on Gateway or `Cva`.

## 5. Hard-cut migration strategy

**Staged hard cut**, not compatibility-preserving coexistence of old and new *internal* entrypoints.

- **Before implementation:** Phase 3 A3 must be accepted. Freeze relevant source hashes, current tests, externally meaningful stored identifiers and known refusals. No Phase 4 rewrite of Phase 3's in-flight imports/history.
- **First move:** establish canonical private validated authority and a clear lifecycle/publication boundary. No generic project backend registry, VCS abstraction framework, or extra facade adapter layers.
- **Then migrate** read consumers, current root/validation, upload/checkpoint consumers and creation/adoption in cohesive slices directly to that target. Preserve the existing two real backend adapters (Lore/Git).
- **Delete old paths** immediately after their last consumer moves. A bounded implementation slice may be temporarily red; record its exact expected breakage. Full green is required at named acceptance gates.
- **No dual formats:** this phase does not modify `PRJCOR02`, `PRJFILE1`, `ProjectFileRef` durable encoding, REL/PHY storage, canonical corpus or physical headers. A discovered unavoidable wire-format change is a **stop gate requiring separately approved design**, not automatic Phase 4 migration machinery.
- **No temporary synthetic tests or compat layers:** use real backend/high-level acceptance tests, with small additional scope-authorization negatives where existing tests do not cover the missing behavior.
- **One source-graph check at stabilized integration:** reuse the Phase 3 accepted source/DAG snapshot if valid for navigation, then do a single Phase 4 acceptance structural comparison when its changed-source graph is available. Do not rescan on every step.

### Phase 3 interface freeze / handoff

Phase 3 remains an actively mutated worktree at this spec's date. It is not a Phase 4 foundation to modify. At A3 handoff record: accepted History/owner-lifecycle access shapes, current wire format after C3, source graph checkpoint after D3, final baseline tests, project-related touched files, and canonical owner authority. Rebase/merge policy and implementation worktree isolation require separate execution authorization.

No Phase 4 task may claim acceptance based solely on Phase 3's historical tests, and no Phase 3 ticket is reclassified as Phase 4 progress.

## 6. Execution specification — 39 bounded tasks, seven acceptance gates

**Corrected 2026-10-23:** The initial 35-task plan concealed multiple independent migrations. The 27-task revision removed administrative steps without adequately splitting them. This specification has **39 tasks because the actual implementations were decomposed**, not because a higher count was targeted. Neither predecessor was executed or accepted.

**Granularity contract:** Each task represents a specific implementation owner or consumer cut, identifies hard prerequisites, and includes observable completion evidence. Tests, scans, receipts, progress reports and gate sign-offs are not extra tasks. Keep actual owner-lifecycle/History from Phase 3; do not create bridge wrappers, duplicate APIs, migration-only shims or unnecessary features. A task is done only after its final implementation and immediate caller/old-path cleanup are verified. Targeted checks are appropriate during an explicitly red hard-cut window; full greenness is required at integration gates.

**Hard implementation prerequisite:** Phase 3 A3 accepted with current store/History/format/graph baseline. The existing Phase 3 worktree must remain untouched. P4.0 freezes current consumers; **B4** is where the denominator becomes authoritative, before execution beyond inventory. Discovery of genuinely new owner work later requires a reviewed task split and reconciled count, not an indefinite hidden subtask.

### Confirmed consumer register (source inspection 2026-10-23)

| Source | Current responsibility and final disposition |
| --- | --- |
| `src/project_environment.rs`, `repository.rs` and `lifecycle.rs` | Repository preparation/current root, creation/adoption. `PreparedProjectRepository` and `lifecycle.rs` depend directly on `Cva`; cut to Project Environment preparation + accepted upper owner-lifecycle composition (P4.1–P4.2). |
| `src/project_environment/git.rs`, `lore.rs`, `lore_repository.rs` | Keep distinct real backend adapters, but share one validated project/repository authority and scope grammar (P4.1). |
| `src/project_environment/file.rs`, `git_file.rs`, `lore_file.rs` | Historical file reads currently validate repository identity/hash but not independently accepted owning-REL subtree; replace ambient read with validated historical scope (P4.3). |
| `src/project_environment/attachment.rs`, `git_upload.rs`, `git_snapshot.rs`, `lore_upload.rs` and `upload_paths.rs` | Current uploads, dedupe, Git snapshot retention and Lore commit must require a validated current scope before filesystem/repository mutation (P4.4). |
| `src/runtime_inference/runtime_host_gateway_ambient_project.rs` | Exactly **five** Project Environment calls: `repository_root`, `current_revision_for`, `checkpoint_manual_lore`, `prepare_upload`, `read_project_file`. Single designated integration owner migrates these calls; preserve grants and post-I/O delivery checks (P4.3–P4.4). |
| `src/runtime_inference/runtime_host_gateway_project_lifecycle.rs` | Creation/adoption/validation; existing same-repository-ID, different-project-path auto-recorrelation requires legitimate relocation proof rather than automatic scope widening (P4.2). |
| `src/archive/project_history_store.rs`, `project_file_binding_store.rs` and `file_store.rs` | Preserve semantic correlation, exact file bindings and Archive content. These are **not** proved to be duplicate Lore/Git project VCS (P4.0.03, P4.5). |
| `src/runtime_inference/runtime_host_gateway_ambient_tests.rs` | One direct test call to ambient `read_project_file(project_dir, reference)`; change with P4.3.03. Existing Project Environment/Archive/Gateway tests are acceptance seams, not fresh test-work packages. |

At B4 repeat bounded source searches after A3, identify exact external/CLI consumers (if any), and freeze the register. Do not infer that every Archive/Import/History consumer of persisted `ProjectFileRef` calls the project repository service.


### P4.0 — Baseline and bounded discovery (4 bounded tasks; B4)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.0.01** — Phase 3 handoff | A3 | Freeze accepted owner-lifecycle/History/Archive interfaces, format, source guard and test baseline. | Explicit receipt; no Phase 3 code changes |
| **P4.0.02** — Actual callers | P4.0.01 | Enumerate Git/Lore repository I/O, five Gateway production calls and direct test/CLI/host callers; map authorization and side effects. | No unspecified migration consumers |
| **P4.0.03** — Duplicate VCS evidence | P4.0.01 | Audit project-history correlations, file binding, Archive contents and any actual duplicate repository ancestry. | Exact source/consumer deletion candidates, or verified none |
| **P4.0.04** — Relocation proof contract | P4.0.02, P4.0.03 | Distinguish legitimate moved Project REL from arbitrary sibling subtree sharing repository ID. | Defined evidence and fail-closed fallback before migration |

**B4 gate (not counted):** Post-A3 exact source/caller map, same-repo relocation/fail-closed decision, preserved Archive evidence, proven duplicate-VCS deletion targets (or verified none), and tests/contracts frozen. Only then freeze execution task count.

### P4.1 — Canonical project authority (10 bounded tasks; A4)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.1.01** — Private scope authority | B4 | Create small internal validated current/historical project authority; no Cva, facade, Gateway or Warlock types. | Final typed inputs/outputs and owner boundary |
| **P4.1.02** — Project path parser | P4.1.01 | Normalize ProjectRepositoryRef.project_path with special valid root '.' and strict relative grammar. | Root/nested/malformed path checks |
| **P4.1.03** — File path parser | P4.1.02 | Normalize ProjectFileRef.path separately, forbidding file-root and traversal. | Exact valid references retained; negatives rejected |
| **P4.1.04** — Subtree proof | P4.1.02, P4.1.03 | Require component-aware historical and current file inclusion under accepted project path. | No sibling-prefix authorization |
| **P4.1.05** — Physical selected mount | P4.1.04 | Resolve actual backend repository root and selected project directory, including symlink/junction/alias handling. | Only physically contained current selection accepted |
| **P4.1.06** — Safe write destination | P4.1.04, P4.1.05 | Authorize current upload target and collision/materialization path under selected subtree at access boundary. | No symlink/junction escape or outside-project writes |
| **P4.1.07** — Git identity constructor | P4.1.04, P4.1.05 | Bind discovered Git kind/ID/root/project path to accepted correlation; distinguish initial bootstrap. | Same-ID wrong project and replaced/missing Git rejected |
| **P4.1.08** — Lore identity constructor | P4.1.04, P4.1.05 | Bind discovered Lore ID/root/subtree and management to accepted correlation. | Wrong ancestor/sibling and changed Lore rejected |
| **P4.1.09** — Enrolled historical project path | P4.1.07, P4.1.08 | Prove referenced project path appears in requesting REL's accepted correlation history, not just the reference. | Old enrolled paths read-only; unrecognized paths refused |
| **P4.1.10** — File binding authorization | P4.1.03, P4.1.04, P4.1.09 | Combine exact owning Archive file binding with accepted historical association before repository read. | Forged/wrong-owner binding refused; old file need not exist locally |

**A4 gate (not counted):** Both backends share the same private validated scope; root/project/file path semantics, physical containment, historical enrolled correlation and actual Archive file-binding admission are proven. Gateway migration is not claimed yet.

### P4.2 — Lifecycle and relocation (6 bounded tasks)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.2.01** — Unassociated preparation | A4 | Extract repository preparation/rollback results from Project Environment Cva path. | Lore initial checkpoint, Git no-commit, no broad bootstrap bypass |
| **P4.2.02** — Create REL publication | P4.2.01 | Route create_project_rel Cva creation, correlation/sync and rollback to accepted higher lifecycle coordinator. | No lower Cva constructor; owner/no-overwrite and errors preserved |
| **P4.2.03** — Adoption inspection | P4.2.01 | Move Cva::open_read_only, owner identity and no-correlation proof to existing upper lifecycle. | Inspection read-only, no duplicated Phase 3 machinery |
| **P4.2.04** — Adopt REL publication | P4.2.02, P4.2.03 | Move adopt_project_rel correlation/sync/rollback into same final coordinator. | No lower Cva mutation; source identity and failures preserved |
| **P4.2.05** — Current association validation | A4, P4.2.04 | Update both Gateway validation sites to require accepted selected project path. | Same repository ID no longer authorizes sibling rebind |
| **P4.2.06** — Relocation/re-association | P4.2.05 | Permit only moves proven by existing lifecycle/REL association, preserving old enrolled read-only paths. | Genuine complete-folder move works where proved; else fail closed |

**Lifecycle join:** P4.2.01–.06 must remove Project Environment's direct Cva construction, open, mutation and recorrelation. They join M4 and I4, not an invented eighth gate.

### P4.3 — Historical file reads (4 bounded tasks; H4)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.3.01** — Git reader | A4 | Make git_file.rs consume validated historical scope, revision and tree path. | Exact bytes/hash/snapshot/GC tests, wrong subtree refuses |
| **P4.3.02** — Lore reader | A4 | Make lore_file.rs consume same validated scope before exact revision output materialization. | Exact bytes/hash/edit/delete tests, wrong subtree refuses |
| **P4.3.03** — Validated file dispatch | P4.3.01, P4.3.02 | Replace ambient project_dir+reference project_environment/file.rs dispatcher with typed scoped dispatch. | Old unscoped internal read entrypoint deleted |
| **P4.3.04** — Gateway read consumer | P4.3.03 | Migrate resolved_file_bytes_for to capture owning binding+correlation history, I/O outside lock and reauthorization. | ViewRel, races and Archive-local fallback preserved; direct test caller migrated |

**H4 gate (not counted):** Git/Lore exact historical files, GC, edit/delete, previously enrolled paths, no wrong-REL/sibling/forged/path traversal reads, grants, short locks, revoked delivery and Archive local-byte fallback. No ambient read path survives.

### P4.4 — Current repository operations (13 bounded tasks; M4)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.4.01** — Current revision | A4, P4.2.05 | Migrate current_revision_for and Gateway validation to accepted current scope. | Wrong selected subtree fails; query has no ignore side effect |
| **P4.4.02** — Repository root | P4.4.01 | Migrate repository_root and Gateway root consumer to current scope. | Correct root; ManageProject authorization preserved |
| **P4.4.03** — Git excludes | P4.4.02 | Gate info/exclude mutation to authorized Git bootstrap/maintenance. | No change on invalid scope or to HEAD/index |
| **P4.4.04** — Lore ignores | P4.4.01 | Gate .loreignore modification to authorized Lore preparation/maintenance. | No change on invalid scope; reserved exclusion intact |
| **P4.4.05** — Manual Lore checkpoint | P4.4.01, P4.4.04, P4.2.06 | Migrate checkpoint_manual_lore and Gateway caller through validated manual Lore scope. | Correct revision, errors and post-I/O grants |
| **P4.4.06** — Git tracked-match | A4, P4.2.06 | Constrain Git dedupe candidate enumeration and exact match to current subtree. | No sibling reuse; deterministic eligible match |
| **P4.4.07** — Git materialization | P4.4.06, P4.1.06 | Constrain upload destination/collisions to current validated subtree. | No symlink or parent escape; exact bytes |
| **P4.4.08** — Git snapshot | P4.4.07 | Bind Git temporary-index snapshot and ref to validated uploaded file path. | HEAD/index untouched, ref survives GC |
| **P4.4.09** — Lore tracked-match | A4, P4.2.06 | Constrain Lore candidate scan and historical equality matching to selected subtree. | No sibling reuse; exact deterministic match |
| **P4.4.10** — Lore materialization | P4.4.09, P4.1.06 | Constrain uploads and collisions to validated current subtree. | No outside-project materialization |
| **P4.4.11** — Lore stage/commit | P4.4.10, P4.4.04 | Apply current scope to attachment stage/commit and managed/manual exception. | Repository ID verified before write; exact revision |
| **P4.4.12** — Gateway upload preparation | P4.4.08, P4.4.11 | Migrate ingest_project_attachment_for to scoped backend preparation outside owner locks. | Delete ambient prepare_upload; refused scope causes no repository writes |
| **P4.4.13** — Gateway Archive publication | P4.4.12 | Publish exactly accepted prepared revision correlation+file binding and reauthorize delivery. | No revoked/detached REL publication; FileId/provenance/race preserved |

**M4 gate (not counted):** P4.2.06 and P4.4.13 accepted; creation/adoption/current path, Git/Lore excludes, manual Lore checkpoint, both upload pipelines, post-I/O authorization, index/HEAD stability, historical bytes and no unauthorized repository/REL mutations.

### P4.5 — Verified VCS retirement (V4 gate; no speculative implementation task)

**V4 gate (not counted):** Following **B4, H4 and M4**, use the recorded deletion/retention inventory. Delete any *proven* duplicate project-tree/history implementation via explicit source-bounded tasks **defined before B4 freezes the denominator**. If, as in the inspected code, no separate project VCS implementation is identified, record **no deletion** rather than inventing work. Verify all superseded ID-only, ambient read, upload and Cva lifecycle entrypoints were removed within their respective migration tasks. Preserve `ProjectHistoryStore`, `ProjectFileStore`, ordinary Archive contents and semantic history. No task exists merely to search, delete nothing or report a gate.

### P4.6 — Dependency policy ratchet (1 bounded task; D4)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.6.01** — Source owner ratchet | V4 | Tighten existing Project Environment -> facade/Cva/Gateway/default-deny and validated-backend rules. | Source guards/reverse consumers checked; no new scanner |

**D4 gate (not counted):** Source policy/reverse consumers and one stabilized DAG graph if available. If Arcana still fails, explicitly report missing graph evidence; direct-source checks aren't misrepresented as a graph pass.

### P4.7 — Documentation and handoff (1 bounded task; I4)

| Task | Hard predecessors | Implementation boundary | Completion evidence |
| --- | --- | --- | --- |
| **P4.7.01** — Final canonical documentation | D4 | Update Architecture, subsystem, API, limitations, maintainer map and ADR cross-references with actual ownership/relocation. | Phase 5 handoff recorded; no invented feature/compatibility |

**I4 gate (not counted):** Locked Rust/CLI, project/Gateway grant races, Archive historical binding, import/repack/reconciliation and applicable corpus tests; no `PRJCOR02`, `PRJFILE1` or REL/PHY format changes; final docs/source/deletion receipts with real pass/fail/ignored statuses. No repeated exhaustive oracle after each task.
### Execution dependency order and ownership

1. **A3 → P4.0.01 → {P4.0.02, P4.0.03} → P4.0.04 → B4.**
2. **B4 → P4.1.01–.04 → P4.1.05/.06 and Git .07/Lore .08 → P4.1.09 → .10 → A4.** Consult exact per-task predecessors above.
3. **After A4**, Project lifecycle P4.2 and Git/Lore historical readers P4.3.01/.02 can proceed in independent files. P4.3.03/.04 integrates Gateway read → H4. P4.2.01–.06 finishes current-lifecycle authority before current mutators.
4. **P4.4** separates actual backend operations: Git .06→.07→.08; Lore .09→.10→.11; Gateway .12→.13. Root/revision/excludes/checkpoint .01–.05 have specific predecessors in the table. All join at M4, which also requires P4.2.06.
5. **H4 + M4 + B4 → V4 → P4.6.01 → D4 → P4.7.01 → I4.** There are seven gates, but no gate sign-off tickets.
6. **One integration file owner** for overlapping edits to `src/runtime_inference/runtime_host_gateway_ambient_project.rs`; architectural parallel readiness does not authorize competing changes in one worktree. Don't reopen Phase 3.

### Denominator audit

| Workstream | Bounded tasks | Gate |
| --- | ---: | --- |
| P4.0 — Inventory and decisions | 4 | B4 |
| P4.1 — Canonical scope | 10 | A4 |
| P4.2 — Lifecycle / relocation | 6 | M4/I4 |
| P4.3 — Exact historical reads | 4 | H4 |
| P4.4 — Repository operations | 13 | M4 |
| P4.5 — Conditional VCS retirement | 0 planned | V4 |
| P4.6 — Source enforcement | 1 | D4 |
| P4.7 — Documentation / handoff | 1 | I4 |
| **Total** | **39** | **7 independent acceptance gates** |

**Accounting:** The earlier 35 and 27 IDs were planning drafts, not accepted/cancelled implementation steps. These 39 tasks are a provisional bounded source-based breakdown until the B4 post-A3 inventory. Thereafter any unavoidable new task requires concrete newly found owner/source, why an existing bounded task cannot contain it, its dependencies and explicit numerator/denominator reconciliation. No test-only tasks or invisible expansions.

## 7. Testing decisions and coverage matrix

Reuse high-level integration tests instead of multiplying low-level tests per helper. Only new negative coverage that proves the missing project-scope boundary earns a permanent test.

| Seam | Existing test surface | Mandatory new proof / preserved behavior |
| --- | --- | --- |
| Scope + correlation | `project_environment_tests.rs`; Archive `project_history_tests.rs` | Same repo ID but wrong nested project path fails; accepted relocation doesn't silently authorize sibling; old correlation history remains readable. |
| Exact historical file read | `project_environment_git_tests.rs`; `project_environment_lore_tests.rs` | Wrong-REL reference, sibling/subtree escape, malformed relative paths and symlink/junction cases; accepted old path, exact revision/hash and working-tree changes still work. |
| Upload + backend management | Git/Lore project tests | Git HEAD/index unaffected; snapshot remains reachable across GC; Lore managed/manual checkpoint semantics, explicit missing ID/repo refusal, upload collisions and deterministic eligible dedupe. |
| Gateway grants + I/O race | `runtime_host_gateway_project_tests.rs`; `runtime_host_gateway_ambient_tests.rs` | Wrong owner, missing grant, revoke/detach during I/O, no REL publication after loss, owner locks not held across repository operations. |
| Semantic evidence | `src/archive/project_file_attachment_tests.rs`; `src/archive/project_history_tests.rs`; History/reconciliation tests | Project correlations and file bindings survive reopen/cuts/import/repack/reconciliation; no second VCS; ordinary Archive content is unaffected. |
| Source architecture | Phase 0/2 owner guards, Pitlord source manifest and target DAG | Production Project Environment doesn't call facade, higher runtime, Import/Maintenance, or raw unauthorized backend entrypoints; negative cases fail. |

**Order:** focused compile / targeted owner + backend tests per coherent cut; full library/integration/CLI/source/doc gate at I4. Do not run every long corpus oracle on every small code move. If an integration gate fails because of unrelated preexisting test debt, record the exact baseline and relevant isolation evidence rather than claiming success or altering unrelated subsystems.

## 8. Explicit out of scope

- New repository management policies, managed Git, new project creation UX, new CLI modes, global repository manager, remote/cloud sync, branch/merge logic or Git/Lore backend redesign.
- REL/PHY semantic reconciliation redesign, History cuts, Import/settlement compression, storage reclaim, new formats or migration machinery; Phase 3 owns its current correction and later features remain roadmap work.
- External Sources transcript authority/duplication, conversation importer redesign, new file search, file indexing, rename/move usability features or new artifact lifecycle features.
- Phases 5/6 RuntimeHost retirement, Gateway architecture redesign, unrelated facade/export renaming, semantic Observations/Ego/Context Engine work.
- A generic repository adapter trait hierarchy, generalized filesystem transaction engine, stronger public auth features, background inference or periodic indexing.
- Erasing durable Warlock-named identifiers/files, or deleting `ProjectHistoryStore`/`ProjectFileStore` without evidence that an exact responsibility is obsolete.

## 9. Stop conditions, risks and evidence policy

1. **Phase 1–3 closure evidence missing:** no implementation until C-01–C-04 production source-conformance proof is complete; Phase 3 A3 was accepted locally.
2. **Changed underlying wire or canonical corpus format:** stop for explicit approval; do not silently add a compatibility bridge or converter.
3. **Unproven same-repository relocation:** refuse implicit path widening and document exact missing caller proof. Avoid a new public migration workflow in this phase.
4. **Backend mutation before validation:** reject, maintain existing error/side-effect checkpoints where safe, and test lack of unintended commits/REL publication.
5. **A supposed duplicate actually carries Archive/REL semantic historical authority:** retain it. Deletion requires evidence, not a task-count target.
6. **Source scan unavailable:** rely on direct source/consumer/negative-guard tests and report the gap; do not claim Arcana verification or rerun the known-broken manifest ad infinitum.
7. **New foundational seam discovered:** first analyze against P4.1/P4.2/P4.3 scopes; record specific work rather than hiding indefinite effort inside one ticket or adding arbitrary acceptance counts.
8. **Branch/worktree policy:** Phase 3 auxiliary worktrees were archived and retired. Do not recreate or merge stale Phase 3 checkouts, push/rebase, or stage unrelated files as part of Phase 4 planning.

## 10. Completion gate and Phase 5 handoff

Phase 4 is complete when:

- Lore/Git is the sole authority for project-file tree/history; Archive remains the sole authority for semantic correlations and exact repository references;
- every repository-backed historical read and every repository mutation uses the one validated Project Environment scope, with matching backend, actual repository ID and accepted project subtree;
- no wrong-project/same-repository alias grants file bytes or side effects, and legitimate enrolled historical paths remain meaningful after approved relocation;
- all obsolete ID-only checks and duplicative internal call routes are deleted; no Project Environment -> facade/runtime upward edge remains;
- expected Git/Lore management behavior, error order, REL publication, historical evidence and mounted grants are preserved;
- existing required tests, negative authorization cases, full integration and source/docs gates pass with traceable receipts; and
- any phase-specific working trees, detached drafts and integration branches have been reconciled against the accepted main source: uncommitted changes are either integrated, explicitly superseded with evidence, or independently archived and referenced; no uncleared scratch work is counted complete;
- no unrelated Phase 3/5/6 features, file-format conversions, new inference work, or gratuitous extra tests have been introduced.

**Handoff to Phase 5:** Gateway continues to own mounted execution, grants and post-I/O delivery, but its repository operations receive a bounded Project Environment service rather than raw paths and direct backend access. Phase 5 may simplify runtime orchestration without reopening repository authority.

## 11. Source and decision references

- Campaign: `docs/dependency-convergence-architecture-refactor-2026-10-21.md`; `docs/dependency-convergence-target-dag-2026-10-21.md`; `docs/dependency-convergence-phase0-2026-10-21.md`.
- Phase 3 handoff: `docs/dependency-convergence-phase3-history-maintenance-import-spec-2026-10-22.md`, P3.11; authoritative Phase 3 execution ledger (not a live Phase 4 status feed).
- Decisions: ADRs `0027`, `0028`, `0037`, `0038`.
- Current project implementation: `src/project_environment.rs`, `src/project_environment/**` and Gateway ambient project/lifecycle modules.
- Durable semantic records: `src/archive/project_history_model.rs`, `project_history_store.rs`, `project_file_binding_store.rs`, `file_store.rs`.
- Tests: `src/project_environment*_tests.rs`, `src/runtime_inference/runtime_host_gateway_project_tests.rs`, `src/archive/project_file_attachment_tests.rs`, `src/archive/project_history_tests.rs`.

**This document is a planning specification and task design, not an implementation receipt.**
## Related docs

- [Phase 1–3 dependency closure audit](dependency-convergence-phase1-3-closure-audit-2026-10-23.md) — C-01–C-04 must be resolved before Phase 4 implementation.
- [Phase 3 worktree integration audit](reliquary-phase3-worktree-audit-2026-10-23.md) — precedent for proving parallel work is incorporated before acceptance.
- [Phase 3 to Phase 4 handoff](dependency-convergence-phase3-phase4-handoff-2026-10-23.md).
- [Dependency convergence architecture campaign](dependency-convergence-architecture-refactor-2026-10-21.md).

## Notes

Phase 4 remains **specified, not implemented**. The cleanup of historic Phase 3 worktrees does not satisfy C-01–C-04 or close any Phase 4 execution gate. Existing 39 bounded implementation tasks and seven gates are unchanged.
