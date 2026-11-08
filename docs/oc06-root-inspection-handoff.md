# OC06 Root inspection handoff

Import the types below from `reliquary_memory`. The CLI can mount the requested owner through its existing admission path, then call these methods. Its paired PHY setup can use the existing `Phylactery::create`, `Phylactery::verify`, and `Gateway::mount_phylactery` methods.

```rust
// Instance
pub fn inspect_owner(
    &self,
    owner: OwnerId,
    kind: OwnerKind,
    query: &InspectionQuery,
    cursor: Option<&InspectionCursor>,
    limit: usize,
) -> Result<InspectionPage, ProductError>;
pub fn workspace_management(&self, owner: OwnerId) -> Result<WorkspaceManagement, ProductError>;
pub fn rel_info(&self, owner: OwnerId) -> Result<OwnerInfo, ProductError>;
pub fn phylactery_info(&self, owner: OwnerId) -> Result<OwnerInfo, ProductError>;

// WorkspaceManagement
pub fn inspect(
    &self,
    query: &InspectionQuery,
    cursor: Option<&InspectionCursor>,
    limit: usize,
) -> Result<InspectionPage, ProductError>;
pub fn create_memory_with_request(&self, request: CreateMemoryRequest) -> Result<MemorySummary, ProductError>;
pub fn retire_memory(&self, request: MemoryRetirementRequest) -> Result<TerminalTombstone, ProductError>;
pub fn retire_entity(&self, request: EntityRetirementRequest) -> Result<EntityTombstone, ProductError>;
pub fn withdraw_entity_mention(&self, request: WithdrawEntityMentionRequest) -> Result<MemoryEntityResolution, ProductError>;
pub fn create_relationship(&self, request: CreateRelationshipRequest) -> Result<Relationship, ProductError>;
pub fn retire_relationship(&self, request: RelationshipRetirementRequest) -> Result<RelationshipTombstone, ProductError>;
```

| CLI operation | `InspectionQuery` |
| --- | --- |
| Memory list | `Memories` |
| Memory get | `Memory { id: MemoryId, revision: Option<u64> }` |
| Memory provenance | `Provenance { id: MemoryId, revision: Option<u64> }` |
| Memory history | `MemoryHistory { id: MemoryId, memory_cut: Option<u64> }` |
| Archive conversations | `Conversations` |
| Archive branches | `Branches { conversation: String }` |
| Archive transcript | `Transcript { conversation: String, leaf: String }` |
| Exact source turn | `SourceTurn { conversation: String, node: String }` |
| Graph neighbors | `Neighbors { anchor: GraphNode }` |
| Graph edge | `Edge { source: GraphNode, target: GraphNode, kind: RelationKind }` |
| Entity list/get | `Entities` or `Entity(EntityId)` |
| Relationship list/get | `Relationships` or `Relationship(RelationshipId)` |
| Community list | `Communities` |

`GraphNode::Memory` and `GraphNode::Entity` take `[u8; 32]`. Use `None` for a first page and pass the returned `next` unchanged to continue the same query. `InspectionPage` serializes to JSON, and `InspectionCursor` supports both serialization and deserialization. Each page reports `cut`, `items`, `next`, and `truncated`. An expired cursor returns `ProductErrorKind::Stale`. Restart from the first page. The window ends at the existing maximum start offset of 4096. Each response is capped at 128 items and 256 KiB. An individual record too large for the page produces a size error.

`Memory` and `Provenance` use the current revision when `revision` is absent. The Memory history cut is the accepted Memory version. Exact lookups report the body ID, revision, and publication version. Terminal Memory entries carry a tombstone coordinate, and historical reads report whether the original body remains available.

Native provenance resolves the accepted Episode and immutable Archive node, including recorded content and grounding source coordinates where available. A source turn exposes the Archive revision, content ID, parent, timestamp, and availability. Exact source lookups and provenance also expose an accepted import receipt witness when the turn came from an import. A routed PHY Memory requires a separate grant on its source REL to disclose Archive provenance. Native extraction records retain turn identities. They do not retain the original extraction quote's byte offsets.

`WorkspaceManagement` is REL-scoped. A View, Operate, or Manage grant allows inspection. Named writes require Manage and a mounted writer. `CreateMemoryRequest` takes `request_id`, `title`, `content`, `category`, `memory_type`, and `created_at_ns`. Relationship creation uses `CreateRelationshipRequest.mutation_id` as the caller's request ID. Reuse it for the same request. Retire and withdraw requests carry the exact expected revisions and source/body evidence required by the accepted owner primitive.

The shared `src/host_instance.rs` hunk only makes `require_phy_read` crate-visible and routes the mounted branches of the two existing info methods through `self.mounted_owner_info(owner, kind)`. Preserve the host lane's other methods when integrating.

## Integration paths

| Directory | Changed files |
| --- | --- |
| `src` | `management.rs`, `host_instance.rs`, `error.rs`, `lib.rs` |
| `crates/reliquary-runtime-gateway/src` | `instance_management.rs`, `instance_inspection.rs`, `lib.rs` |
| `crates/reliquary-application/src` | `management_workflow.rs`, `lib.rs`, `management_workflow/inspection.rs`, `management_workflow/inspection_archive.rs`, `management_workflow/inspection_memory.rs`, `management_workflow/inspection_provenance.rs`, `management_workflow/inspection_semantic.rs` |
| `crates/reliquary-owner-session/src` | `lib.rs`, `session.rs`, `session/management.rs`, `session/management_archive.rs`, `session/management_inspection.rs`, `session/phy_writer.rs`, `session/perception_lifecycle/entity.rs`, `session/perception_lifecycle/memory.rs`, `session/perception_lifecycle/relationship.rs`, `session/perception_lifecycle/support.rs` |
| Store crates | `crates/reliquary-archive/src/archive.rs`, `crates/reliquary-graph/src/lib.rs`, `crates/reliquary-memory-core/src/store.rs` |
| `tests` | `oc06_inspection.rs`, `oc06_routed_provenance.rs` |
| `docs` | `oc06-root-inspection-handoff.md` |
