use super::*;
use reliquary_owner_session::{
    CreateRelationshipRequest, EntityMentionCommit, RelationshipParticipantEvidence,
    model::{
        EntityResolutionReason, MemoryEntityMentionKey, MemoryEntityResolutionStatus, MemoryId,
        MemoryTextField,
    },
};

pub(super) struct ImportDreamEndpoint(pub &'static str);
impl GeneralEndpoint for ImportDreamEndpoint {
    fn model(&self) -> &str {
        "offline-import-dream"
    }

    fn complete_json(
        &self,
        _: &str,
        _: &str,
        schema: &str,
        _: &Value,
    ) -> Result<Value, GeneralEndpointError> {
        Ok(if schema == "dream_pair_verification" {
            json!({"relation_supported":self.0,"direction_supported":self.0,"evidence_supported":self.0})
        } else {
            json!({"relation":"topical","direction":"undirected","evidence":[
                {"side":"a","quote":"content"},{"side":"b","quote":"content"}
            ]})
        })
    }
}

pub(super) struct ImportEmbedding;
impl crate::EmbeddingEndpoint for ImportEmbedding {
    fn dimensions(&self) -> u32 {
        2
    }
    fn normalization(&self) -> crate::VectorNormalization {
        crate::VectorNormalization::L2
    }
    fn supports_bounded_embedding(&self) -> bool {
        true
    }
    fn embed(
        &self,
        _: reliquary_owner_session::model::EmbeddingMode,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        Ok(vec![vec![1.0, 0.0]; inputs.len()])
    }
    fn embed_bounded(
        &self,
        mode: reliquary_owner_session::model::EmbeddingMode,
        inputs: &[String],
        limit: usize,
    ) -> Result<Vec<Vec<f32>>, crate::EmbeddingEndpointError> {
        assert!(inputs.len() * 8 <= limit);
        self.embed(mode, inputs)
    }
}

pub(super) fn add_required_perception(
    writer: &mut RelWriter,
    import_id: &str,
    source: MemoryId,
    plan: &mut NativeImportPostprocessingPlan,
) {
    let source = writer.read_session().current_memory(source).unwrap().record;
    let owner_id = writer.owner_id();
    let key = MemoryEntityMentionKey {
        memory_id: source.id.0,
        field: MemoryTextField::Title,
        start_byte: 0,
        end_byte: 8,
    };
    plan.entity_mentions.push(key);
    assert!(finish_native_import_postprocessing(writer, import_id, plan, 800).is_err());
    let command = EntityMentionCommit {
        owner_id,
        memory_revision: source.revision,
        body_id: source.body_id,
        key,
        surface: "Imported".into(),
        expected_revision: 0,
        updated_at_ns: 705,
        status: MemoryEntityResolutionStatus::Unresolved {
            reason: EntityResolutionReason::NamedReferent,
        },
        target_revisions: vec![],
        new_entity: Some(("tool".into(), "Imported migration tool".into())),
    };
    assert!(
        writer
            .commit_native_import_entity_mention("wrong-epoch", command.clone())
            .is_err()
    );
    let mention = writer
        .commit_native_import_entity_mention(import_id, command.clone())
        .unwrap();
    assert_eq!(
        writer
            .commit_native_import_entity_mention(import_id, command)
            .unwrap(),
        mention
    );
    let MemoryEntityResolutionStatus::Resolved { entity_id, .. } = mention.status else {
        panic!("expected resolved Entity mention");
    };
    let request = CreateRelationshipRequest {
        owner_id,
        kind: "describes".into(),
        participants: vec![RelationshipParticipantEvidence {
            entity_id,
            entity_revision: 1,
            role: Some("tool".into()),
        }],
        source_memory_id: source.id,
        source_memory_revision: source.revision,
        source_body_id: source.body_id,
        source_field: MemoryTextField::Title,
        start_byte: 0,
        end_byte: 15,
        source_quote: "Imported source".into(),
        mutation_id: "import-relationship-fixture".into(),
        updated_at_ns: 706,
    };
    let relationship =
        publish_native_import_relationship(writer, import_id, request.clone()).unwrap();
    assert_eq!(
        publish_native_import_relationship(writer, import_id, request).unwrap(),
        relationship
    );
    plan.relationships
        .push((relationship.id, relationship.revision));
}
