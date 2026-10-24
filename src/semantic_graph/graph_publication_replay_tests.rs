use super::*;
use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope};
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    GraphRelationKind, GraphRelationOrigin, MemoryId, SemanticGraphRelationKind, SemanticNodeRef,
};

fn object(offset: u64, len: u64) -> ObjectRef {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&offset.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    ObjectRef::from_legacy_bytes(bytes)
}
fn initialized() -> GraphOpenState {
    let mut state = GraphOpenState::new();
    state
        .ingest(object(40, 12), &crate::graph_codec::encode_format(), 0)
        .unwrap();
    for id in 0..2 {
        let bytes = crate::graph_codec::encode_node(crate::graph_codec::GraphNodePayload {
            semantic_node: SemanticNodeRef::memory(MemoryId([id as u8 + 1; 32])),
            node_id: arcana::NodeId(id),
        });
        state
            .ingest(object(60 + id as u64 * 60, bytes.len() as u64), &bytes, 0)
            .unwrap();
    }
    state
}
fn row() -> GraphOrdinalMutation {
    GraphOrdinalMutation {
        source: 0,
        target: 1,
        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
        active: true,
        origin: GraphRelationOrigin::User,
    }
}
fn admit(
    state: &mut GraphOpenState,
    local: u64,
    global: u64,
    frontier: u64,
    rows: Vec<GraphOrdinalMutation>,
) -> Result<bool, GraphError> {
    let body = publication::encode(&GraphPublication {
        graph_version: local,
        transaction: GraphTransaction::Batch(rows),
    })?;
    state.ingest_publication_envelope(
        PublicationEnvelope {
            global_version: global,
            clock: PublicationClock::BindReserved,
            body: &body,
        },
        frontier,
    )
}
fn snapshot(state: &GraphOpenState) -> (u64, u64, usize, usize, usize) {
    let stats = state.store.stats();
    (
        stats.graph_version,
        stats.memory_graph_version,
        stats.active_relations,
        stats.relation_mutations,
        stats.nodes,
    )
}

#[test]
fn native_admission_rejects_missing_mappings_duplicates_and_bad_contracts_before_mutation() {
    for rows in [
        vec![row(), GraphOrdinalMutation { target: 2, ..row() }],
        vec![row(), row()],
        vec![
            row(),
            GraphOrdinalMutation {
                kind: SemanticGraphRelationKind::EntityAssociation,
                origin: GraphRelationOrigin::Perception,
                ..row()
            },
        ],
        vec![
            row(),
            GraphOrdinalMutation {
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Causal),
                origin: GraphRelationOrigin::Perception,
                ..row()
            },
        ],
    ] {
        let mut state = initialized();
        let before = snapshot(&state);
        assert!(admit(&mut state, 1, 1, 1, rows).is_err());
        assert_eq!(snapshot(&state), before);
    }
}

#[test]
fn native_owner_clocks_reject_gaps_reversed_reservations_and_future_global_without_consuming_cut() {
    let mut state = initialized();
    admit(&mut state, 1, 3, 3, vec![row()]).unwrap();
    let before = snapshot(&state);
    for (local, global, frontier) in [(3, 4, 4), (2, 2, 3), (2, 3, 3), (2, 4, 3)] {
        assert!(
            admit(
                &mut state,
                local,
                global,
                frontier,
                vec![GraphOrdinalMutation {
                    active: false,
                    ..row()
                }]
            )
            .is_err()
        );
        assert_eq!(snapshot(&state), before);
    }
    admit(
        &mut state,
        2,
        4,
        4,
        vec![GraphOrdinalMutation {
            active: false,
            ..row()
        }],
    )
    .unwrap();
    assert_eq!(state.store.graph_version(), 2);
    assert_eq!(state.store.memory_graph_version(), 2);
}

#[test]
fn graph_has_no_repeat_link_and_bare_native_or_foreign_body_is_not_admitted() {
    let mut state = initialized();
    let carrier = crate::graph_compact_codec::encode_mutation(row());
    let reference = object(200, carrier.len() as u64);
    state.ingest(reference, &carrier, 1).unwrap();
    let marker = crate::graph_codec::encode_version(crate::graph_codec::GraphVersionPayload {
        global_version: 1,
        graph_version: 1,
        mutation: reference,
    });
    state.ingest(object(300, 40), &marker, 1).unwrap();
    let repeated = crate::graph_codec::encode_version(crate::graph_codec::GraphVersionPayload {
        global_version: 2,
        graph_version: 2,
        mutation: reference,
    });
    state.ingest(object(400, 40), &repeated, 2).unwrap();
    let before = snapshot(&state);
    let bare = publication::encode(&GraphPublication {
        graph_version: 2,
        transaction: GraphTransaction::Single(row()),
    })
    .unwrap();
    assert!(
        state
            .ingest(object(500, bare.len() as u64), &bare, 2)
            .is_ok()
    );
    assert!(
        !state
            .ingest_publication_envelope(
                PublicationEnvelope {
                    global_version: 2,
                    clock: PublicationClock::BindReserved,
                    body: b"CVAMC002"
                },
                2
            )
            .unwrap()
    );
    assert_eq!(snapshot(&state), before);
}

#[test]
fn rel_and_phy_dispatch_admit_single_and_mixed_origin_batch_exactly_once() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".test-tmp")
        .join(format!("graph-native-open-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    macro_rules! seed {
        ($owner:expr) => {{
            let mut owner = $owner;
            let a = owner
                .publish_memory(
                    None,
                    0,
                    crate::entity_candidate_test_support::memory_draft("a", "a", "a body"),
                )
                .unwrap()
                .0
                .id;
            let b = owner
                .publish_memory(
                    None,
                    0,
                    crate::entity_candidate_test_support::memory_draft("b", "b", "b body"),
                )
                .unwrap()
                .0
                .id;
            owner
                .owner
                .graph
                .set_relation_with_origin(
                    &mut owner.owner.container,
                    a,
                    b,
                    GraphRelationKind::Factual,
                    true,
                    GraphRelationOrigin::User,
                    0,
                )
                .unwrap();
            let stable = GraphMutationPayload {
                source: SemanticNodeRef::memory(a),
                target: SemanticNodeRef::memory(b),
                kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Factual),
                active: false,
                origin: GraphRelationOrigin::User,
            };
            let compact = owner
                .graph
                .encode_compact_mutations(&[stable], true)
                .unwrap();
            let row = crate::graph_compact_codec::decode(&compact)
                .unwrap()
                .unwrap()[0];
            let body = publication::encode(&GraphPublication {
                graph_version: 2,
                transaction: GraphTransaction::Single(row),
            })
            .unwrap();
            owner.container.append_publication_envelope(&body).unwrap();
            let body = publication::encode(&GraphPublication {
                graph_version: 3,
                transaction: GraphTransaction::Batch(vec![
                    GraphOrdinalMutation {
                        origin: GraphRelationOrigin::Dream,
                        ..row
                    },
                    GraphOrdinalMutation {
                        kind: SemanticGraphRelationKind::Memory(GraphRelationKind::Topical),
                        active: true,
                        ..row
                    },
                ]),
            })
            .unwrap();
            owner
                .container
                .set_next_transaction_time_override(Some(-17));
            let (last, _) = owner.container.append_publication_envelope(&body).unwrap();
            owner.sync().unwrap();
            drop(owner);
            last
        }};
    }
    let rel = dir.join("test.rel");
    let phy = dir.join("test.phy");
    let last = seed!(crate::Cva::create_project(&rel).unwrap());
    let owner = crate::Cva::open_project(&rel).unwrap();
    assert_eq!(owner.graph.graph_version(), 3);
    assert_eq!(owner.graph.memory_graph_version(), 3);
    assert_eq!(owner.graph.stats().relation_mutations, 4);
    assert_eq!(owner.graph.stats().active_relations, 1);
    assert_eq!(owner.transaction_time_ns(last), Some(-17));
    drop(owner);
    let last = seed!(crate::Phylactery::create(&phy).unwrap());
    let owner = crate::Phylactery::open(&phy).unwrap();
    assert_eq!(owner.graph.graph_version(), 3);
    assert_eq!(owner.graph.memory_graph_version(), 3);
    assert_eq!(owner.graph.stats().relation_mutations, 4);
    assert_eq!(owner.graph.stats().active_relations, 1);
    assert_eq!(owner.transaction_time_ns(last), Some(-17));
    drop(owner);
    std::fs::remove_dir_all(dir).unwrap();
}
