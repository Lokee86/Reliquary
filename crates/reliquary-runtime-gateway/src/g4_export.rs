//! Read-only final cohort export for independent CLI and root-library processes.
use super::*;
use reliquary_owner_session::model::{MemoryId, PayloadState};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub(crate) fn export_completed_cohort(root: &Path, paths: [&Path; 4], mut witness: Value) {
    verify_semantic_reopen(paths, &witness["semantics"]);
    let destination = std::env::var_os("G4_EXPORT_DIR").map(std::path::PathBuf::from);
    if let Some(destination) = &destination {
        fs::create_dir_all(destination).unwrap();
    }
    let names = [
        "rel-lantern.rel",
        "rel-cascade.rel",
        "phy-reader-a.phy",
        "phy-reader-b.phy",
    ];
    let mut owners = Vec::new();
    for (index, (path, name)) in paths.into_iter().zip(names).enumerate() {
        let (id, accepted_sources, memory_count, proof) = if index < 2 {
            let reader = RelSession::open_path_current(path).unwrap();
            let proof = reader
                .historical_proof(reader.owner_id(), &reader.current_historical_cut())
                .unwrap();
            (
                reader.owner_id(),
                reader.accepted_turn_sources().len(),
                reader.memory_ids_page(None, 128).unwrap().1.len(),
                proof,
            )
        } else {
            let reader = PhySession::open_path_current(path).unwrap();
            let proof = reader
                .historical_proof(reader.owner_id(), reader.current_historical_cut())
                .unwrap();
            (
                reader.owner_id(),
                0,
                reader.memory_ids_page(None, 128).unwrap().1.len(),
                proof,
            )
        };
        assert!(
            memory_count > 0,
            "completed owner must have semantic Memory: {name}"
        );
        if let Some(destination) = &destination {
            let target = destination.join(name);
            assert!(!target.exists(), "export never replaces an existing owner");
            fs::copy(path, &target).unwrap();
            assert_eq!(fs::read(path).unwrap(), fs::read(&target).unwrap());
        }
        owners.push(json!({
            "file": name, "owner_id": id.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
            "kind": if index < 2 { "rel" } else { "phy" },
            "scope": match index { 0 => "project", 1 => "connection", _ => "unspecified" },
            "accepted_sources": accepted_sources, "memory_count": memory_count,
            "sha256": crate::hex(&Sha256::digest(fs::read(path).unwrap())),
            "history_digest": crate::hex(&proof.digest), "retained_history_records": proof.retained_records
        }));
    }
    witness["schema_version"] = json!(1);
    witness["owners"] = json!(owners);
    witness["semantic_reopen_verified"] = json!(true);
    witness["rewrite_policy"] =
        serde_json::to_value(reliquary_application::RewritePolicy::default()).unwrap();
    let Some(destination) = destination else {
        return;
    };
    for name in ["lifecycle-queue", "lifecycle-race-queue"] {
        copy_tree(&root.join(name), &destination.join(name));
    }
    let witness_path = destination.join("witness.json");
    assert!(
        !witness_path.exists(),
        "export never replaces an existing witness"
    );
    fs::write(witness_path, serde_json::to_vec_pretty(&witness).unwrap()).unwrap();
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn verify_semantic_reopen(paths: [&Path; 4], witness: &Value) {
    let rels = [
        RelSession::open_path_current(paths[0]).unwrap(),
        RelSession::open_path_current(paths[1]).unwrap(),
    ];
    let phys = [
        PhySession::open_path_current(paths[2]).unwrap(),
        PhySession::open_path_current(paths[3]).unwrap(),
    ];
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/g4-native-rel21-v1.json"
    ))
    .unwrap();
    verify_dream_and_entity_reopen(&rels[0], witness);
    let private = &witness["negative_and_private_entity"];
    let encoded = private["private_phy_memory_id"].as_str().unwrap();
    let memory_id =
        std::array::from_fn(|i| u8::from_str_radix(&encoded[i * 2..i * 2 + 2], 16).unwrap());
    let key = reliquary_owner_session::model::MemoryEntityMentionKey {
        memory_id,
        field: reliquary_owner_session::model::MemoryTextField::Content,
        start_byte: private["private_phy_mention_key"]["start_byte"]
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap(),
        end_byte: private["private_phy_mention_key"]["end_byte"]
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap(),
    };
    let mention = phys[0].mention(key).unwrap();
    let reliquary_owner_session::model::MemoryEntityResolutionStatus::Resolved {
        entity_id, ..
    } = mention.status
    else {
        panic!("private PHY Entity did not survive reopen")
    };
    assert_eq!(
        crate::hex(&entity_id.0),
        private["private_phy_entity_id"].as_str().unwrap()
    );
    assert!(phys[1].entity(entity_id).is_none());
    assert!(rels.iter().all(|rel| rel.entity(entity_id).is_none()));
    for expected in manifest["source_references"].as_array().unwrap() {
        let observed = witness["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["source_id"] == expected["id"])
            .unwrap();
        let encoded = observed["memory_id"].as_str().unwrap();
        assert_eq!(encoded.len(), 64);
        let id = MemoryId(std::array::from_fn(|i| {
            u8::from_str_radix(&encoded[i * 2..i * 2 + 2], 16).unwrap()
        }));
        let alias = expected["expected_route"]["owner"].as_str().unwrap();
        let record = match alias {
            "rel-lantern" => rels[0].current_memory(id).unwrap(),
            "rel-cascade" => rels[1].current_memory(id).unwrap(),
            "phy-reader-a" => {
                assert!(phys[1].memory_history(id).unwrap().is_none());
                assert!(
                    rels.iter()
                        .all(|rel| rel.memory_history(id).unwrap().is_none())
                );
                phys[0].current_memory(id).unwrap()
            }
            "phy-reader-b" => {
                assert!(phys[0].memory_history(id).unwrap().is_none());
                assert!(
                    rels.iter()
                        .all(|rel| rel.memory_history(id).unwrap().is_none())
                );
                phys[1].current_memory(id).unwrap()
            }
            other => panic!("unexpected source owner {other}"),
        };
        assert_eq!(json!(record.record.revision), observed["body_revision"]);
        if manifest["expected_state"]["active"]
            .as_array()
            .unwrap()
            .contains(&expected["id"])
        {
            assert!(
                !record.record.archived,
                "authored active source was archived: {}",
                expected["id"]
            );
            assert!(record.record.superseded_by.is_none());
        }
        let PayloadState::Available(body) = record.payload else {
            panic!("authored Memory has no retained body")
        };
        assert!(
            body.content
                .contains(expected["exact_span"].as_str().unwrap())
        );
        if expected["expected_route"]["owner_kind"] == "REL" {
            assert_eq!(
                record.record.source_node_id.as_deref(),
                expected["turn_id"].as_str()
            );
        } else {
            assert!(record.record.source_node_id.is_none());
            assert!(record.record.source_ref.is_some());
        }
    }
}

fn verify_dream_and_entity_reopen(rel: &RelSession, witness: &Value) {
    use reliquary_owner_session::model::{
        EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus,
        MemoryTextField, NodeRef, RelationKind, RelationOrigin,
    };
    let source_id = |alias: &str| {
        let source = witness["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["source_id"] == alias)
            .unwrap();
        let encoded = source["memory_id"].as_str().unwrap();
        MemoryId(std::array::from_fn(|i| {
            u8::from_str_radix(&encoded[i * 2..i * 2 + 2], 16).unwrap()
        }))
    };
    let release = source_id("src-release-label");
    let intro = source_id("src-lantern-intro");
    let frontier = rel
        .initial_dream_frontier("g4-authored-first-dream")
        .unwrap();
    assert_eq!(frontier.source.id, intro);
    assert_eq!(
        frontier
            .candidates
            .iter()
            .map(|cut| cut.id)
            .collect::<Vec<_>>(),
        vec![release]
    );
    assert!(rel.dream_pair_evaluated(release, intro));
    let duplicate_edges = rel
        .graph_relations()
        .into_iter()
        .filter(|edge| {
            edge.kind == RelationKind::DuplicateOf
                && [edge.source, edge.target].contains(&NodeRef::Memory(release.0))
                && [edge.source, edge.target].contains(&NodeRef::Memory(intro.0))
        })
        .collect::<Vec<_>>();
    assert_eq!(duplicate_edges.len(), 1);
    assert_eq!(duplicate_edges[0].origin, RelationOrigin::Dream);
    let canonical = rel.current_memory(release).unwrap().record;
    assert_eq!(canonical.lifecycle_state, "canonical");
    assert!(!canonical.archived);
    assert!(rel.current_memory(intro).unwrap().record.archived);
    let cobalt = source_id("src-cobalt-protocol");
    let mira = source_id("src-mira");
    assert!(rel.dream_pair_evaluated(cobalt, mira));
    assert!(!rel.graph_relations().iter().any(|edge| {
        [edge.source, edge.target].contains(&NodeRef::Memory(cobalt.0))
            && [edge.source, edge.target].contains(&NodeRef::Memory(mira.0))
    }));
    let pending = source_id("src-rehearsal-pending");
    let complete = source_id("src-rehearsal-complete");
    let old = rel.current_memory(pending).unwrap().record;
    assert!(old.archived);
    assert_eq!(old.lifecycle_state, "archived");
    assert_eq!(old.superseded_by, Some(complete));
    assert!(!rel.current_memory(complete).unwrap().record.archived);
    assert!(
        rel.graph_relation(
            NodeRef::Memory(complete.0),
            NodeRef::Memory(pending.0),
            RelationKind::Supersedes
        )
        .is_some()
    );
    assert!(rel.dream_pair_evaluated(pending, complete));
    let entity =
        serde_json::from_value::<[u8; 32]>(witness["entity"]["mira_entity_id"].clone()).unwrap();
    for memory in [mira, source_id("src-mira-known")] {
        let key = MemoryEntityMentionKey {
            memory_id: memory.0,
            field: MemoryTextField::Content,
            start_byte: 0,
            end_byte: 4,
        };
        assert!(matches!(rel.mention(key).unwrap().status,
            MemoryEntityResolutionStatus::Resolved { entity_id, .. } if entity_id.0 == entity));
    }
    let ambiguous = MemoryEntityMentionKey {
        memory_id: mira.0,
        field: MemoryTextField::Content,
        start_byte: 10,
        end_byte: 20,
    };
    assert!(matches!(
        rel.mention(ambiguous).unwrap().status,
        MemoryEntityResolutionStatus::Ambiguous { .. }
            | MemoryEntityResolutionStatus::Unresolved {
                reason: EntityResolutionReason::Ambiguous
            }
    ));
}
