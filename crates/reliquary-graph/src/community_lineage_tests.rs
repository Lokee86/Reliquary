use crate::community_lineage::{derive_lineage, resolve_semantic_name};
use crate::{
    Community, CommunityId, CommunitySemanticName, CommunitySemanticNameSource, CommunitySnapshot,
    MemoryId,
};
use std::collections::HashMap;

#[test]
fn plus_one_memory_is_a_clear_continuation() {
    let previous = snapshot(1, vec![community(1, &[1, 2, 3, 4])]);
    let current = snapshot(2, vec![community(2, &[1, 2, 3, 4, 5])]);
    let transition = derive_lineage(&previous, &current);
    assert_eq!(transition.links.len(), 1);
    assert!(transition.links[0].continuation);
    assert_eq!(transition.links[0].jaccard_per_mille(), 800);
}

#[test]
fn dream_name_retention_is_measured_from_original_baseline() {
    let snapshots = vec![
        snapshot(1, vec![community(1, &[1, 2, 3, 4, 5, 6, 7, 8])]),
        snapshot(2, vec![community(2, &[1, 2, 3, 4, 5, 6, 7, 8, 9])]),
        snapshot(3, vec![community(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])]),
        snapshot(4, vec![community(4, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11])]),
    ];
    let names = names(CommunitySemanticNameSource::Dream);
    assert!(resolve_semantic_name(&snapshots[..3], &names, id(3)).is_some());
    assert!(resolve_semantic_name(&snapshots, &names, id(4)).is_none());
}

#[test]
fn user_name_survives_material_drift_along_clear_lineage() {
    let snapshots = vec![
        snapshot(1, vec![community(1, &[1, 2, 3, 4])]),
        snapshot(2, vec![community(2, &[1, 2, 3, 4, 5, 6])]),
        snapshot(3, vec![community(3, &[1, 2, 3, 4, 5, 6, 7, 8])]),
    ];
    let names = names(CommunitySemanticNameSource::User);
    let inherited = resolve_semantic_name(&snapshots, &names, id(3)).unwrap();
    assert_eq!(inherited.community_id, id(3));
    assert_eq!(inherited.baseline_community_id, id(1));
    assert_eq!(inherited.name, "Architecture");
}

#[test]
fn tied_split_does_not_choose_a_name_heir() {
    let snapshots = vec![
        snapshot(1, vec![community(1, &[1, 2, 3, 4])]),
        snapshot(2, vec![community(2, &[1, 2]), community(3, &[3, 4])]),
    ];
    let names = names(CommunitySemanticNameSource::User);
    assert!(resolve_semantic_name(&snapshots, &names, id(2)).is_none());
    assert!(resolve_semantic_name(&snapshots, &names, id(3)).is_none());
}

fn names(source: CommunitySemanticNameSource) -> HashMap<CommunityId, CommunitySemanticName> {
    let record = CommunitySemanticName {
        community_id: id(1),
        baseline_community_id: id(1),
        contract_version: if source == CommunitySemanticNameSource::Dream {
            2
        } else {
            0
        },
        source,
        name: "Architecture".into(),
        representative_memories: if source == CommunitySemanticNameSource::Dream {
            vec![[1; 32]]
        } else {
            Vec::new()
        },
    };
    HashMap::from([(id(1), record)])
}

fn snapshot(generation: u64, communities: Vec<Community>) -> CommunitySnapshot {
    CommunitySnapshot {
        generation,
        derived_graph_version: generation,
        algorithm_version: crate::COMMUNITY_ALGORITHM_VERSION,
        seed: crate::COMMUNITY_LEIDEN_SEED,
        resolution: crate::COMMUNITY_LEIDEN_RESOLUTION,
        quality: 0.0,
        communities,
    }
}

fn community(identifier: u8, members: &[u8]) -> Community {
    Community {
        id: id(identifier),
        members: members.iter().map(|member| [*member; 32]).collect(),
    }
}

fn id(value: u8) -> CommunityId {
    CommunityId([value; 32])
}
