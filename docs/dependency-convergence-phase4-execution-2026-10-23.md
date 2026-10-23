# Phase 4 execution receipt

Date: 2026-10-23
Repository: `Reliquary` at `C:\!bin\workspace\Reliquary`
Status: Phase 4 implemented locally; full Rust/CLI and bounded refreshed architecture acceptance passed, with the verification gaps below. Gate evidence and limitations are recorded below, without claiming unavailable checks.

## Controlling inputs and bounded reading

- Phase 4 spec: `docs/dependency-convergence-phase4-project-environment-spec-2026-10-23.md`, SHA-256 `305D92F1D7BD4A40B34E7B2A815317B226BDDC963B65008DA096A2663DDA60C0`.
- Phase 3 to Phase 4 handoff: `docs/dependency-convergence-phase3-phase4-handoff-2026-10-23.md`, SHA-256 `BFD744B70737F12BA8E7A52084195A98A5BE004DFCC8A426ABE1D0F2F701B5EC`.
- Repository `AGENTS.md` and bounded portions of `docs/architecture.md`, `invariants.md`, `storage-format.md`, `maintainer-map.md`, `current-limitations.md`, `roadmap.md`, `version-history-plan.md`, ADR-0002, ADR-0027 and ADR-0028 were read. These establish downward ownership, the distinction between repository history and semantic history, and complete-folder relocation as a supported scenario.
- The handoff accepts the bounded C-01–C-04 source-contract closures, with C-02's 82 unresolved cycle-forming imports assigned to the existing Phase 6 facade correction. This is not a complete symbol-level production graph proof. The source-file-only Arcana graph cannot prove the full component SCC; retain that gap for architecture acceptance. The known Arcana wrong-field-count failure is not a reason to claim a graph pass or repeat broad scans.

## Source fingerprint baseline

SHA-256 fingerprints below identify the clean pre-execution HEAD source snapshot, verified against `git show HEAD:<path>` to avoid concurrent working-tree inventory drift. These are inventory fingerprints, not a claim that each source must change.

| Source | SHA-256 |
| --- | --- |
| `src/project_environment.rs` | `69BA6964B9F36267E1A05DE10A6B6574758E2E274EE4290D539EAD0C4689EF48` |
| `src/project_environment/file.rs` | `4B02124DDBA6B1CC5AD67B53F6C1CC8F2536A002722485FCC75D623C94AACA37` |
| `src/project_environment/git_file.rs` | `F77A6D53B8142F2B9FABD84FB081E51CA5D3D789012731C2C42654ADEB3BCBA6` |
| `src/project_environment/lore_file.rs` | `6B0D22F4DB25FC833BA7C91EDEA01692419FB9599528CA98DA725BBE12F5DA73` |
| `src/project_environment/repository.rs` | `785D62CDADF8F87FC48CD6E78FA91A8B913B379014A3F8C8B736B06868E03637` |
| `src/project_environment/git_upload.rs` | `B6AE0A9B8BAB6586C3C1C0F161FFDEA9DA7607C2350DAB7258CAB56B9F9AB059` |
| `src/project_environment/attachment.rs` | `27A25EFAC50498FC9995917E9BD904FE3B0D69630D19756391BB21F14F0B3AA4` |
| `src/project_environment/lore_upload.rs` | `3EA626FF27E7E9CED0E66D69B0897A8E38F977B202AE38DC62D2FDA93D224BD0` |
| `src/project_environment/lore_repository.rs` | `5F460A0BDA6DF9F739A5DEA3774471D9CE5920C632FBDCEA2F2E534B8CE20A0C` |
| `src/project_environment/git.rs` | `BE2423F8B49D0A8A41ED1AE51E3562C347F8EE23E3FB094DC0EB16FEFB6FC42C` |
| `src/project_environment/upload_paths.rs` | `99A2E9639ED4662E2921EE7D5313203E011DDC63925333FFDED4D91C08F2E9AC` |
| `src/runtime_inference/runtime_host_gateway_ambient_project.rs` | `8251B7DD2E09E7DDA4CD6FD1FFE155EA39E5CAE61C712BB0248F7129C216EB15` |
| `src/runtime_inference/runtime_host_gateway_project_lifecycle.rs` | `A97C292B19B8CE758EC10C0CE8851445FD917263FEBF8CCD5163405D1018B784` |
| `src/archive/project_history_model.rs` | `4D15F3105145AA68A4F82D748B303D47955A1CC957078665FB06AD1854CFA04F` |
| `src/archive/project_history_store.rs` | `5DABB73A67EF4A3C546ACD5746C7F900E175868C39F5BDA9D2E0D4ABC90503BA` |
| `src/archive/project_file_binding_store.rs` | `D6FE41A2AE138C28A6B3D51BE1C80625D0E826BF8FBD6421A6C2EBAEBD6050B9` |
| `src/archive/file_store.rs` | `A83AF801F31FFA5EED2CC380F02C1F7D62EC1532713A00046D5D4A2906028431` |

## Baseline caller inventory and boundary

Production Project Environment entrypoints are called by `src/runtime_inference/runtime_host_gateway_ambient_project.rs`: `repository_root`, `current_revision_for`, `checkpoint_manual_lore`, `prepare_upload`, and `read_project_file`. `src/runtime_inference/runtime_host_gateway_project_lifecycle.rs` also calls `current_revision_for` during lifecycle validation. These calls are the integration surface; Gateway still owns grants, owner selection, and authorization revalidation after I/O.

The backend dispatch is in `project_environment/file.rs` and `attachment.rs`, with Git-specific reads/uploads in `git_file.rs` and `git_upload.rs`, Lore-specific reads/uploads in `lore_file.rs` and `lore_upload.rs`, and repository resolution/checkpoint logic in `repository.rs`. `lore_repository.rs`, `git.rs`, and `upload_paths.rs` participate in backend discovery and path preparation. The public(crate) `project_environment` exports keep these helpers within the Rust crate.

Bounded search of `cli/`, `tests/`, and production sources found no CLI command calling Project Environment repository/read/upload helpers. The detachable CLI still has independent Cargo verification obligations if the public library signature or CLI source changes. The tests include one direct helper read in `runtime_host_gateway_ambient_tests.rs`; retain/update it with the read API migration.

## VCS duplication disposition

No duplicate VCS implementation is established by this inventory. `ProjectHistoryStore` stores append-only `ProjectRevisionCorrelation` records tying a REL semantic cut to an exact Lore/Git revision and management mode. `ProjectFileStore` stores immutable `FileId -> ProjectFileRef` bindings; Archive file identity, source attachment, and ordinary Archive-owned content remain semantic/provenance responsibilities. Import, history replay, reconciliation, rewrite, and repack consume these facts. None of this is repository tree/blob ancestry, branches, rename history, merge, or filesystem reconstruction. Preserve Archive history and these stores. Reconsider deletion only if an exact duplicate tree/history responsibility and its consumers are independently demonstrated.

## Scope and relocation proof

Current authorization checks can establish repository identity but do not consistently bind a caller-selected folder or historical file path to the owning REL's independently accepted project-path correlation. A same-repository ID cannot authorize a sibling subtree. Historical references must be checked against the owning REL's accepted correlation history, including the old subtree for reads; current writes require the current accepted subtree.

ADR-0028 supports moving or renaming the complete project folder. A whole-checkout move that preserves the repository-relative selected project path is supported by the durable identity model: machine-local root changes while `(backend kind, repository ID)` and the accepted relative subtree remain stable. The present lifecycle caller can observe a selected path and repository ID, but the inspected source does not supply proof that a changed repository-relative subtree is the same moved project rather than a sibling. Therefore fail closed on relative-subtree mismatch until an existing owner/mount correlation plus applicable explicit management authorization proves relocation. Do not silently append a new correlation from repository-ID equality alone. This is the missing bounded caller contract for the relocation gate.

## Focused existing tests and backend constraints

Primary existing acceptance seams:

- `project_environment_tests.rs`: `managed_lore_bootstrap_records_repository_correlation`, `manual_lore_checkpoint_is_explicit_and_updates_revision`, `git_bootstrap_preserves_head_and_index_and_assigns_stable_identity`, `project_adoption_preserves_owner_and_existing_rel_content`, `lore_attachment_reference_reads_exact_historical_bytes`, `git_attachment_snapshot_does_not_move_head_and_reads_exact_bytes`, `repository_identity_mismatch_is_rejected`.
- `project_environment_git_tests.rs`: `git_upload_reuses_exact_tracked_file_without_branch_or_index_mutation`, `git_upload_snapshot_preserves_branch_index_and_snapshot_ref`, `git_historical_upload_survives_edit_delete_and_gc`, `git_upload_requires_existing_repository_identity_without_reassigning_it`, `git_nested_project_upload_stays_inside_project_subtree`.
- `project_environment_lore_tests.rs`: `lore_upload_reuses_exact_tracked_file_without_new_revision`, `lore_upload_requires_existing_repository_without_replacing_it`, `lore_new_upload_creates_revision_and_exact_historical_file`, `lore_nested_project_upload_stays_inside_project_subtree`, `lore_historical_read_survives_edit_delete_and_fails_closed_without_repository`, `lore_upload_filename_collision_allocates_deterministic_sibling`.
- `runtime_host_gateway_project_tests.rs`: `project_families_require_the_exact_owner_and_applicable_grant`, `project_io_races_reauthorize_before_commit_or_delivery`.
- Archive preservation: `project_history_tests.rs` and `project_file_attachment_tests.rs`, including reopen, idempotence, exact binding, attachment, repack, and divergent-reconcile cases.

Git constraints: preserve user HEAD and index; Warlock snapshots remain reachable across GC; no bootstrap commit; retain `.warlock/` exclusion and existing `refs/warlock/snapshots` and upload markers. Lore constraints: managed/manual mode semantics, explicit manual checkpoint, exact historical revision reads, deterministic upload collision handling, and existing repository identity refusal. Both backends must reject sibling/subtree escape before repository or semantic mutation. Gateway must retain pre-I/O and post-I/O grants and avoid holding owner locks over repository operations.

## Baseline source-guard requirements

The final source enforcement should assert the target boundary after call migration:

1. `project_environment` production modules have no `Cva`, facade, Gateway/runtime-host, Import, or Maintenance dependency, and lifecycle/correlation publication does not construct or mutate a facade-owned Cva.
2. All production Git/Lore historical reads, current revision/root operations, uploads, and checkpoints enter through the validated scope; direct backend helpers cannot accept ambient unvalidated roots/correlations as authority.
3. Gateway's five ambient calls and lifecycle revision check pass the accepted correlation/history into that scope; the historical read also carries the owning REL association, not only a reference and local directory.
4. Keep explicit negative fixtures for wrong same-repository subtree, unaccepted historical path, malformed relative path, and repository replacement. Preserve existing Gateway post-I/O authorization-race assertions and Git HEAD/index preservation assertions.
5. Archive project correlation/reference stores remain classified as semantic Archive owners; do not add a guard that treats their use of `ProjectFileRef` as VCS ownership.

At the initial read-only inventory stage, no implementation gates had run. The final implementation and acceptance evidence below supersede that inventory-stage status; the inventory itself is not a graph or completion proof.

## Purpose

Record the baseline, implementation and actual acceptance evidence for Phase 4 without changing prior phase denominators.

## Overview

B4 retains the 39 bounded tasks and seven gates. Source implementation is complete locally; the acceptance record distinguishes passing checks, corrected intermediate failures and unavailable proof.

## Related docs

- [Phase 4 specification](dependency-convergence-phase4-project-environment-spec-2026-10-23.md)
- [Project Environment](subsystems/project-environment.md)
- [Phase 3 handoff](dependency-convergence-phase3-phase4-handoff-2026-10-23.md)

## Notes

A full symbol-level production graph proof remains unavailable. No wire-format or prior-phase denominator change is authorized by this receipt.

## Integrated implementation

The 39-task denominator is unchanged. P4.0 establishes the accepted bounded prerequisite receipt, exact callers, no duplicate VCS deletion and the fail-closed relocation contract. P4.1 introduces `ValidatedProjectScope`, structural project/file path validation, current backend/ID/subtree binding, accepted historical enrollment and physical containment. P4.2 moves actual REL creation/open/publication into upper public lifecycle composition and removes lower `Cva` dependencies. P4.3 migrates both readers and the Gateway owning-binding/history capture. P4.4 migrates current operations, restricts candidate reuse, guards upload materialization, preserves Git snapshot refs/index/HEAD, and rechecks correlation before semantic publication. Routine revision/root validation is read-only. Explicit Lore checkpoint/attachment ignore maintenance is scope-authorized.

Windows materialization pins ancestor directories and final files against rename/reparse/write sharing. Unix uses `openat`/`mkdirat` with retained directory descriptors and `NOFOLLOW`; the Unix implementation has not been compiled on this Windows-only host. The small OS helpers are current-project file mechanics, not a VCS registry or global lock manager. Newly created upload files use exclusive creation and remain held through snapshot/staging. Backend repositories and all durable formats are unchanged. Temporary Git index and Lore read output stay in the active backend repository.

P4.5 retains Archive project correlation, file binding and ordinary embedded payload stores: no duplicate VCS was proven. Superseded ambient dispatcher signatures, ID-only reference helpers, lower lifecycle implementation and implicit Gateway recorrelation were removed. P4.6 extends the existing owner checker and platform ownership map; the multiplexing coverage checker now recognizes validation as a grant-checked read and rejects mutation there. P4.7 updates architecture, API, subsystem, limitations and maintainer ownership documentation.

Machine-readable [source and acceptance receipt](../benchmarks/dependency-convergence-phase4-acceptance-2026-10-23.json) pins the changed-source hashes and disclosed check status.

## Acceptance record

Full Rust/CLI acceptance and bounded refreshed architecture/convergence acceptance passed. The unavailable checks below remain explicit gaps. A passing source guard is not a full symbol-level graph proof. The task denominator and historical Phase 1–3 evidence remain unchanged.

| Check | Result |
| --- | --- |
| Final settled `cargo check --locked` and formatting | Passed; existing warnings |
| Owner-boundary self-test and production guard | Passed after platform modules were given distinct names |
| Physical boundary self-test | Passed; thirteen fixtures |
| Read-only History boundary self-test | Passed |
| Multiplexing coverage fixtures | Passed; 96 fixtures and 166 classified APIs |
| Component target/graph self-test | Passed; target/fixture evidence only |
| Settled source architecture / convergence | Passed after exact ledger reconciliation: 25 base rules; 4,626 classified convergence findings; no new or stale debt |
| Documentation policy, full and changed-from | Passed; final receipt check also passed |
| Initial broad Cargo test attempt | Failed with Rust internal compiler errors / missing intermediate rlib artifacts; retried serially without incremental compilation |
| Archivist integrity | Passed after excluding generated folders through `.docignore` and refreshing the documentation index; initial scan hit preexisting denied scratch state |
| `pitlord docs` | Unavailable: both installed and repository-built executables lack the required `docs` subcommand |
| Detachable CLI formatting / locked check / locked tests | Passed; 32 tests passed, none ignored |
| Final focused Project Environment tests | Passed; 25 tests passed, none ignored |
| Final full locked Rust gate | Passed: 1,999 library + 163 integration tests; zero failed; six ignored (five library, one integration); doctests passed |

The exact local verification logs are under ignored `.test-tmp/phase4-*`. The six ignored cases are named in the machine receipt: retained historical corpus, semantic release measurement, frozen agentic fixtures, two crash subprocess helpers, and promoted six-file corpus admission. None was silently counted as a pass. This bounded Project Environment change does not affect the graph-corpus smoke path.

## Corrected verification findings

The initial focused run exposed Git's rejection of extended Win32 paths in `GIT_INDEX_FILE`; the Windows environment value now uses Git-compatible drive/UNC syntax. A subsequent run showed Git's replace-by-rename index machinery needs a writable backend metadata directory while project ancestors stay pinned. Temporary indexes now use Git's own `rev-parse --git-path` resolution inside repository metadata; protections were not weakened. The established repository identity mismatch wording was restored. All 25 project tests pass after these corrections.

One intermediate broad run recorded 1,991 passes, six Git snapshot failures and five ignored tests, with two long-running unrelated cases not completed when the known-failing run was interrupted. It is not a final acceptance run. The final broad gate uses serial compilation, no incremental compilation and a root-package debug-information override to reduce Windows linker cost; assertions and dependencies are unchanged.

## Settled graph and exact ledger reconciliation

The frozen original Phase 0 graph/captures and convergence policy are unchanged. The settled refresh is `sha256:7fc9afe72d7cd05c7beac8193e59c6342348b3c8d9458b293f86d1a4db10dab7`; Arcana sync completed with zero compatibility warnings. The unbaselined architecture policy reports no violations across 25 rules, 105,932 source nodes and 9,196 relationships. This source graph is not the missing full symbol-level 34-component production proof.

The convergence checker first correctly rejected eight stale findings and fourteen unclassified findings. Both finding ledgers were then reconciled using that exact report: eight resolved production lifecycle/correlation calls retired, thirteen new acceptance-test findings classified test-only, and one exact false local `visit` target resolution classified analyzer-artifact. Source review and Luna independently confirmed the helper is local to `upload_paths.rs`, with no Freshness call/import. The remaining Gateway `Cva` import is retained and reviewed as the supported public signature value type. No new production dependency was waived. Total classified findings change from 4,620 to 4,626; source-confirmed Phase 4 production call debt falls by eight. The machine receipt records exact entries, before/after hashes, classifications and snapshot provenance. No baseline recapture or policy expansion occurred. The final architecture command passed, including the convergence engine: 21 convergence rules, 91,516 source nodes, 245,202 relationships and no new or stale debt.

## Documentation impact

- Inspected: repository guidance, controlling specification/handoff, Architecture, invariants, storage format, maintainer map, limitations, roadmap and relevant ADRs.
- Updated: Architecture, Project Environment subsystem, API, limitations, maintainer map, specification status, execution/source receipt, documentation index and generated-state `.docignore`.
- Not affected: REL/PHY and project reference encodings, semantic owner history, external sources, later Runtime/facade redesigns and shared engineering standards.
- Compliance check: full and changed-from documentation policy and Archivist integrity passed; installed and repository-built Pitlord lack the required documentation subcommand.
- Known documentation gaps: automated Pitlord documentation-ownership enforcement remains unavailable on this host.

## Architecture impact

- Standards added or changed: repository-local Project Environment scope/backend guard and read-only Gateway validation fixtures; no shared standard or target-DAG edge was added.
- Ownership or boundary impact: Project Environment owns validated repository/subtree operations; upper lifecycle owns REL creation/open/publication; Gateway keeps mounted grants and post-I/O admission; Archive semantic evidence is retained.
- Pitlord enforcement impact: explicit current platform/source ownership; exact obsolete Cva deferred imports retired; original Phase 0 graph/captures retained. Final source graph/convergence outcome is recorded in acceptance evidence.
- Other verification impact: 2,162 full Rust tests and 32 CLI tests passed, zero failed; six full-suite cases explicitly ignored. All 25 focused project cases passed.
- Known architectural gaps: full symbol-level 34-component production graph proof remains an inherited unverified obligation; Unix guard implementation is not compiled/tested on this Windows host.

## Seven-gate disposition and Phase 5 handoff

| Gate | Local disposition |
| --- | --- |
| B4 | Accepted bounded prerequisite/inventory receipt; four inventory tasks; original 39-task denominator retained |
| A4 | Scoped authority and Windows path/identity refusal coverage passed; Unix platform verification remains unavailable here |
| H4 | Exact historical reads, historical enrollment and owning-binding/grant checks passed |
| M4 | Lifecycle/current operations, Git HEAD/index/ref preservation, Lore upload/checkpoint and post-I/O authorization tests passed |
| V4 | No duplicate VCS proven; semantic Archive stores retained; obsolete ambient/lower lifecycle paths retired |
| D4 | Refreshed source policy and exact convergence ratchet passed; full symbol-level production component proof remains unverified |
| I4 | Locked Rust/CLI and documentation policy/integrity passed; Pitlord documentation ownership remains unavailable |

Phase 5 receives the supported public Gateway lifecycle signatures and the private `ValidatedProjectScope` seam: upper `cva_project_environment` composes REL lifecycle; Gateway captures owner binding/current history and grants; Project Environment performs bounded scoped I/O; Archive owns durable semantic correlation/binding evidence. Phase 5 must preserve those responsibilities and the exact accepted-correlation publication recheck. No Phase 5 runtime implementation is included. Work is local and uncommitted.

A concurrent edit to the Phase 5 planning specification was observed and preserved. The Phase 4 diff passes whitespace checks; the whole-worktree check separately identified two trailing-space lines in that concurrent document.
