# Reliquary 2.1 operator workflow

Build the standalone CLI with `cargo build --manifest-path cli/Cargo.toml --locked`.
The CLI has one Reliquary dependency, `reliquary-memory`. Successful one-shot
reports go to stdout as JSON. Errors and import-plan diagnostics go to stderr.
`--json` is accepted for machine-oriented invocations.

## Configure and stage

Use an explicit [runtime configuration](configured-runtime.md). The check is
offline by default. `--online` runs synthetic probes through the selected
provider transports. For an existing protected `.cfg` file, also pass
`--master-key PATH`. Both arguments are paths. Credential material is read
locally and stays out of CLI arguments and reports.

```text
reliquary config check runtime.json
reliquary rel create project.rel --scope project
reliquary phy create user.phy
reliquary rel import project.rel --source conversations.json --format chatgpt --phy user.phy --dry-run
reliquary rel import project.rel --source conversations.json --format chatgpt --phy user.phy
reliquary rel process project.rel --import IMPORT_ID --phy user.phy --config runtime.json --until ready
reliquary rel verify project.rel
reliquary phy verify user.phy
```

Use `--format normalized` for the known visible-turn manifest. Raw exports
accept a half-open selection with `--start` and `--end` RFC3339 timestamps.
For normalized input, the selection is already recorded in the manifest.
Coverage reports include omitted asset references and remapped parents.
`rel import --process` uses the same processing path as `rel process`.

Keep the import ID from the stage report. Use `rel work status project.rel
--import IMPORT_ID` for durable progress and `rel work resume project.rel
--import IMPORT_ID --phy user.phy --config runtime.json` to resume. An
explicit `--episode EPISODE_ID` retries a held import Episode under the
same input and lease checks. A separate `rel work stop` invocation reports
the lack of a control channel to an already running process. In-process
hosts can call `OwnerProcessingJob::request_stop` and join it.

## Find and inspect

```text
reliquary rel search project.rel --query "movement packet" --mode lexical
reliquary rel search project.rel --query "movement packet" --mode hybrid --config runtime.json
reliquary rel query project.rel --query "How does the server handle movement?" --config runtime.json
reliquary rel interact project.rel --config runtime.json
```

In the interactive process, use `query`, `refine`, `traverse`, `show`,
`source`, `neighbors`, `status`, `new-turn`, and `quit`. Shell commands
take plain text or a quoted JSON string. `--commands PATH` reads JSONL with
one reply per request. The query and refine records use `text`. Show,
source, and neighbors use `memory_id`. Traverse uses `memory_ids`.

```jsonl
{"request_id":1,"command":"query","text":"How does the server handle movement?"}
{"request_id":2,"command":"refine","text":"Focus on the user's server-side requirements"}
{"request_id":3,"command":"new-turn"}
{"request_id":4,"command":"quit"}
```

Read exact evidence with `rel memory get`, `rel memory provenance`,
`rel archive branches`, and `rel archive show`. Graph inspection supports
`graph neighbors` and `graph edge`. It accepts a Memory ID or an explicit
`entity:ID` selector. Mounted PHY Memory reads use the separate `phy` family.
Pass `--rel PATH` to `phy memory provenance` to resolve evidence from the
explicitly mounted source REL. Local CLI grants cover the owner files named
in that command and rely on the local account's filesystem access.

## Public entry points and executable evidence

| Capability | Root symbols | Evidence entry point |
| --- | --- | --- |
| Provider configuration | `ConfiguredRuntime`, `CredentialResolver`, `EnvironmentCredentialResolver` | `external-consumer/tests/configured_runtime.rs`, Switchboard loopback tests |
| Stage and resume | `NativeImportPlan`, `Instance::stage_native_import`, `native_import_status` | `tests/native_import.rs`, CLI import/reopen test |
| Processing | `ProcessOptions`, `Instance::start_processing`, `processing_status`, `process_and_drain`, `OwnerProcessingJob` | `external-consumer/tests/operational_processing.rs` |
| Passive search and query | `Instance::search`, `begin_retrieval`, `memory_query`, `refine_query`, `traverse_graph` | Gateway retrieval tests, CLI search and JSONL inspection |
| Confirmed host use | `preview_retrieval`, `deliver_retrieval`, `recover_retrieval`, `TrustedHostAdapter` | Gateway confirmed-use and recovery tests |
| Owner inspection and writes | `inspect_owner`, `InspectionQuery`, `InspectionCursor`, `WorkspaceManagement` | `tests/oc06_inspection.rs`, `tests/oc06_routed_provenance.rs` |
| Host operations | `ConversationQueue`, `ContextView`, `AuthorizedContextCheckpoint`, `ExternalHostSource`, `TrustedProjectSource` | `external-consumer/tests/retained_host` |
| Quiesce and verify | `WorkspaceManagement::quiesce`, `verify_quiesced`, `reopen` | Existing public API lifecycle tests and CLI independent reopen |

CLI inspection sessions live in one process. Query, display, and traversal
carry no accepted-use credit. Confirmed use requires the trusted host to
consume and reconcile the exact prepared manifest through the delivery API.
