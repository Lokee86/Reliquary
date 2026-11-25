//! Private checkpoint projection of the authoritative Graph records.
use super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize)]
struct CheckpointRef<'a> {
    owner: [u8; 16],
    identity: ContainerIdentity,
    history: &'a [Relation],
    user_associations: &'a BTreeMap<[u8; 32], UserAssociation>,
}

#[derive(Deserialize)]
struct Checkpoint {
    owner: [u8; 16],
    identity: ContainerIdentity,
    history: Vec<Relation>,
    user_associations: BTreeMap<[u8; 32], UserAssociation>,
}

impl Serialize for GraphStore {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        CheckpointRef {
            owner: self.owner,
            identity: self.identity,
            history: &self.history,
            user_associations: &self.user_associations,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GraphStore {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let checkpoint = Checkpoint::deserialize(deserializer)?;
        let mut store = Self::empty(checkpoint.owner, checkpoint.identity);
        let mut states = BTreeMap::new();
        for relation in &checkpoint.history {
            let version = relation.graph_version;
            // Grouped publications can contain several relations at one cut.
            if version == 0 || version < store.graph_version || version - store.graph_version > 1 {
                return Err(serde::de::Error::custom("invalid Graph checkpoint version"));
            }
            store.graph_version = version;
            states.insert(Key::for_relation(relation), *relation);
        }
        for (&key, relation) in &states {
            store.index_active_relation(key, relation);
        }
        store.states = states;
        store.history = checkpoint.history;
        store.user_associations = checkpoint.user_associations;
        Ok(store)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_container::ReliquaryScope;

    #[test]
    fn checkpoint_rebuilds_graph_indexes_and_preserves_association_receipts() {
        let owner = [1; 16];
        let identity = ContainerIdentity::reliquary(ReliquaryScope::Project);
        let [a, b, c] = [[1; 32], [2; 32], [3; 32]];
        let admission = GraphAdmission::for_changes(owner, [a, b, c], []);
        let mut store = GraphStore::empty(owner, identity);
        let first = Relation {
            source: NodeRef::Memory(a),
            target: NodeRef::Memory(b),
            kind: RelationKind::DuplicateOf,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: 1,
        };
        let second = Relation {
            target: NodeRef::Memory(c),
            ..first
        };
        store
            .replay_clocked(
                ClockedGraphPublication {
                    owner,
                    graph_version: 1,
                    global_version: 1,
                    bound_reservation: false,
                    transaction_time_ns_in_envelope: None,
                    relations: vec![first, second],
                },
                &admission,
            )
            .unwrap();
        store
            .apply_dream_relation(
                &admission,
                Relation {
                    graph_version: 2,
                    active: false,
                    ..first
                },
            )
            .unwrap();

        let associate = UserAssociation {
            request_key: [1; 32],
            fingerprint: [2; 32],
            source: AssociationEndpoint {
                id: a,
                revision: 1,
                body: [3; 32],
            },
            target: AssociationEndpoint {
                id: b,
                revision: 1,
                body: [4; 32],
            },
            graph_version: 2,
            expected_edge_revision: None,
            active: true,
        };
        let retract = UserAssociation {
            request_key: [2; 32],
            graph_version: 3,
            expected_edge_revision: Some(3),
            active: false,
            ..associate
        };
        for effect in [associate, retract] {
            store.replay_user_association(&admission, effect).unwrap();
        }

        let bytes = rmp_serde::to_vec_named(&store).unwrap();
        let full_bytes = rmp_serde::to_vec_named(&(
            CheckpointRef {
                owner,
                identity,
                history: &store.history,
                user_associations: &store.user_associations,
            },
            &store.states,
            &store.outgoing,
            &store.incoming,
            &store.duplicate_incident,
        ))
        .unwrap();
        assert!(bytes.len() * 4 < full_bytes.len() * 3);

        let mut restored: GraphStore = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(restored.owner, owner);
        assert_eq!(restored.identity, identity);
        assert_eq!(restored.graph_version(), 4);
        assert_eq!(restored.history, store.history);
        assert_eq!(restored.history_at(1).unwrap(), [first, second]);
        assert_eq!(restored.states, store.states);
        assert_eq!(restored.outgoing(NodeRef::Memory(a)), [second]);
        assert_eq!(restored.incoming(NodeRef::Memory(c)), [second]);
        assert_eq!(
            restored.duplicate_context(&[a], 3).unwrap(),
            (BTreeSet::from([a, c]), vec![second])
        );
        assert_eq!(
            restored.user_association(associate.request_key),
            Some(associate)
        );
        assert_eq!(
            restored.user_association(retract.request_key),
            Some(retract)
        );

        let reactivate = UserAssociation {
            request_key: [3; 32],
            graph_version: 4,
            expected_edge_revision: Some(4),
            ..associate
        };
        restored
            .replay_user_association(&admission, reactivate)
            .unwrap();
        assert_eq!(restored.graph_version(), 5);
        assert!(
            restored
                .outgoing(NodeRef::Memory(a))
                .contains(&reactivate.relation())
        );
    }

    #[test]
    fn checkpoint_roundtrips_empty_history_and_rejects_invalid_version_sequences() {
        let identity = ContainerIdentity::reliquary(ReliquaryScope::Project);
        let mut store = GraphStore::empty([1; 16], identity);
        let bytes = rmp_serde::to_vec_named(&store).unwrap();
        let empty: GraphStore = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(empty.graph_version(), 0);
        assert!(empty.history.is_empty());

        store.history.push(Relation {
            source: NodeRef::Memory([1; 32]),
            target: NodeRef::Memory([2; 32]),
            kind: RelationKind::DuplicateOf,
            active: true,
            origin: RelationOrigin::Dream,
            graph_version: 2,
        });
        let bytes = rmp_serde::to_vec_named(&store).unwrap();
        assert!(rmp_serde::from_slice::<GraphStore>(&bytes).is_err());
    }
}
