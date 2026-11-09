# Reliquary 2.1 public product API

The operational completion documents define the broader public API and CLI
acceptance criteria. [Specification](public-api-cli-operational-completion-spec-2026-11-07.md), [interaction contract](public-api-cli-interaction-contract-2026-11-07.md), [Switchboard configuration restoration](switchboard-configuration-restoration-spec-2026-11-08.md), [gap audit](public-api-cli-gap-audit-2026-11-07.md).

Downstream Rust consumers depend on the root `reliquary-memory` package. Create
or open owners with `Rel` and `Phylactery`; both are opaque read handles.
Writable work is mounted once in `Gateway` and reached only through an
authenticated product `Instance`.

```rust
use reliquary_memory::{
    Gateway, HostCredentialBinding, OwnerGrant, OwnerScope, Rel,
    TrustedHostAdmission,
};

let rel = Rel::create("project.rel", OwnerScope::Project)?;
let gateway = Gateway::new();
let owner = gateway.mount_rel(&rel, true)?;
let token = load_secret_from_host_configuration();
let admission = TrustedHostAdmission::new([
    HostCredentialBinding::new("operator", &token, [OwnerGrant::ManageRel(owner)])
]);
let (requests, revocation) = admission.install(&gateway)?;
let instance = requests.authenticate(&token)?;
let management = instance.workspace_management(owner)?;
let health = management.health()?;
# Ok::<(), reliquary_memory::ProductError>(())
```

Tokens belong in host configuration, never request parameters, logs, or CLI
arguments. Grants are immutable. Revocation is checked again on reads and at
writer handoff/commit.

The canonical local provider configuration is the binary `reliquary.cfg`.
Switchboard keeps credential objects encrypted in the configuration and
stores the master key for its canonical path in the operating system
credential service. `ConfiguredRuntime` opens the validated configuration
through the Root API. See the [configured runtime guide](configured-runtime.md) and [OS-backed master-key specification](os-master-key-store-spec-2026-11-08.md).

The separate `reliquary` CLI supports explicit REL and PHY commands. It emits
JSON and returns nonzero on failure or incomplete requested readiness. Copy
maintenance keeps its create-only destinations. See the
[operator workflow and public symbol map](operator-workflow.md).

## Checked manual Memory creation

Pass the `InspectionCut` from an authorized owner inspection to
`WorkspaceManagement::create_memory_at_cut` along with a `CreateMemoryRequest`.
The operation requires `ManageRel` and compares a new request's cut under
the same writer lock as publication. An exact accepted request can replay in
the same owner history generation. Changing the draft under an accepted
request ID returns a conflict.

The receipt carries the durable Memory identity and revision, the current
owner cut after the operation, and whether this call created the Memory. The
principal comes from the authenticated Instance, and the direct Memory keeps
operator provenance.

## Checked Memory correction and retirement

`WorkspaceManagement::correct_memory_at_cut` accepts the current inspection
cut, original Memory ID, revision and body ID, and a stable request ID.
The immutable successor, original lifecycle revision and user supersession
edge share one durable owner commit. The response identifies both Memories
and returns the replacement's authenticated user provenance. Exact replay
reuses the accepted effect after reopening the owner.

`WorkspaceManagement::retire_memory_at_cut` takes a stable request ID and
the exact original and successor evidence required by the existing
supersession and reference-closure checks. It returns the committed
tombstone revision and resulting cut.

## Checked user Memory associations

`WorkspaceManagement::change_memory_association` requires a ManageRel grant,
the current inspection cut and both Memory revisions. A new link names the
source and target. Changing an existing association also supplies its edge ID
and revision. The operation admits only directed user-origin Topical edges and
protects an existing Dream edge on the same pair. Unlinking writes an inactive
user edge so Dream cannot silently restore it.

The receipt carries the original edge effect and the current cut. Exact
principal-bound requests replay the original edge across reopen, with
`replayed` set and no second write. A reused request ID with different evidence
conflicts. A foreign owner or generation, changed cut, or stale endpoint or
edge revision is rejected before publication.

## Hermes copied-source import

A trusted host can construct a bounded `HermesCopiedSnapshot`, validate it with
`NativeImportPlan::from_hermes_copy`, and call the existing
`Instance::stage_native_import` operation with an `OperateRel` grant. The
receipt binds the original profile, session, message IDs, times and supplied
author metadata to the normalized copy digest. A known branch and revision are
retained in source provenance. Unknown branch and parent identities remain
unasserted. The local ordered Archive branch describes only the copied view.
The native worker accepts REL-only processing for this import. Source author
metadata carries no PHY grant.

For incremental copy, call `NativeImportPlan::from_hermes_copy_extension`
with only the contiguous new messages and the previous completed copy's
native `source_sha256`. The owner checks the authenticated predecessor
receipt, exact current branch head and all incoming message IDs before
staging. The native extension digest binds both the suffix and its
predecessor. The resulting receipt tracks only the new source messages,
and the new native Episodes enter the same processing workflow. Exact
replay preserves the previous receipt and does not allocate new turns.
Changed prefixes, branch changes and stale anchors fail with a conflict.
The host owns source-view invalidation on those transitions.

Use `Instance::tombstone_hermes_copy` with the same authenticated principal,
source scope and an exact accepted copy SHA-256 after the import epoch
settles. The marker covers the whole copied conversation, including every
extension. It retains provenance, hides the source bodies from inspection
and rejects stale copy replay. Repeating the terminal notice is idempotent.
The host owns pending-source invalidation while an epoch is still open.

## Dashboard inspection projections

Use `InspectionQuery::MemorySummaries` for bounded filtering by text, category,
type, lifecycle and current community. The result carries a UTF-8-safe excerpt
and a filter-bound continuation. A terminal Memory has no body or title in the
list projection. Use the existing exact inspection queries for detail, history
and provenance.

`InspectionQuery::MemoryGraph` returns active Memory-to-Memory edges with stable
owner-scoped IDs, graph revisions and local endpoints. The request bounds depth
and the node and edge counts. Use the returned cut when expanding a
neighborhood. A changed cut returns stale. The projection reports truncation
when it hits the scan, graph or byte budget. Community filters apply only to
a current graph generation.

## Community names and membership

Authorized REL managers can use `WorkspaceManagement::rename_community_at_cut`
on an accepted, current Community. Supply the complete inspection cut, exact
Community generation, stable request ID, and bounded name. The durable receipt
binds the authenticated principal, target and input. An exact replay reports
the original effect. A changed name advances the Community generation while
preserving membership and the Graph version.

`InspectionQuery::Communities` projects a user name and its origin.
`InspectionQuery::CommunityMembers` returns bounded member IDs with
cut-bound pagination. The Community snapshot must be current. Missing derived
metadata remains unavailable. User naming preserves the existing partition and
does not generate one.

Verified storage reclamation and pruning retain the name receipt. Fresh-history
rewrite stops on user Memory correction, retirement, association and Community
naming commits until it can preserve their authenticated authority and request
history. Divergent reconciliation stops when a divergent tail contains any of
these commits.

## Migration from the reference facade

- `Cva` → `Rel` (intentional hard cut; no alias).
- mutable root/store access → opaque owner inspection or authenticated
  `Instance`/`WorkspaceManagement` operation.
- caller-created replay evidence → durable current-owner admission.
- in-place maintenance → explicit plan plus verified, independently reopened
  no-replace copy.
- direct provider/worker CLI commands → configured trusted-host workflows.

Unsupported scenarios include 2.0 readers/conversion, cross-owner or PHY
reconciliation, arbitrary Dream/Insomnia maintenance, mutable public stores,
network daemons, and unrestricted provider administration.
