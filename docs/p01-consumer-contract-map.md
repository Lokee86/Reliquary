# P01 consumer contract map

**Historical scope notice (2026-11-07):** The [operational completion specification](public-api-cli-operational-completion-spec-2026-11-07.md) now requires public host and CLI access to import, processing, retrieval/query/refinement/traversal and inspection. It supersedes the exclusion of host-owned operations from the CLI below. This record preserves the earlier P01 disposition; it is not evidence that the new OC01–OC09 closure has passed.

This map freezes the supported 2.1 consumer boundary before P01 acceptance. The
pinned 2.0 reference is a behavior oracle, not a source-compatibility promise.
`Cva` is intentionally retired; the product type and CLI family are `Rel`.

## Retained consumers

| Retained behavior | Public 2.1 surface | Executable evidence | Intentional difference |
| --- | --- | --- | --- |
| Create, admit, inspect and independently reopen REL | `Rel::{create,open,verify,info}` | `tests/public_api.rs`; CLI tests | Durable 2.1 admission replaces reference header/open checks. |
| Create, admit and independently reopen private PHY | `Phylactery::{create,open,verify,info}` | `tests/public_api.rs` | Rust API only; no initial PHY CLI. |
| Trusted credential-to-owner admission | `TrustedHostAdmission`, immutable `OwnerGrant`, `HostAdmissionAdapter` | `tests/public_api.rs` plus retained Gateway suites | Requests cannot construct grants or obtain the runtime `Instance`. |
| Native bounded conversation queue and committed output | product `Instance` queue methods and exported immutable queue/stream values | retained runtime Gateway suites | Writer and mounted session remain private. |
| Workspace conversation paging, transcript selection, titles, direct Memory, provenance, statistics and health | `WorkspaceManagement` | `tests/public_api.rs`; management workflow tests | Mutation requires `ManageRel`; immutable page limits are 128 items/256 KiB. |
| Quiesce, independently verify and reopen | `WorkspaceManagement::{quiesce,verify_quiesced,reopen}` | public management tests and owner reopen suites | No mutable file handle is returned. |
| Owner Memory history and current canonical proof | `Rel`/`Phylactery` history and proof methods | Application/owner historical proof suites; CLI history tests | Unavailable payloads stay explicit; old bytes are never synthesized. |
| Policy rewrite and verified-copy maintenance | `plan_rewrite`, `execute_rewrite`, `VerifiedCopyPlan`, named prune/repair/reclaim functions | retained GM suites; CLI dry-run test | Every output is distinct/no-replace; there is no force flag. |
| Same-owner REL reconciliation | `reconcile_rel` | M03 suites, including destination Dream admission regression | Cross-owner and arbitrary PHY merge remain unsupported. |
| REL operator create/info/verify/history/rewrite/prune/repair/reclaim/reconcile | separate `cli/` package | `cli/tests/cli.rs` | JSON reports and nonzero typed failures replace ad-hoc text. |

The reference `workspace_management.rs`, `native_import.rs`,
`historical_payload_boundary.rs`, `freshness_retrieval_boundary.rs`, and
`multiplexing_*contract.rs` behaviors remain implemented below the product
boundary by the accepted Application/Gateway workflows. P01 exposes the
management and native queue entry points while retaining the existing workflow
tests as the detailed behavioral oracle.

## Reference CLI disposition

| Reference family | P01 disposition | Reason |
| --- | --- | --- |
| `rel create/info/verify` | Mapped | Direct product API and real full-owner admission. |
| Memory/Archive history and proofs | Mapped | Bounded read-only JSON summaries. |
| rewrite | Mapped | Policy JSON, exact plan, dry run, verified no-replace copy. |
| terminal prune/profile repair/reclaim/reconcile | Mapped | Only GM-accepted typed operations. |
| `archive conversations/show` | Mapped to bounded `rel history archive`; terminal prune maps to `rel prune archive` | Raw fragment/dev inspection and direct tombstoning are not public operator contracts. |
| `import` | Intentionally host-owned | Native import is an authenticated accepted workflow, not an unauthenticated file CLI. No placeholder command. |
| `vectors` | Intentionally host-owned | Embedding execution requires configured endpoints, grants and commit-time recheck. No local provider administration. |
| `insomnia` | Intentionally host-owned | Worker claims and Held recovery stay in the authenticated Gateway lifecycle. No local worker shortcut. |
| reference `dev` and title migration | Retired | Development synthesis and 2.0 migration are excluded from P01. |
| `cva` alias | Retired | Hard-cut rename to `rel`; CLI test proves rejection. |
| `phy` CLI | Not retained | Repository search found only pinned legacy CLI/tests, not a current downstream consumer contract. Full PHY Rust API remains supported. |

## Boundary rules

- IDs are stable owner-local values. `OwnerId` JSON is 32 lowercase hex
  characters; Memory/Entity/Profile IDs are 64 lowercase hex in CLI inputs.
- Enumeration is deterministic and bounded. Management limits cannot be
  widened by a caller.
- Read, plan and dry-run operations never modify a source. Publication always
  rejects an existing destination.
- Current format means native 2.1 only. Unsupported 2.0 signatures fail as
  `wrong_format`; conversion remains D01–D03 work.
- `ProductErrorKind` keeps wrong format, admission, authorization, revocation,
  unavailable history, unsupported operation, stale cut, conflict, writer,
  publication collision, I/O and verification failures distinct.
