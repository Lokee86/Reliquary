# Project Environment crate extraction — implementation specification

**Status:** Specified for correction-only migration; **not implemented by this document** (2026-10-28).
**Target:** Existing placeholder `crates/reliquary-project-environment` (package `reliquary-project-environment`).
**Repository / branch:** `C:\!bin\workspace\Reliquary-crate-first` / `refactor/crate-first-restart`.
**Baseline HEAD:** `1a329197` — Context Engine acceptance evidence committed.
**Preserve:** Pre-existing unrelated modification to `docs/graph-crate-extraction-evidence-2026-10-26.md`; do not stage, discard or rewrite it.
**Mandate:** Extract working Git/Lore project environment code. Do **not** create a new service, repository format, Git/Lore abstraction framework, background worker, project VCS, user-facing capability, or provider operation.

## 1. Current reality and completion criterion

`reliquary-project-environment/src/lib.rs` is comment-only. The real implementation lives in `src/project_environment.rs` and `src/project_environment/*.rs` (16 submodules, approximately 1,500 lines), with Root/Gateway callers and tests. The target crate is therefore a **genuine existing-code move**, not an activation or new-service implementation.

The focused pre-migration baseline completed: `cargo test --lib project_environment --locked --offline -j 1 -- --test-threads=4` with short repository-local `TEMP`/`TMP` **18 passed, zero failed** (Git, managed/manual Lore, adoption, identity conflicts, uploads and historical reads). Retain those exact Root behavior fixtures; their caller APIs remain unchanged.

**Done only when** the crate compiles and owns all existing backend operations; Root/Gateway uses those operations with no parallel implementation; project REL authorization/creation/correlation remains in Root; existing Git/Lore tests and the full product/CLI matrix pass; documentation and commit are complete. No empty or unused target crate qualifies.

## 2. Explicit source and dependency crosswalk

| Current source | New owner or retained authority |
| --- | --- |
| `src/project_environment.rs` | Extract `ProjectRepositoryMode` and `from_correlation` to new crate using Archive-owned value types; extract `PreparedProjectAttachment` as the same data shape. Keep a *thin Root shim* only for Root `lifecycle` and direct compiled-crate re-exports used by existing private callers. `ProjectAdoptionSource` is a Root-facing lifecycle value and may remain in Root to avoid unnecessary API churn. |
| `git.rs`, `git_command.rs` | **Move** current Git root discovery, repository identity establishment/validation, reserved exclusions and invocation mechanics byte-for-byte where possible. |
| `git_snapshot.rs`, `git_upload.rs`, `git_file.rs` | **Move** existing exact historical Git snapshots/refs, tracked-file reuse, staging/index/head preservation, upload materialization, path and content-hash validation. |
| `lore.rs`, `lore_repository.rs`, `lore_runtime.rs` | **Move** current Lore creation, root lookup, revision discovery, FFI/runtime invocation, ignore handling and rollback of prepared repository state. Preserve existing Lore APIs and errors; no Lore redesign. |
| `lore_checkpoint.rs`, `lore_upload.rs`, `lore_file.rs` | **Move** existing explicit manual Lore checkpoint, managed/manual upload, hash/revision checks and exact historical file reads. |
| `attachment.rs`, `file.rs`, `upload_paths.rs` | **Move** immutable Archive-correlation dispatch, VCS file reads, upload path/name/MIME/collision decisions and excluded-path selection. |
| `repository.rs` | **Split only at the concrete `Cva` seam.** Move backend preparation, current revision, root resolution and manual Lore checkpoint into the new crate. Its `PreparedProjectRepository` carries the same revision, management and rollback state with narrow accessors. Move its existing `correlate(&mut Cva)` and `validate(&Cva)` logic to Root orchestration, not a dependency on Root or a second durable-store writer. |
| `lifecycle.rs` | **Remain Root**: `Cva::create_project`, `Cva::open`, source owner validation, REL target admission, correlation publication, `sync`, rollback/delete on failed bootstrap and post-create validation. Import the new crate's prepared repository result; no owner lifecycle in the VCS crate. |
| `src/runtime_inference/runtime_host_gateway_project_lifecycle.rs` | **Remain Gateway/Root**: mount admission, gateway lifecycle lock, owner grants and Cva lifecycle; call Root lifecycle shim and new crate repository operations. |
| `src/runtime_inference/runtime_host_gateway_ambient_project.rs` | **Remain Gateway/Root**: `ManageProject` grant/lease/revalidation, source authorization, `with_rel_mutation`, correlation and `register_project_file`, owner notification and delivery. The VCS crate performs existing authorized Git/Lore I/O on supplied immutable inputs only. |
| `src/lib.rs`, `crates/reliquary-archive/src/project_history_model.rs` | Root retains `ProjectAdoptionSource`/public Gateway API; `ProjectRepositoryMode` is re-exported from compiled crate. Archive remains canonical owner of `ProjectRepositoryKind`, `ProjectRepositoryManagement`, `ProjectRepositoryRef`, `ProjectRevisionRef`, `ProjectFileRef`, `ProjectRevisionCorrelation` and the exact durable representations. **No duplicate models.** |
| `src/project_environment_*_tests.rs`, Root/Gateway authorization/project tests | **Remain in Root** for real REL/PHY, Gateway and VCS end-to-end tests. Add only minimal crate-level external-consumer tests where needed to verify imported public entry points independently; do not duplicate the 18-case integration suite. |

The source move should delete displaced original backend files once Root re-exports the crate. `lifecycle.rs` can be moved to a clearer Root-only location if justified; do not leave two live copies or copy the 16 existing modules merely to call them from the placeholder.

## 3. Compiled and physical authority rules

1. `reliquary-project-environment` may import **`reliquary-archive` for existing project reference/correlation values**. `reliquary-owner-lifecycle` is permitted by `docs/architecture/crate-map.json` but not needed for the present backend behavior; do not add it by default. External dependencies presently required by the code include `lore`, `uuid`, `sha2` and any actual executor dependency needed for existing `lore_runtime.rs`; add only those used. Root depends downward on the project crate. The project crate must not depend on Root, Gateway, Container, History, Import, Maintenance, Ego, or a mutable REL/PHY store.
2. **Git/Lore are authoritative for project revisions, trees, snapshots and content.** Project Environment performs their *existing* VCS/filesystem side effects, not semantic Memory/Archive publication. It must not persist another project-history database in REL, invent a Git shim with independent data, or introduce additional checkpoint policy.
3. **Archive is authoritative for typed correlation and project file reference values; Root/Cva is authoritative for durable REL project association and registered semantic file records.** Gateway owns grants and owner token revalidation. Keep the external I/O outside the owner lock exactly as today. No backend function receives mutable `Cva` or Gateway handles.
4. Preserve `ProjectRepositoryMode::from_correlation` mapping: Lore+WarlockManaged → ManagedLore; Lore+External → ManualLore; Git → Git. Preserve current explicit checkpoint gate for manually managed Lore and the current exception allowing attachment ingestion.
5. Preserve repository identity checks: reject mismatched/missing Git identity, mismatched Lore repository ID, adoption of an already-correlated REL, changed source owner and invalid project paths. Never silently retarget a project or create a replacement repository during a read.
6. Preserve Git branch and index immutability during upload/snapshot; exact tracked-content reuse; `refs/warlock/snapshots` persistence through edits/deletes/GC; nested-project subtree restriction; `warlock/uploads` vs generated exclusions; MIME and collision handling; reserved `.git/info/exclude` behavior.
7. Preserve Lore managed/manual bootstrap and checkpoint behaviors; reserved `.loreignore` handling; historical file access; exact file hashes, revision identity and read failures; rollback only on the existing conditions; no new Lore format/compatibility machinery.
8. Preserve existing public signatures and string error contracts unless a narrowly necessary ownership type change is explicitly recorded and all product call sites are adjusted in the same extraction. Do not introduce parallel `ProjectRevisionRef` IDs or new adapters for long-term compatibility.

## 4. Bounded execution plan

| Gate | Work | Independent acceptance |
| --- | --- | --- |
| **P0 — freeze baseline** | Confirm HEAD/worktree, current Root public and Gateway calls, project type definitions, Git/Lore pins, test list and 18-case baseline. | Report exact commit and dirty Graph evidence; no product changes. |
| **P1 — stable compiled contract** | Add dependencies to the reserved crate; move the Archive-based repository mode and prepared attachment models. Establish only required consumed public entry points; Root public re-export remains identical. | `cargo check -p reliquary-project-environment --locked --offline -j 1`; Root check; public `ProjectRepositoryMode::from_correlation` parity. |
| **P2 — Git backend** | Move `git_command`, `git`, `git_snapshot`, `git_upload`, `git_file` and their existing behavior. Move shared hash/path logic as needed without duplicate implementations. | Existing Git tests, snapshot/index/head, nested path and immutable historical bytes. |
| **P3 — Lore backend** | Move `lore_runtime`, `lore_repository`, `lore`, `lore_checkpoint`, `lore_file`, `lore_upload`, `upload_paths` and shared dispatch. | Existing managed/manual Lore tests; read/mismatch/retry conditions and no unwanted new revision. |
| **P4 — hard Root ownership split** | Move backend portions of `repository.rs`, `attachment.rs` and `file.rs`; keep `Cva` correlation/validation and `lifecycle.rs` in Root. Update Gateway callers/Root public re-exports. Delete every displaced duplicate backend file. | Single compiled implementation; no project crate → Root edge; all 18 project tests and Gateway authorization checks. |
| **P5 — verify & commit** | Run matrix below, update docs/source map and evidence, commit only complete migration. No new service construction. | Green acceptance with exact counts and independent unresolved gates recorded; Graph evidence edit unchanged. |

Each gate is an observable source/consumer transition, not a synthetic task explosion. Keep modules small (target under 200 lines; none above 300). Avoid generalized `ProjectEnvironment` trait families, facade mirrors, automatic scheduler/queue abstractions and new tests for unrelated behaviors.

## 5. Required final acceptance matrix

- `cargo check -p reliquary-project-environment --locked --offline -j 1` and `cargo test -p reliquary-project-environment --locked --offline -j 1`; one genuine external-consumer contract through the new crate for repository modes and a backend operation, not a constants-only smoke test.
- `cargo test --lib project_environment --locked --offline -j 1 -- --test-threads=4` (**baseline 18/18**). Also run relevant `runtime_host::gateway::ambient`/project lifecycle authority tests and Root Echo/Archive tests if relevant integration changes occur. Use a short writable repository-local `TEMP`/`TMP`/ `TMPDIR` on Windows.
- `cargo check --workspace --all-targets --locked --offline -j 1`, complete Root `cargo test --lib --locked --offline -j 1 -- --test-threads=8`, CLI `cargo test --manifest-path cli/Cargo.toml --locked --offline -j 1`, and `cargo fmt --all -- --check`. Full Root testing includes an unrelated expensive 20,000-receipt Freshness fixture; allow it to complete (prior verified full suite: 1,457 passed, 5 ignored); do not call a filtered/unfinished run green.
- `cargo tree -p reliquary-project-environment --depth 1 --offline`; verify Archive as the only *internal* direct dependency (unless another was source-justified), with no project-crate → Root or owner store backedge. Inspect `rg`/Git change detection to ensure one owner for backend operations and unchanged Archive project data types.
- Verify Git/Lore error strings, exact stored project-file bytes and `ProjectRevisionRef` metadata on real fixture bootstrap, adoption, upload, checkpoint, later edit/delete, replay/reopen, identity mismatch and nested-project case. Ensure Git branch/index never unexpectedly moves; ensure failures do not publish a Memory or file record.
- Update `docs/subsystems/project-environment.md`, `docs/api.md`, `docs/architecture.md`, `docs/maintainer-map.md`, `docs/crate-first-layout-spec-2026-10-25.md`, `docs/architecture/crate-map.json` (only if the permitted edge contract genuinely changes) and `docs/INDEX.md` for code **actually moved**, not planned.
- Run `ddocs check --indexes --links`, `git diff --check` and standard architecture guards. **Fresh Lexicon/Arcana graph verification has a pre-existing scanner timeout on this repository** after Context Engine; do not repeatedly spin the expensive scan to manufacture acceptance, disable ownership rules or mark a stale graph green. Record the limitation separately from compile-enforced dependency checks.
- Record exact baseline/final commit, source ownership, test results, known limits and unchanged unrelated worktree items in `docs/project-environment-crate-extraction-evidence-2026-10-28.md`. Stage only intended changes. Do not reformat or commit the unrelated Graph evidence modification.

**Definition of done:** existing VCS and project-file behavior has a single compiled `reliquary-project-environment` owner; Root/Gateway remain the sole REL lifecycle/authorization/publication owners; no new semantics are introduced; the code is actually consumed, verified and committed.

## 6. References

[Project Environment subsystem](subsystems/project-environment.md), [crate-first architecture](crate-first-architecture.md), [crate map](architecture/crate-map.json), [crate-first layout](crate-first-layout-spec-2026-10-25.md), [ADR 0027](decisions/0027-warlock-project-repositories-and-reliquary-storage-boundary.md), [ADR 0028](decisions/0028-project-folder-and-repository-bootstrap-contract.md), [ADR 0037](decisions/0037-reliquary-context-memory-project-environment.md), [behavior contracts](behavioral-contracts.md).

**Specification only:** no implementation, feature, Git/Lore data migration or durable format change in this commit.
