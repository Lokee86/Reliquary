use super::whole_owner_at;
use crate::{Cva, EpisodeBoundary, EpisodeConfig, EpisodeOrigin, GraphRelationKind, MemoryDraft};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!("whole-owner-lineage-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn publish(cva: &mut Cva, name: &str, episode: crate::Episode) -> crate::MemoryId {
    cva.publish_memory(
        None,
        0,
        MemoryDraft {
            category: "fact".into(),
            memory_type: "project".into(),
            authority_kind: "unknown".into(),
            temporal_status: "unknown".into(),
            title: name.into(),
            content: name.into(),
            scope: "private".into(),
            lifecycle_state: "active".into(),
            archived: false,
            superseded_by: None,
            parent_id: None,
            source_node_id: Some("user".into()),
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: Some(episode.id),
            source_time_ns: None,
            mutation_id: name.into(),
            created_at_ns: 1,
            updated_at_ns: 1,
        },
    )
    .unwrap()
    .0
    .id
}

fn episode(cva: &mut Cva) -> crate::Episode {
    cva.append_node(
        "user".into(),
        "lineage".into(),
        None,
        "user".into(),
        10,
        "facts",
    )
    .unwrap();
    cva.append_node(
        "assistant".into(),
        "lineage".into(),
        Some("user".into()),
        "assistant".into(),
        20,
        "noted",
    )
    .unwrap();
    cva.materialize_path_episodes(
        "lineage",
        "assistant",
        EpisodeConfig::default(),
        EpisodeOrigin::Live,
        Some((EpisodeBoundary::Inactivity, 30)),
    )
    .unwrap()
    .created
    .into_iter()
    .next()
    .unwrap()
}

#[test]
fn whole_owner_lineage_fingerprint_tracks_validated_rewrite_baseline_and_rejects_forgery() {
    let fixture = Fixture::new();
    let source_path = fixture.path("source.rel");
    let rewritten_path = fixture.path("rewritten.rel");
    let forged_path = fixture.path("forged.rel");
    let mut source = Cva::create_project(&source_path).unwrap();
    let ep = episode(&mut source);
    let a = publish(&mut source, "baseline-a", ep.clone());
    let b = publish(&mut source, "baseline-b", ep);
    source
        .set_memory_relation(
            a,
            b,
            GraphRelationKind::Factual,
            true,
            source.graph_version(),
        )
        .unwrap();
    source
        .set_memory_relation(
            a,
            b,
            GraphRelationKind::Factual,
            false,
            source.graph_version(),
        )
        .unwrap();
    source.sync().unwrap();
    drop(source);

    crate::rewrite::rewrite_file(
        &source_path,
        &rewritten_path,
        &crate::RewriteRequest::default(),
    )
    .unwrap();
    let mut rewritten = Cva::open(&rewritten_path).unwrap();
    let genesis = rewritten
        .rewrite_genesis()
        .expect("rewrite must carry validated genesis");
    let baseline = rewritten
        .graph
        .historical_baseline_identity()
        .expect("rewrite must carry Graph baseline");
    assert_eq!(baseline.0, genesis.destination_owner);
    assert_eq!(baseline.1, genesis.generation);
    assert_eq!(baseline.2, genesis.policy_digest);
    let head = rewritten.latest_global_version();
    let cut = whole_owner_at(&mut rewritten, head).unwrap();
    let repeated = whole_owner_at(&mut rewritten, head).unwrap();
    assert_eq!(cut.fingerprint, repeated.fingerprint);
    assert_eq!(cut.cuts, repeated.cuts);
    assert_eq!(cut.ego_version, repeated.ego_version);
    assert_eq!(cut.current_memories, repeated.current_memories);
    assert_eq!(cut.visible_graph, repeated.visible_graph);
    drop(rewritten);
    let mut reopened = Cva::open(&rewritten_path).unwrap();
    let reopened_cut = whole_owner_at(&mut reopened, head).unwrap();
    assert_eq!(cut.fingerprint, reopened_cut.fingerprint);
    assert_eq!(cut.cuts, reopened_cut.cuts);
    assert_eq!(cut.ego_version, reopened_cut.ego_version);
    assert_eq!(cut.current_memories, reopened_cut.current_memories);
    assert_eq!(cut.visible_graph, reopened_cut.visible_graph);
    drop(reopened);

    // Change only the Graph baseline's policy identity, preserving its valid
    // checksum. Reopen must reject its mismatch with the authentic genesis.
    fs::copy(&rewritten_path, &forged_path).unwrap();
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&forged_path)
        .unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    let marker = b"CVAGBL01";
    let start = bytes
        .windows(marker.len())
        .position(|window| window == marker)
        .expect("baseline frame");
    let payload_start = start;
    let payload_len = 108 + 2 * 32;
    assert_eq!(bytes[payload_start + 40], baseline.2[0]);
    bytes[payload_start + 40] ^= 0x01;
    let checksum_start = payload_start + payload_len - 32;
    let checksum = Sha256::digest(&bytes[payload_start..checksum_start]);
    bytes[checksum_start..payload_start + payload_len].copy_from_slice(&checksum);
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(&bytes).unwrap();
    file.sync_all().unwrap();
    assert!(Cva::open(&forged_path).is_err());
}
