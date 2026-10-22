use crate::community_codec::encode_snapshot;
use crate::community_scan_merge::scan_merge_snapshot;
use crate::community_test_support::{edge, member_sets, path, phy_memory, rel_episode, rel_memory};
use crate::{COMMUNITY_ALGORITHM_VERSION, CommunityError, Cva, GraphRelationKind, Phylactery};
use std::collections::HashSet;
use std::fs;

#[test]
fn leiden_scan_merge_detects_dense_regions_and_persists() {
    let file = path("communities.prj.rel");
    let mut rel = Cva::create_project(&file).unwrap();
    let episode = rel_episode(&mut rel);
    let a = rel_memory(&mut rel, &episode, "a");
    let b = rel_memory(&mut rel, &episode, "b");
    let c = rel_memory(&mut rel, &episode, "c");
    let d = rel_memory(&mut rel, &episode, "d");
    let e = rel_memory(&mut rel, &episode, "e");
    let f = rel_memory(&mut rel, &episode, "f");

    rel.set_memory_relations(
        &[
            edge(a, b),
            edge(b, c),
            edge(c, a),
            edge(d, e),
            edge(e, f),
            edge(f, d),
            edge(c, d),
        ],
        0,
    )
    .unwrap();

    let snapshot = rel.refresh_communities_leiden().unwrap();
    assert_eq!(snapshot.generation, 1);
    assert_eq!(snapshot.derived_graph_version, 1);
    assert_eq!(snapshot.communities.len(), 2);
    assert_eq!(rel.community_stats().memberships, 6);
    assert!(rel.community_stats().current);

    let expected: HashSet<Vec<[u8; 32]>> = [vec![a.0, b.0, c.0], vec![d.0, e.0, f.0]]
        .into_iter()
        .map(|mut members| {
            members.sort();
            members
        })
        .collect();
    assert_eq!(member_sets(&snapshot), expected);

    rel.sync().unwrap();
    let bytes_before_noop = fs::metadata(&file).unwrap().len();
    let repeated = rel.refresh_communities_leiden().unwrap();
    assert_eq!(repeated, snapshot);
    assert_eq!(fs::metadata(&file).unwrap().len(), bytes_before_noop);
    drop(rel);

    let reopened = Cva::open_project(&file).unwrap();
    assert_eq!(reopened.community_snapshot().unwrap(), snapshot);
    assert!(reopened.community_stats().current);
}

#[test]
fn graph_change_marks_snapshot_stale_until_refresh() {
    let file = path("stale.prj.rel");
    let mut rel = Cva::create_project(&file).unwrap();
    let episode = rel_episode(&mut rel);
    let a = rel_memory(&mut rel, &episode, "a");
    let b = rel_memory(&mut rel, &episode, "b");
    let c = rel_memory(&mut rel, &episode, "c");
    rel.set_memory_relations(&[edge(a, b), edge(b, c)], 0)
        .unwrap();
    let initial = rel.refresh_communities_leiden().unwrap();
    assert_eq!(initial.generation, 1);

    rel.set_memory_relation(a, c, GraphRelationKind::Factual, true, 1)
        .unwrap();
    assert!(!rel.community_stats().current);
    assert_eq!(rel.community_snapshot(), Some(initial));
    let refreshed = rel.refresh_communities_leiden().unwrap();
    assert_eq!(refreshed.generation, 2);
    assert_eq!(refreshed.derived_graph_version, 2);
    assert!(rel.community_stats().current);
}

#[test]
fn clock_neutral_memory_tombstones_invalidate_community_projections_for_rel_and_phy() {
    let file = path("tombstoned-community-member.prj.rel");
    let mut rel = Cva::create_project(&file).unwrap();
    let episode = rel_episode(&mut rel);
    let a = rel_memory(&mut rel, &episode, "tombstone-rel-a");
    let b = rel_memory(&mut rel, &episode, "tombstone-rel-b");
    let c = rel_memory(&mut rel, &episode, "tombstone-rel-c");
    rel.set_memory_relations(&[edge(a, b), edge(b, c), edge(c, a)], 0)
        .unwrap();
    let old_rel_snapshot = rel.refresh_communities_leiden().unwrap();
    let graph_version = rel.memory_graph_version();
    assert!(
        old_rel_snapshot
            .communities
            .iter()
            .any(|community| community.members.contains(&b))
    );
    assert!(rel.community_stats().current);

    rel.tombstone_memory(b, 1).unwrap();
    assert_eq!(rel.memory_graph_version(), graph_version);
    assert!(!rel.community_stats().current);
    assert!(rel.community_snapshot().is_none());

    rel.sync().unwrap();
    drop(rel);
    let mut rel = Cva::open_project(&file).unwrap();
    assert_eq!(
        rel.community_stats().generations,
        old_rel_snapshot.generation
    );
    assert!(!rel.community_stats().current);
    assert!(rel.community_snapshot().is_none());

    let rel_snapshot = rel.refresh_communities_leiden().unwrap();
    assert_eq!(rel_snapshot.generation, old_rel_snapshot.generation + 1);
    assert_eq!(rel_snapshot.derived_graph_version, graph_version);
    let rel_members: Vec<_> = rel_snapshot
        .communities
        .iter()
        .flat_map(|community| community.members.iter().copied())
        .collect();
    assert!(!rel_members.contains(&b));
    assert!(rel_members.contains(&a));
    assert!(rel_members.contains(&c));
    assert!(rel.community_stats().current);
    assert_eq!(rel.community_snapshot(), Some(rel_snapshot.clone()));

    rel.sync().unwrap();
    drop(rel);
    let reopened_rel = Cva::open_project(&file).unwrap();
    assert_eq!(reopened_rel.community_snapshot(), Some(rel_snapshot));
    assert!(reopened_rel.community_stats().current);

    let file = path("tombstoned-community-member.phy");
    let mut phy = Phylactery::create(&file).unwrap();
    let a = phy_memory(&mut phy, "tombstone-phy-a");
    let b = phy_memory(&mut phy, "tombstone-phy-b");
    let c = phy_memory(&mut phy, "tombstone-phy-c");
    phy.set_memory_relations(&[edge(a, b), edge(b, c), edge(c, a)], 0)
        .unwrap();
    let old_phy_snapshot = phy.refresh_communities_leiden().unwrap();
    let graph_version = phy.memory_graph_version();
    assert!(
        old_phy_snapshot
            .communities
            .iter()
            .any(|community| community.members.contains(&b))
    );
    assert!(phy.community_stats().current);

    phy.tombstone_memory(b, 1).unwrap();
    assert_eq!(phy.memory_graph_version(), graph_version);
    assert!(!phy.community_stats().current);
    assert!(phy.community_snapshot().is_none());

    phy.sync().unwrap();
    drop(phy);
    let mut phy = Phylactery::open(&file).unwrap();
    assert_eq!(
        phy.community_stats().generations,
        old_phy_snapshot.generation
    );
    assert!(!phy.community_stats().current);
    assert!(phy.community_snapshot().is_none());

    let phy_snapshot = phy.refresh_communities_leiden().unwrap();
    assert_eq!(phy_snapshot.generation, old_phy_snapshot.generation + 1);
    assert_eq!(phy_snapshot.derived_graph_version, graph_version);
    let phy_members: Vec<_> = phy_snapshot
        .communities
        .iter()
        .flat_map(|community| community.members.iter().copied())
        .collect();
    assert!(!phy_members.contains(&b));
    assert!(phy_members.contains(&a));
    assert!(phy_members.contains(&c));
    assert!(phy.community_stats().current);
    assert_eq!(phy.community_snapshot(), Some(phy_snapshot.clone()));

    phy.sync().unwrap();
    drop(phy);
    let reopened_phy = Phylactery::open(&file).unwrap();
    assert_eq!(reopened_phy.community_snapshot(), Some(phy_snapshot));
    assert!(reopened_phy.community_stats().current);
}

#[test]
fn phylactery_has_independent_owner_local_communities() {
    let file = path("user.phy");
    let mut phy = Phylactery::create(&file).unwrap();
    let a = phy_memory(&mut phy, "a");
    let b = phy_memory(&mut phy, "b");
    let c = phy_memory(&mut phy, "c");
    let d = phy_memory(&mut phy, "d");
    phy.set_memory_relations(&[edge(a, b), edge(c, d)], 0)
        .unwrap();

    let snapshot = phy.refresh_communities_leiden().unwrap();
    assert_eq!(snapshot.communities.len(), 2);
    assert_eq!(snapshot.derived_graph_version, 1);
    phy.sync().unwrap();
    drop(phy);

    let reopened = Phylactery::open(&file).unwrap();
    assert_eq!(reopened.community_snapshot().unwrap(), snapshot);
    assert!(reopened.community_stats().current);
}

#[test]
fn v1_snapshot_reopens_and_explicit_refresh_upgrades_to_v2() {
    let file = path("community-v1-upgrade.prj.rel");
    let mut rel = Cva::create_project(&file).unwrap();
    let episode = rel_episode(&mut rel);
    let a = rel_memory(&mut rel, &episode, "legacy-a");
    let b = rel_memory(&mut rel, &episode, "legacy-b");
    rel.set_memory_relations(&[edge(a, b)], 0).unwrap();

    let owner_uuid = rel.owner_uuid().unwrap();
    let mut legacy = scan_merge_snapshot(&rel.graph, owner_uuid, 1).unwrap();
    legacy.algorithm_version = 1;
    rel.container
        .append(&encode_snapshot(&legacy).unwrap())
        .unwrap();
    rel.sync().unwrap();
    drop(rel);

    let mut reopened = Cva::open_project(&file).unwrap();
    assert_eq!(reopened.community_snapshot().unwrap().algorithm_version, 1);
    assert!(!reopened.community_stats().current);
    let upgraded = reopened.refresh_communities_leiden().unwrap();
    assert_eq!(upgraded.algorithm_version, COMMUNITY_ALGORITHM_VERSION);
    assert_eq!(upgraded.generation, 2);
    assert!(reopened.community_stats().current);
}

#[test]
fn recorded_coordinates_do_not_uniquely_reconstruct_accepted_membership() {
    let file = path("reconstruction-counterexample.phy");
    let mut phy = Phylactery::create(&file).unwrap();
    let ids = vec![
        phy_memory(&mut phy, "a"),
        phy_memory(&mut phy, "b"),
        phy_memory(&mut phy, "c"),
    ];
    phy.set_memory_relations(&[edge(ids[0], ids[1]), edge(ids[1], ids[2])], 0)
        .unwrap();
    let owner = phy.owner_uuid().unwrap();
    let cut = phy.memory_graph_version();
    phy.sync().unwrap();
    drop(phy);
    let make = |mut members: Vec<crate::MemoryId>| {
        members.sort();
        crate::Community {
            id: crate::community_store::community_id(owner, &members),
            members,
        }
    };
    let grouped = vec![make(ids.clone())];
    let mut split = ids.into_iter().map(|id| make(vec![id])).collect::<Vec<_>>();
    split.sort_by_key(|c| c.id);
    let mut accepted = Vec::new();
    for (name, communities) in [("grouped", grouped), ("split", split)] {
        let copy = file.with_extension(format!("{name}.phy"));
        fs::copy(&file, &copy).unwrap();
        let snapshot = crate::CommunitySnapshot {
            generation: 1,
            derived_graph_version: cut,
            algorithm_version: COMMUNITY_ALGORITHM_VERSION,
            seed: crate::COMMUNITY_LEIDEN_SEED,
            resolution: 1.0,
            quality: 0.0,
            communities,
        };
        let mut container = crate::Container::open(&copy).unwrap();
        container
            .append(&encode_snapshot(&snapshot).unwrap())
            .unwrap();
        container.sync().unwrap();
        drop(container);
        let reopened = Phylactery::open(&copy).unwrap();
        assert_eq!(reopened.community_snapshot(), Some(snapshot.clone()));
        accepted.push(snapshot);
    }
    assert_eq!(
        accepted[0].derived_graph_version,
        accepted[1].derived_graph_version
    );
    assert_ne!(accepted[0].communities, accepted[1].communities);
}
