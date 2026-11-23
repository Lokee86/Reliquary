use super::perception_lifecycle::support::{
    contains_normalized, has_direct_source, owner_hex, text_field,
};
use super::*;
use reliquary_memory_core::{MemoryBody, MemoryRecord, MemorySourceRef};
use reliquary_perception_store::{
    PRINCIPAL_ENTITY_KIND,
    identity::{
        IdentityLimit, IdentityMemoryRef, IdentitySpan, MAX_IDENTITY_RELATIONSHIPS_PER_SIDE,
    },
    identity_evidence::{
        IdentityEvidence, IdentityEvidenceCut, IdentityMemoryExclusion, IdentityObservedMemory,
        IdentityParticipantEvidence, IdentityRelationshipEvidence,
    },
};
use sha2::{Digest, Sha256};

impl RelSession {
    /// A change hint for mounted comparisons. A changed cut requires recapture;
    /// the pair fingerprint determines whether its decision input has changed.
    pub fn identity_evidence_cut(&self) -> IdentityEvidenceCut {
        (
            self.common.perception.version(),
            self.common.memories.memory_version(),
            self.common
                .perception
                .relationship_description_prunes()
                .len(),
            self.common.memories.pruned_body_count(),
        )
    }

    /// Capture a bounded owner-local basis for comparing one active Entity.
    pub fn identity_evidence(
        &self,
        id: EntityId,
    ) -> Result<Option<IdentityEvidence>, OwnerSessionError> {
        let Some(endpoint) = self.common.identity_entity(id) else {
            return Ok(None);
        };
        let owner = owner_hex(&self.owner_id);
        let mut evidence = IdentityEvidence {
            owner: self.owner_id,
            endpoint: endpoint.into(),
            relationships: Vec::new(),
            limit: None,
        };
        for relationship in self
            .common
            .perception
            .relationships_for_local_participant(id)
        {
            if relationship.evidence.iter().any(|r| r.owner_id != owner) {
                continue;
            }
            let Some(participants) = self.common.identity_participants(relationship, &owner) else {
                continue;
            };
            let (active, inactive): (Vec<_>, Vec<_>) = relationship
                .evidence
                .iter()
                .map(|e| e.memory_id)
                .partition(|id| self.common.identity_memory(MemoryId(*id)).is_some());
            if active.is_empty() {
                continue;
            }
            if evidence.relationships.len() == MAX_IDENTITY_RELATIONSHIPS_PER_SIDE {
                evidence.limit = Some(IdentityLimit::Relationships);
                break;
            }

            let names_grounded = participants.iter().all(|participant| {
                quote_names_entity(
                    &relationship.summary,
                    &participant.entity.canonical_name,
                    &participant.entity.aliases,
                )
            });
            let mut memories = Vec::with_capacity(active.len());
            for id in active {
                memories.push(self.common.observe_identity_memory(
                    MemoryId(id),
                    &relationship.summary,
                    names_grounded,
                )?);
            }
            evidence.relationships.push(IdentityRelationshipEvidence {
                id: relationship.id,
                revision: relationship.revision,
                kind: relationship.kind.clone(),
                participants,
                quote: memories
                    .iter()
                    .any(|m| m.reference.span.is_some())
                    .then(|| relationship.summary.clone()),
                memories,
                inactive_memories: inactive,
                quote_digest: Sha256::digest(relationship.summary.as_bytes()).into(),
            });
        }
        Ok(Some(evidence))
    }
}

impl CommonState {
    fn identity_entity(&self, id: EntityId) -> Option<&Entity> {
        self.perception
            .entity(id)
            .filter(|entity| entity.kind != PRINCIPAL_ENTITY_KIND)
    }

    fn identity_participants(
        &self,
        relationship: &Relationship,
        owner: &str,
    ) -> Option<Vec<IdentityParticipantEvidence>> {
        if relationship
            .participants
            .iter()
            .any(|p| p.entity.owner_id != owner)
        {
            return None;
        }
        let mut participants: Vec<IdentityParticipantEvidence> = Vec::new();
        for participant in &relationship.participants {
            let id = participant.entity.entity_id;
            if let Some(previous) = participants.last_mut().filter(|p| p.entity.id == id) {
                previous.roles.extend(participant.role.iter().cloned());
            } else {
                participants.push(IdentityParticipantEvidence {
                    entity: self.identity_entity(id)?.into(),
                    roles: participant.role.iter().cloned().collect(),
                });
            }
        }
        (participants.len() >= 2).then_some(participants)
    }

    fn identity_memory(&self, id: MemoryId) -> Option<&MemoryRecord> {
        self.memories
            .record_at_cut(id, self.memories.memory_version())
            .filter(|record| !record.archived && record.superseded_by.is_none())
    }

    fn observe_identity_memory(
        &self,
        id: MemoryId,
        quote: &str,
        names_grounded: bool,
    ) -> Result<IdentityObservedMemory, OwnerSessionError> {
        let record = self.memories.revision_record(id, None)?;
        let mut lineage = Vec::new();
        let grounding = match record
            .source_ref
            .as_ref()
            .filter(|_| has_direct_source(record))
        {
            Some(source) => {
                lineage = source_lineage(source);
                if quote.trim().is_empty() {
                    Err(IdentityMemoryExclusion::QuoteAbsent)
                } else if !names_grounded {
                    Err(IdentityMemoryExclusion::ParticipantNames)
                } else {
                    let (_, body) = self.memories.revision_projection(id, None)?;
                    match body {
                        Some(body) => exact_quote_span(&body, quote),
                        None => Err(IdentityMemoryExclusion::BodyAbsent),
                    }
                }
            }
            None => Err(IdentityMemoryExclusion::SourceNotDirect),
        };
        Ok(IdentityObservedMemory {
            reference: IdentityMemoryRef {
                id: record.id.0,
                revision: record.revision,
                body_id: record.body_id.0,
                span: grounding.ok(),
            },
            lineage,
            exclusion: grounding.err(),
        })
    }
}

pub(super) fn quote_names_entity(quote: &str, canonical_name: &str, aliases: &[String]) -> bool {
    std::iter::once(canonical_name)
        .chain(aliases.iter().map(String::as_str))
        .any(|name| contains_normalized(quote, name))
}

pub(super) fn exact_quote_span(
    body: &MemoryBody,
    quote: &str,
) -> Result<IdentitySpan, IdentityMemoryExclusion> {
    if quote.trim().is_empty() {
        return Err(IdentityMemoryExclusion::QuoteAbsent);
    }
    let mut found = None;
    for field in [MemoryTextField::Title, MemoryTextField::Content] {
        let text = text_field(body, field);
        if let Some(start) = text.find(quote) {
            let end = start + quote.len();
            let next_char = start + text[start..].chars().next().map_or(0, char::len_utf8);
            // Start after the first character so overlapping matches count too.
            if found.is_some() || text[next_char..].contains(quote) {
                return Err(IdentityMemoryExclusion::QuoteRepeated);
            }
            let start_byte =
                u32::try_from(start).map_err(|_| IdentityMemoryExclusion::QuoteAbsent)?;
            let end_byte = u32::try_from(end).map_err(|_| IdentityMemoryExclusion::QuoteAbsent)?;
            found = Some(IdentitySpan {
                field,
                start_byte,
                end_byte,
            });
        }
    }
    found.ok_or(IdentityMemoryExclusion::QuoteAbsent)
}

fn source_lineage(source: &MemorySourceRef) -> Vec<[u8; 32]> {
    let digest = |kind: &[u8], parts: &[&[u8]]| {
        let mut hash = Sha256::new();
        hash.update(b"reliquary-entity-identity-source-v1\0");
        hash.update(kind);
        hash.update([0]);
        for bytes in parts {
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hash.finalize().into()
    };
    let owner = source.owner_id.as_bytes();
    let node = source.source_node_id.as_bytes();
    let mut keys = vec![digest(b"local-node", &[owner, node])];
    if source.source_episode_id != [0; 32] {
        keys.push(digest(
            b"local-episode",
            &[owner, &source.source_episode_id],
        ));
    }
    for (conversation, node) in [
        (
            &source.content_source_conversation_id,
            &source.content_source_node_id,
        ),
        (
            &source.grounding_source_conversation_id,
            &source.grounding_source_node_id,
        ),
    ] {
        if let (Some(conversation), Some(node)) = (conversation, node)
            && !conversation.trim().is_empty()
            && !node.trim().is_empty()
        {
            keys.push(digest(
                b"original-turn",
                &[conversation.as_bytes(), node.as_bytes()],
            ));
        }
    }
    keys.sort_unstable();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_current_quote_rejects_repetition_and_reports_utf8_bytes() {
        let mut body = MemoryBody {
            title: "Grounding".into(),
            content: "🦀 Élodie signed the Meridian Contract.".into(),
        };
        let quote = "Élodie signed the Meridian Contract.";
        let span = exact_quote_span(&body, quote).unwrap();
        assert_eq!(span.field, MemoryTextField::Content);
        assert_eq!(span.start_byte as usize, "🦀 ".len());
        assert_eq!(span.end_byte as usize, body.content.len());
        body.title = quote.into();
        assert_eq!(
            exact_quote_span(&body, quote),
            Err(IdentityMemoryExclusion::QuoteRepeated)
        );
        body.content = "ééé".into();
        assert_eq!(
            exact_quote_span(&body, "éé"),
            Err(IdentityMemoryExclusion::QuoteRepeated)
        );
        assert_eq!(
            exact_quote_span(&body, "Mira"),
            Err(IdentityMemoryExclusion::QuoteAbsent)
        );
    }

    #[test]
    fn source_lineage_survives_republication_in_another_owner() {
        let source = MemorySourceRef {
            owner_id: "recording owner".into(),
            principal_id: None,
            source_episode_id: [1; 32],
            source_node_id: "copy-1".into(),
            content_source_conversation_id: Some("original conversation".into()),
            content_source_node_id: Some("original turn".into()),
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
        };
        let mut copy = source.clone();
        copy.owner_id = "another owner".into();
        copy.source_node_id = "copy-2".into();
        assert!(
            source_lineage(&source)
                .iter()
                .any(|id| source_lineage(&copy).contains(id))
        );
        copy.content_source_node_id = Some("an independent turn".into());
        assert!(
            source_lineage(&source)
                .iter()
                .all(|id| !source_lineage(&copy).contains(id))
        );
    }
}
