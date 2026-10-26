use super::*;
use crate::graph_compact_codec::publication::{self, GraphPublication, GraphTransaction};
use crate::graph_compact_contract::GraphOrdinalMutation;
use crate::{
    GraphRelationKind, GraphRelationOrigin, MemoryId, SemanticGraphRelationKind, SemanticNodeRef,
};
use reliquary_container::publication_envelope::{PublicationClock, PublicationEnvelope};

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
            semantic_node: SemanticNodeRef::memory([id as u8 + 1; 32]),
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
