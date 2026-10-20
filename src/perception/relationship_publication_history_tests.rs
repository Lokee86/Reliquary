//! Historical parity between Relationship marker-bound publications and envelopes.
use crate::container::publication_envelope::{PublicationClock, PublicationEnvelope};
use crate::relationship_codec::{
    RelationshipTombstone, RelationshipVersion, encode_tombstone, encode_version,
};
use crate::relationship_compact_codec::publication::{self, RelationshipPublication};
use crate::relationship_compact_codec::{encode_creation, encode_delta};
use crate::relationship_compact_contract::{RelationshipCreation, RelationshipDelta};
use crate::relationship_model::RelationshipRecord;
use crate::relationship_store::RelationshipStore;
use crate::{
    Container, Cva, EntityId, EntityRef, MemoryDraft, MemoryId, MemoryRef, Phylactery,
    RelationshipId, RelationshipParticipant,
};
use std::fs;
use std::path::PathBuf;

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".test-tmp")
            .join(format!(
                "relationship-publication-history-{}",
                uuid::Uuid::new_v4()
            ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone, Copy)]
enum WireFormat {
    MarkerBound,
    Envelope,
}

trait HistoryOwner {
    fn container(&mut self) -> &mut Container;
    fn publish_interleaved_memory(&mut self) -> Result<(), String>;
    fn relationship_store(&self) -> &RelationshipStore;
    fn sync_history(&mut self) -> Result<(), String>;
}

impl HistoryOwner for Cva {
    fn container(&mut self) -> &mut Container {
        &mut self.container
    }
    fn publish_interleaved_memory(&mut self) -> Result<(), String> {
        self.container.set_next_transaction_time_override(Some(300));
        self.publish_memory(None, 0, memory_draft())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    fn relationship_store(&self) -> &RelationshipStore {
        &self.relationships
    }
    fn sync_history(&mut self) -> Result<(), String> {
        self.sync().map_err(|error| error.to_string())
    }
}

impl HistoryOwner for Phylactery {
    fn container(&mut self) -> &mut Container {
        &mut self.container
    }
    fn publish_interleaved_memory(&mut self) -> Result<(), String> {
        self.container.set_next_transaction_time_override(Some(300));
        self.publish_memory(None, 0, memory_draft())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    fn relationship_store(&self) -> &RelationshipStore {
        &self.relationships
    }
    fn sync_history(&mut self) -> Result<(), String> {
        self.sync().map_err(|error| error.to_string())
    }
}

fn memory_draft() -> MemoryDraft {
    MemoryDraft {
        category: "fact".into(),
        memory_type: "project".into(),
        authority_kind: "direct".into(),
        temporal_status: "current".into(),
        title: "Interleaved memory".into(),
        content: "An unrelated owner publication between Relationship revisions.".into(),
        scope: "project".into(),
        lifecycle_state: "knowledge".into(),
        archived: false,
        superseded_by: None,
        parent_id: None,
        source_node_id: None,
        content_source_conversation_id: None,
        content_source_node_id: None,
        grounding_source_conversation_id: None,
        grounding_source_node_id: None,
        source_episode_id: None,
        source_time_ns: None,
        mutation_id: "interleaved-memory".into(),
        created_at_ns: 5,
        updated_at_ns: 5,
    }
}

fn record(revision: u64, global: u64, local: u64, mutation: &str) -> RelationshipRecord {
    RelationshipRecord {
        id: RelationshipId([0xA7; 32]),
        revision,
        kind: "observes".into(),
        participants: vec![
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "external-entity-a".into(),
                    entity_id: EntityId([0x11; 32]),
                },
                role: Some("source".into()),
            },
            RelationshipParticipant {
                entity: EntityRef {
                    owner_id: "external-entity-b".into(),
                    entity_id: EntityId([0x22; 32]),
                },
                role: None,
            },
        ],
        evidence: vec![MemoryRef {
            owner_id: "external-memory-a".into(),
            memory_id: MemoryId([0x33; 32]),
        }],
        summary: format!("Exact summary at revision {revision}"),
        mutation_id: mutation.into(),
        created_at_ns: 10,
        updated_at_ns: 20 + revision as i64,
        global_version: global,
        relationship_version: local,
    }
}

fn reserve(container: &mut Container, global: u64, time_ns: i64) {
    container.set_next_transaction_time_override(Some(time_ns));
    assert_eq!(container.allocate_version().unwrap(), global);
}

fn append_reserved(
    container: &mut Container,
    format: WireFormat,
    global: u64,
    publication: &RelationshipPublication,
) {
    match format {
        WireFormat::Envelope => {
            let body = publication::encode(publication).unwrap();
            let bytes = crate::container::publication_envelope::encode(PublicationEnvelope {
                global_version: global,
                clock: PublicationClock::BindReserved,
                body: &body,
            })
            .unwrap();
            container.append(&bytes).unwrap();
        }
        WireFormat::MarkerBound => {
            let (body, local) = match publication {
                RelationshipPublication::Creation(creation) => (
                    encode_creation(creation).unwrap(),
                    creation.record.relationship_version,
                ),
                RelationshipPublication::Delta(delta) => {
                    (encode_delta(delta).unwrap(), delta.relationship_version)
                }
                RelationshipPublication::Tombstone {
                    id,
                    revision,
                    relationship_version,
                } => (
                    encode_tombstone(RelationshipTombstone {
                        id: *id,
                        revision: *revision,
                    }),
                    *relationship_version,
                ),
            };
            let carrier = container.append(&body).unwrap();
            container
                .append(&encode_version(RelationshipVersion {
                    global_version: global,
                    relationship_version: local,
                    record: carrier,
                }))
                .unwrap();
        }
    }
}

fn append_allocated_terminal(container: &mut Container, format: WireFormat, id: RelationshipId) {
    match format {
        WireFormat::Envelope => {
            let body = publication::encode(&RelationshipPublication::Tombstone {
                id,
                revision: 3,
                relationship_version: 3,
            })
            .unwrap();
            container.set_next_transaction_time_override(Some(400));
            assert_eq!(container.append_publication_envelope(&body).unwrap().0, 4);
        }
        WireFormat::MarkerBound => {
            let carrier = container
                .append(&encode_tombstone(RelationshipTombstone { id, revision: 3 }))
                .unwrap();
            container.set_next_transaction_time_override(Some(400));
            assert_eq!(container.allocate_version().unwrap(), 4);
            container
                .append(&encode_version(RelationshipVersion {
                    global_version: 4,
                    relationship_version: 3,
                    record: carrier,
                }))
                .unwrap();
        }
    }
}

fn write_history(owner: &mut impl HistoryOwner, format: WireFormat) -> RelationshipId {
    let first = record(1, 2, 1, "create");
    {
        let container = owner.container();
        reserve(container, 1, 100);
        reserve(container, 2, 200);
        append_reserved(
            container,
            format,
            2,
            &RelationshipPublication::Creation(RelationshipCreation::new(first.clone()).unwrap()),
        );
    }

    // A real Memory write occupies global ticket 3 between local Relationship cuts.
    owner.publish_interleaved_memory().unwrap();
    assert_eq!(owner.container().latest_version(), 3);

    // Local revision two is physically next but binds to earlier reserved global 1.
    let mut second = first.clone();
    second.revision = 2;
    second.global_version = 1;
    second.relationship_version = 2;
    second.mutation_id = "revise".into();
    second.participants[0].role = Some("observer".into());
    second.evidence = vec![MemoryRef {
        owner_id: "external-memory-b".into(),
        memory_id: MemoryId([0x44; 32]),
    }];
    second.summary = "Exact revised summary".into();
    second.updated_at_ns = 31;
    append_reserved(
        owner.container(),
        format,
        1,
        &RelationshipPublication::Delta(RelationshipDelta::between(&first, &second).unwrap()),
    );
    append_allocated_terminal(owner.container(), format, first.id);
    owner.sync_history().unwrap();
    first.id
}

fn assert_relationship_cuts_equal(
    left_container: &Container,
    left: &RelationshipStore,
    right_container: &Container,
    right: &RelationshipStore,
) {
    assert_eq!(left.relationship_version(), right.relationship_version());
    for cut in 0..=left.relationship_version() {
        let a =
            crate::historical_relationship_fingerprint::relationship_cut(left_container, left, cut)
                .unwrap();
        let b = crate::historical_relationship_fingerprint::relationship_cut(
            right_container,
            right,
            cut,
        )
        .unwrap();
        assert_eq!(a.fingerprint, b.fingerprint, "Relationship cut {cut}");
        assert_eq!(
            a.current, b.current,
            "current Relationship set at cut {cut}"
        );
    }
    assert_eq!(
        left_container.latest_version(),
        right_container.latest_version()
    );
    for global in 0..=left_container.latest_version() {
        assert_eq!(
            left_container.transaction_time_ns(global),
            right_container.transaction_time_ns(global),
            "global transaction time at ticket {global}"
        );
    }
}

fn compare_rel(source_path: PathBuf, envelope_path: PathBuf, id: RelationshipId) {
    let mut source = Cva::open(source_path).unwrap();
    let mut envelope = Cva::open(envelope_path).unwrap();
    assert_eq!(source.owner_uuid(), envelope.owner_uuid());
    assert_eq!(
        source.relationships.records(),
        envelope.relationships.records()
    );
    assert_eq!(
        source.relationship_retention_descriptor(id).unwrap(),
        envelope.relationship_retention_descriptor(id).unwrap()
    );
    assert_relationship_cuts_equal(
        &source.container,
        &source.relationships,
        &envelope.container,
        &envelope.relationships,
    );
    for global in 0..=source.latest_global_version() {
        let source_cuts =
            crate::historical_global_cuts::reliquary_global_cuts(&source, global).unwrap();
        let envelope_cuts =
            crate::historical_global_cuts::reliquary_global_cuts(&envelope, global).unwrap();
        assert_eq!(
            source_cuts, envelope_cuts,
            "REL owner cuts at global {global}"
        );
        let source_relationship = crate::historical_relationship_fingerprint::relationship_cut(
            &source.container,
            &source.relationships,
            source_cuts.relationship,
        )
        .unwrap();
        let envelope_relationship = crate::historical_relationship_fingerprint::relationship_cut(
            &envelope.container,
            &envelope.relationships,
            envelope_cuts.relationship,
        )
        .unwrap();
        assert_eq!(
            source_relationship.fingerprint, envelope_relationship.fingerprint,
            "REL Relationship facts at global cut {global}"
        );
        let source_whole =
            crate::historical_whole_owner_fingerprint::whole_owner_at(&mut source, global).unwrap();
        let envelope_whole =
            crate::historical_whole_owner_fingerprint::whole_owner_at(&mut envelope, global)
                .unwrap();
        assert_eq!(
            source_whole.fingerprint, envelope_whole.fingerprint,
            "REL physical global frontier {global}"
        );
    }
}

fn compare_phy(source_path: PathBuf, envelope_path: PathBuf, id: RelationshipId) {
    let mut source = Phylactery::open(source_path).unwrap();
    let mut envelope = Phylactery::open(envelope_path).unwrap();
    assert_eq!(source.owner_uuid(), envelope.owner_uuid());
    assert_eq!(
        source.relationships.records(),
        envelope.relationships.records()
    );
    assert_eq!(
        source.relationship_retention_descriptor(id).unwrap(),
        envelope.relationship_retention_descriptor(id).unwrap()
    );
    assert_relationship_cuts_equal(
        &source.container,
        &source.relationships,
        &envelope.container,
        &envelope.relationships,
    );
    for global in 0..=source.latest_global_version() {
        let source_cuts =
            crate::historical_global_cuts::phylactery_global_cuts(&source, global).unwrap();
        let envelope_cuts =
            crate::historical_global_cuts::phylactery_global_cuts(&envelope, global).unwrap();
        assert_eq!(
            source_cuts, envelope_cuts,
            "PHY owner cuts at global {global}"
        );
        let source_relationship = crate::historical_relationship_fingerprint::relationship_cut(
            &source.container,
            &source.relationships,
            source_cuts.relationship,
        )
        .unwrap();
        let envelope_relationship = crate::historical_relationship_fingerprint::relationship_cut(
            &envelope.container,
            &envelope.relationships,
            envelope_cuts.relationship,
        )
        .unwrap();
        assert_eq!(
            source_relationship.fingerprint, envelope_relationship.fingerprint,
            "PHY Relationship facts at global cut {global}"
        );
        let source_whole =
            crate::historical_whole_phy_fingerprint::whole_owner_at_phy(&mut source, global)
                .unwrap();
        let envelope_whole =
            crate::historical_whole_phy_fingerprint::whole_owner_at_phy(&mut envelope, global)
                .unwrap();
        assert_eq!(
            source_whole.fingerprint, envelope_whole.fingerprint,
            "PHY physical global frontier {global}"
        );
    }
}

#[test]
fn rel_marker_bound_and_envelope_histories_match_each_local_and_global_cut() {
    let scratch = Scratch::new();
    let uuid = [0xA1; 16];
    let mut marker =
        Cva::create_rel_with_uuid(scratch.file("marker.rel"), uuid, Some("Project".into()))
            .unwrap();
    let mut envelope =
        Cva::create_rel_with_uuid(scratch.file("envelope.rel"), uuid, Some("Project".into()))
            .unwrap();
    let marker_id = write_history(&mut marker, WireFormat::MarkerBound);
    let envelope_id = write_history(&mut envelope, WireFormat::Envelope);
    assert_eq!(marker_id, envelope_id);
    drop(marker);
    drop(envelope);
    compare_rel(
        scratch.file("marker.rel"),
        scratch.file("envelope.rel"),
        marker_id,
    );
}

#[test]
fn phy_marker_bound_and_envelope_histories_match_each_local_and_global_cut() {
    let scratch = Scratch::new();
    let uuid = [0xB2; 16];
    let mut marker = Phylactery::create_with_uuid(scratch.file("marker.phy"), uuid).unwrap();
    let mut envelope = Phylactery::create_with_uuid(scratch.file("envelope.phy"), uuid).unwrap();
    let marker_id = write_history(&mut marker, WireFormat::MarkerBound);
    let envelope_id = write_history(&mut envelope, WireFormat::Envelope);
    assert_eq!(marker_id, envelope_id);
    drop(marker);
    drop(envelope);
    compare_phy(
        scratch.file("marker.phy"),
        scratch.file("envelope.phy"),
        marker_id,
    );
}
