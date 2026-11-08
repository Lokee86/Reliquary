use reliquary_memory::{
    CorrectMemoryRequest, CreateMemoryRequest, Gateway, HostCredentialBinding, InspectionCut,
    InspectionItem, InspectionQuery, MemoryAssociationAction, MemoryAssociationRequest,
    MemoryBodyId, MemoryId, MemoryRetirementRequest, OwnerGrant, OwnerScope, Rel,
    TrustedHostAdmission, WorkspaceManagement,
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const TOKEN: &str = "fixture-hp02-mixed-writes-credential";

struct OwnerFile(PathBuf);

impl OwnerFile {
    fn new() -> Self {
        let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        fs::create_dir_all(&scratch).unwrap();
        let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let path = scratch.join(format!(
            "hp02-mixed-{}-{}.rel",
            std::process::id(),
            time.as_nanos()
        ));
        Rel::create(&path, OwnerScope::Project).unwrap();
        Self(path)
    }

    fn mount(&self) -> WorkspaceManagement {
        let gateway = Gateway::new();
        let rel = Rel::open(&self.0).unwrap();
        let owner = gateway.mount_rel(&rel, true).unwrap();
        let (admission, _) = TrustedHostAdmission::new([HostCredentialBinding::new(
            "operator",
            TOKEN,
            [OwnerGrant::ManageRel(owner)],
        )])
        .install(&gateway)
        .unwrap();
        admission
            .authenticate(TOKEN)
            .unwrap()
            .workspace_management(owner)
            .unwrap()
    }
}

impl Drop for OwnerFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn cut(workspace: &WorkspaceManagement) -> InspectionCut {
    workspace
        .inspect(&InspectionQuery::Memories, None, 1)
        .unwrap()
        .cut
}

fn digest(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    std::array::from_fn(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap())
}

fn manual(id: &str) -> CreateMemoryRequest {
    CreateMemoryRequest {
        request_id: id.into(),
        title: format!("Memory {id}"),
        content: format!("Original body {id}"),
        category: "fact".into(),
        memory_type: "note".into(),
        created_at_ns: 42,
    }
}

#[test]
fn association_correction_and_retirement_replay_together_without_rewriting_the_owner() {
    let file = OwnerFile::new();
    let workspace = file.mount();
    let original = workspace
        .create_memory_at_cut(cut(&workspace), manual("original"))
        .unwrap();
    let peer = workspace
        .create_memory_at_cut(original.cut, manual("peer"))
        .unwrap();
    let original_id = MemoryId(digest(&original.memory.id));

    let link = MemoryAssociationRequest {
        request_id: "link".into(),
        source_id: original_id,
        target_id: MemoryId(digest(&peer.memory.id)),
        source_revision: original.memory.revision,
        target_revision: peer.memory.revision,
        expected_edge_id: None,
        expected_edge_revision: None,
        action: MemoryAssociationAction::Link,
    };
    let linked = workspace
        .change_memory_association(peer.cut, link.clone())
        .unwrap();
    assert!(linked.edge.edge.active);
    let unlink = MemoryAssociationRequest {
        request_id: "unlink".into(),
        expected_edge_id: Some(linked.edge.id.clone()),
        expected_edge_revision: Some(linked.edge.edge.graph_version),
        action: MemoryAssociationAction::Unlink,
        ..link.clone()
    };
    let unlinked = workspace
        .change_memory_association(linked.cut, unlink.clone())
        .unwrap();
    assert!(!unlinked.edge.edge.active);

    let correction = CorrectMemoryRequest {
        request_id: "correct".into(),
        memory_id: original_id,
        expected_revision: original.memory.revision,
        expected_body_id: MemoryBodyId(digest(&original.memory.body_id)),
        title: "Corrected title".into(),
        content: "Corrected body".into(),
        created_at_ns: 43,
    };
    let corrected = workspace
        .correct_memory_at_cut(unlinked.cut, correction.clone())
        .unwrap();
    assert!(corrected.created);
    assert_eq!(corrected.original_memory_id, original.memory.id);
    assert_ne!(corrected.replacement.id, original.memory.id);
    let replacement_id = MemoryId(digest(&corrected.replacement.id));
    let retirement = MemoryRetirementRequest {
        owner_id: workspace.owner_id().as_bytes(),
        memory_id: original_id,
        memory_revision: corrected.original_revision,
        body_id: MemoryBodyId(digest(&original.memory.body_id)),
        superseded_by: replacement_id,
        successor_revision: corrected.replacement.revision,
        successor_body_id: MemoryBodyId(digest(&corrected.replacement.body_id)),
    };
    let retired = workspace
        .retire_memory_at_cut(corrected.cut, "retire", retirement.clone())
        .unwrap();
    assert!(retired.retired);
    drop(workspace);
    let committed_bytes = fs::read(&file.0).unwrap();

    let reopened = file.mount();
    let replayed_link = reopened.change_memory_association(peer.cut, link).unwrap();
    assert!(replayed_link.replayed && replayed_link.edge.edge.active);
    let replayed_unlink = reopened
        .change_memory_association(linked.cut, unlink)
        .unwrap();
    assert!(replayed_unlink.replayed && !replayed_unlink.edge.edge.active);
    let replayed_correction = reopened
        .correct_memory_at_cut(unlinked.cut, correction)
        .unwrap();
    assert!(!replayed_correction.created);
    assert_eq!(replayed_correction.replacement.id, corrected.replacement.id);
    let replayed_retirement = reopened
        .retire_memory_at_cut(corrected.cut, "retire", retirement)
        .unwrap();
    assert!(!replayed_retirement.retired);
    assert_eq!(replayed_retirement.revision, retired.revision);
    assert_eq!(
        reopened.memory(replacement_id).unwrap().content,
        "Corrected body"
    );
    let terminal = reopened
        .inspect(
            &InspectionQuery::Memory {
                id: original_id,
                revision: None,
            },
            None,
            1,
        )
        .unwrap();
    assert!(matches!(
        terminal.items.as_slice(),
        [InspectionItem::MemoryTerminal { .. }]
    ));
    drop(reopened);
    assert_eq!(fs::read(&file.0).unwrap(), committed_bytes);
}
