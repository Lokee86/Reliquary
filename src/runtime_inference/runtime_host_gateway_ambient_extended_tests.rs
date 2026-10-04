use super::*;
use crate::runtime_host_test_support::test_path;
use crate::{Cva, EpisodePolicy, InsomniaWorkerConfig, InteractionRuntime, ReliquaryRuntimeRoutes};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

fn gateway(routes: ReliquaryRuntimeRoutes) -> GatewayRuntime {
    GatewayRuntime::new(
        test_path("authorization-state"),
        routes,
        InsomniaWorkerConfig {
            workers: 0,
            ..InsomniaWorkerConfig::default()
        },
        EpisodePolicy::default(),
    )
}
fn grants(owner: &str, caps: &[OwnerGrant]) -> BTreeMap<String, BTreeSet<OwnerGrant>> {
    BTreeMap::from([(owner.to_owned(), caps.iter().copied().collect())])
}
fn seeded() -> (
    Cva,
    crate::MemoryId,
    crate::MemoryId,
    crate::CommunityId,
    crate::CompatibilityProfileId,
) {
    let mut cva = Cva::create_project(test_path("authorization.rel")).unwrap();
    cva.append_node(
        "node".into(),
        "conversation".into(),
        None,
        "user".into(),
        1,
        "searchable bluejay evidence",
    )
    .unwrap();
    let mut ids = Vec::new();
    for title in ["bluejay", "lake"] {
        let mut draft = user_creation_draft(
            title.into(),
            format!("searchable {title} evidence"),
            "knowledge".into(),
            "fact".into(),
            1,
        );
        draft.lifecycle_state = "canonical".into();
        ids.push(cva.publish_memory(None, 0, draft).unwrap().0.id);
    }
    cva.set_memory_relation(
        ids[0],
        ids[1],
        crate::GraphRelationKind::Topical,
        true,
        cva.graph_version(),
    )
    .unwrap();
    cva.refresh_communities_leiden().unwrap();
    let community = cva.community_snapshot().unwrap().communities[0].id;
    let endpoint = crate::SimulatedEmbeddingEndpoint::new(8, crate::VectorNormalization::L2, 77);
    let profile = cva.establish_compatibility_profile(&endpoint).unwrap().id;
    cva.build_missing_memory_vectors(profile, &endpoint)
        .unwrap();
    cva.sync().unwrap();
    (cva, ids[0], ids[1], community, profile)
}

#[derive(Clone, Copy, Debug)]
enum ReadOp {
    SummaryPage,
    TranscriptPage,
    TranscriptPageFor,
    ArchivePage,
    DependencySearch,
    DependencySearchFor,
    TextSearch,
    TextSearchFor,
    VectorSearch,
    VectorSearchFor,
    EchoSource,
    Knowledge,
    Provenance,
}
impl ReadOp {
    fn explicit(self) -> bool {
        matches!(
            self,
            Self::TranscriptPageFor
                | Self::DependencySearchFor
                | Self::TextSearchFor
                | Self::VectorSearchFor
                | Self::EchoSource
                | Self::Knowledge
                | Self::Provenance
        )
    }
    fn call(
        self,
        g: &GatewayRuntime,
        i: &InstanceRuntime,
        owner: &str,
        memory: crate::MemoryId,
        profile: crate::CompatibilityProfileId,
    ) -> Result<(), ReliquaryRuntimeHostError> {
        let config = crate::MemoryRetrievalConfig::default();
        match self {
            Self::SummaryPage => g
                .conversation_summaries_page(i, 0, 1)
                .map(|v| assert_eq!(v.len(), 1)),
            Self::TranscriptPage => g
                .conversation_transcript_page(i, "conversation", "node", 0, 1)
                .map(|v| assert_eq!(v.len(), 1)),
            Self::TranscriptPageFor => g
                .conversation_transcript_page_for(i, owner, "conversation", "node", 0, 1)
                .map(|v| assert_eq!(v.len(), 1)),
            Self::ArchivePage => g
                .search_archive_page(i, "bluejay", 0, 1)
                .map(|v| assert!(!v.is_empty())),
            Self::DependencySearch => g
                .search_dependency_archive(i, "bluejay", 1)
                .map(|v| assert_eq!(v.len(), 1)),
            Self::DependencySearchFor => g
                .search_dependency_archive_for(i, owner, "bluejay", 1)
                .map(|v| assert_eq!(v.len(), 1)),
            Self::TextSearch => g
                .search_memories_by_text(i, profile, "bluejay", config)
                .map(|v| assert!(!v.items.is_empty())),
            Self::TextSearchFor => g
                .search_memories_by_text_for(i, owner, profile, "bluejay", config)
                .map(|v| assert!(!v.items.is_empty())),
            Self::VectorSearch => g
                .retrieve_reliquary_memories(i, profile, &[1.0 / 8.0_f32.sqrt(); 8], config)
                .map(|v| assert!(!v.items.is_empty())),
            Self::VectorSearchFor => g
                .retrieve_reliquary_memories_for(
                    i,
                    owner,
                    profile,
                    &[1.0 / 8.0_f32.sqrt(); 8],
                    config,
                )
                .map(|v| assert!(!v.items.is_empty())),
            Self::EchoSource => g
                .read_echo_source_for(i, owner, "conversation", "node", "node")
                .map(|_| ()),
            Self::Knowledge => g.read_reliquary_knowledge_for(i, owner).map(|_| ()),
            Self::Provenance => g.read_memory_provenance_for(i, owner, memory).map(|_| ()),
        }
    }
}
const READS: &[ReadOp] = &[
    ReadOp::SummaryPage,
    ReadOp::TranscriptPage,
    ReadOp::TranscriptPageFor,
    ReadOp::ArchivePage,
    ReadOp::DependencySearch,
    ReadOp::DependencySearchFor,
    ReadOp::TextSearch,
    ReadOp::TextSearchFor,
    ReadOp::VectorSearch,
    ReadOp::VectorSearchFor,
    ReadOp::EchoSource,
    ReadOp::Knowledge,
    ReadOp::Provenance,
];

#[test]
fn paging_and_search_families_require_exact_attached_view_authority() {
    let endpoint = Arc::new(crate::SimulatedEmbeddingEndpoint::new(
        8,
        crate::VectorNormalization::L2,
        77,
    ));
    let g = gateway(ReliquaryRuntimeRoutes::new(
        None,
        None,
        None,
        None,
        Some(endpoint),
    ));
    let (cva, memory, _, _, profile) = seeded();
    let owner = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let other = g
        .mount_rel(InteractionRuntime::new(
            Cva::create_project(test_path("ungranted-search.rel")).unwrap(),
        ))
        .unwrap();
    let reader = g
        .attach_instance(
            "reader".into(),
            None,
            grants(&owner, &[OwnerGrant::ViewRel]),
        )
        .unwrap();
    let wrong = g
        .attach_instance(
            "wrong-grant".into(),
            None,
            grants(&owner, &[OwnerGrant::ManageRel]),
        )
        .unwrap();
    g.select_rel(&reader, &owner).unwrap();
    assert!(g.select_rel(&reader, &other).is_err());
    for op in READS {
        op.call(&g, &reader, &owner, memory, profile).unwrap();
        assert!(
            op.call(&g, &wrong, &owner, memory, profile).is_err(),
            "{op:?}: wrong grant"
        );
        if op.explicit() {
            let error = op
                .call(&g, &reader, &other, memory, profile)
                .unwrap_err()
                .to_string();
            assert!(error.contains("ViewRel"), "{op:?}: wrong owner: {error}");
        }
    }
    assert_eq!(g.viewable_rel_owners(&reader).unwrap(), vec![owner.clone()]);
    assert!(g.viewable_rel_owners(&wrong).unwrap().is_empty());
    g.replace_instance_grants(&reader, BTreeMap::new()).unwrap();
    assert!(g.viewable_rel_owners(&reader).unwrap().is_empty());
    for op in READS {
        assert!(
            op.call(&g, &reader, &owner, memory, profile).is_err(),
            "{op:?}: revoked"
        );
    }
    g.replace_instance_grants(&reader, grants(&owner, &[OwnerGrant::ViewRel]))
        .unwrap();
    g.select_rel(&reader, &owner).unwrap();
    g.detach_instance(&reader).unwrap();
    for op in READS {
        assert!(
            op.call(&g, &reader, &owner, memory, profile).is_err(),
            "{op:?}: detached"
        );
    }
    assert!(g.viewable_rel_owners(&reader).is_err());
    g.detach_instance(&wrong).unwrap();
    drop(g.unmount_rel(&owner).unwrap());
    drop(g.unmount_rel(&other).unwrap());
}

#[test]
fn paging_boundaries_do_not_enumerate_or_bypass_authorization() {
    let g = gateway(ReliquaryRuntimeRoutes::default());
    let (cva, _, _, _, _) = seeded();
    let owner = g.mount_rel(InteractionRuntime::new(cva)).unwrap();
    let reader = g
        .attach_instance("pager".into(), None, grants(&owner, &[OwnerGrant::ViewRel]))
        .unwrap();
    let denied = g
        .attach_instance("denied-pager".into(), None, BTreeMap::new())
        .unwrap();
    g.select_rel(&reader, &owner).unwrap();
    for instance in [&*reader, &*denied] {
        assert!(g.conversation_summaries_page(instance, 0, 0).is_err());
        assert!(
            g.conversation_transcript_page(instance, "conversation", "node", 0, 0)
                .is_err()
        );
        assert!(
            g.conversation_transcript_page_for(instance, &owner, "conversation", "node", 0, 0)
                .is_err()
        );
        assert!(g.search_archive_page(instance, "bluejay", 0, 0).is_err());
    }
    let known = g
        .conversation_transcript_page_for(&denied, &owner, "conversation", "node", 0, 0)
        .unwrap_err()
        .to_string();
    let unknown = g
        .conversation_transcript_page_for(&denied, "rel-unmounted", "conversation", "node", 0, 0)
        .unwrap_err()
        .to_string();
    assert_eq!(known, unknown, "invalid limits exposed owner existence");
    assert!(
        g.conversation_summaries_page(&reader, usize::MAX, 1)
            .unwrap()
            .is_empty()
    );
    assert!(
        g.conversation_transcript_page(&reader, "conversation", "node", usize::MAX, 1)
            .unwrap()
            .is_empty()
    );
    assert!(
        g.conversation_transcript_page_for(&reader, &owner, "conversation", "node", usize::MAX, 1)
            .unwrap()
            .is_empty()
    );
    assert!(
        g.search_archive_page(&reader, "bluejay", usize::MAX, 1)
            .unwrap()
            .is_empty()
    );
    assert!(
        g.conversation_summaries_page(&denied, usize::MAX, 1)
            .is_err()
    );
    assert!(
        g.conversation_transcript_page_for(&denied, &owner, "conversation", "node", usize::MAX, 1)
            .is_err()
    );
    assert!(
        g.search_archive_page(&denied, "bluejay", usize::MAX, 1)
            .is_err()
    );
    for i in [&*reader, &*denied] {
        g.detach_instance(i).unwrap();
    }
    drop(g.unmount_rel(&owner).unwrap());
}

#[test]
fn phy_private_semantic_operations_remain_unsupported_on_gateway_surface() {
    let g = gateway(ReliquaryRuntimeRoutes::default());
    let phy = g
        .mount_phy(crate::Phylactery::create(test_path("private-authorization.phy")).unwrap())
        .unwrap();
    let i = g
        .attach_instance(
            "private-principal".into(),
            Some(phy.clone()),
            grants(&phy, &[OwnerGrant::UseOwnPhy]),
        )
        .unwrap();
    g.authorize(&i, &phy, OwnerGrant::UseOwnPhy).unwrap();
    assert!(g.viewable_rel_owners(&i).unwrap().is_empty());
    assert!(g.read_reliquary_knowledge_for(&i, &phy).is_err());
    assert!(
        g.search_memories_by_text_for(
            &i,
            &phy,
            crate::CompatibilityProfileId([0; 32]),
            "private",
            crate::MemoryRetrievalConfig::default()
        )
        .is_err()
    );
    assert!(
        g.create_knowledge_memory_for(
            &i,
            &phy,
            "private".into(),
            "private".into(),
            "knowledge".into(),
            "fact".into(),
            1
        )
        .is_err()
    );
    g.detach_instance(&i).unwrap();
    drop(g.unmount_phy(&phy).unwrap());
}

#[cfg(test)]
#[path = "runtime_host_gateway_management_tests.rs"]
mod management;
