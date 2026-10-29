use reliquary_dream::{DreamLifecycleMemory, plan_lifecycle};
use reliquary_memory_core::MemoryId;
#[test]
fn direct_authority_promotes_without_following_knowledge_revision() {
    let source = DreamLifecycleMemory {
        id: MemoryId([1; 32]),
        lifecycle_state: "extracted",
        archived: false,
        superseded_by: None,
    };
    let intents = plan_lifecycle(source, &[], false, vec![], true);
    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].lifecycle_state, "canonical");
}
