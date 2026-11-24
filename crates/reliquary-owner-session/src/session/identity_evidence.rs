use super::perception_lifecycle::support::{
    contains_normalized, has_direct_source, owner_hex, text_field,
};
use super::*;
use reliquary_memory_core::{MemoryBody, MemoryRecord, MemorySourceRef};
use reliquary_perception_store::{
    PRINCIPAL_ENTITY_KIND,
    identity::{
        IdentityLimit, IdentityMemoryRef, IdentitySpan, MAX_IDENTITY_MENTION_CONTEXT_WORDS,
        MAX_IDENTITY_MENTION_QUOTE_BYTES, MAX_IDENTITY_MENTIONS_PER_SIDE,
        MAX_IDENTITY_RELATIONSHIPS_PER_SIDE,
    },
    identity_evidence::{
        IdentityEntityEvidence, IdentityEvidence, IdentityEvidenceCut, IdentityMemoryExclusion,
        IdentityMentionEvidence, IdentityObservedMemory, IdentityParticipantEvidence,
        IdentityRelationshipEvidence,
    },
};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;

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
            self.common.perception.retained_resolution_history().len(),
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
        let endpoint = IdentityEntityEvidence::from(endpoint);
        let mentions = self.common.identity_mentions(&endpoint)?;
        let mut evidence = IdentityEvidence {
            owner: self.owner_id,
            endpoint,
            relationships: Vec::new(),
            mentions,
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

    fn identity_mentions(
        &self,
        entity: &IdentityEntityEvidence,
    ) -> Result<Vec<IdentityMentionEvidence>, OwnerSessionError> {
        let mut mentions = self
            .perception
            .resolved_mentions_for_entity(entity.id)
            .peekable();
        let mut selected: Vec<(usize, IdentityMentionEvidence)> = Vec::new();
        while let Some(first) = mentions.next() {
            let id = MemoryId(first.key.memory_id);
            let group = std::iter::once(first).chain(std::iter::from_fn(|| {
                mentions.next_if(|mention| mention.key.memory_id == id.0)
            }));
            let Some(source) = self
                .identity_memory(id)
                .filter(|record| has_direct_source(record))
                .and_then(|record| record.source_ref.as_ref())
            else {
                let _ = group.last();
                continue;
            };
            let (record, body) = self.memories.revision_projection(id, None)?;
            let Some(body) = body.as_deref() else {
                let _ = group.last();
                continue;
            };
            let lineage = source_lineage(source);
            let Some((score, mention)) = group
                .filter_map(|mention| {
                    let (span, quote) = identity_mention_excerpt(body, mention.key)?;
                    let witness = IdentityMentionEvidence {
                        key: mention.key,
                        revision: mention.revision,
                        memory: IdentityObservedMemory {
                            reference: IdentityMemoryRef {
                                id: record.id.0,
                                revision: record.revision,
                                body_id: record.body_id.0,
                                span: Some(span),
                            },
                            lineage: lineage.clone(),
                            exclusion: None,
                        },
                        quote: quote.to_owned(),
                        retained_ordinal: mention.retained_ordinal,
                    };
                    Some((witness.context_score(entity), witness))
                })
                .max_by_key(|(score, mention)| (*score, Reverse(mention.key)))
            else {
                continue;
            };
            let rank = (Reverse(score), mention.key);
            let overlaps = |other: &IdentityMentionEvidence| {
                other.memory.lineage.iter().any(|key| lineage.contains(key))
            };
            if selected
                .iter()
                .any(|(score, other)| (Reverse(*score), other.key) <= rank && overlaps(other))
            {
                continue;
            }
            selected.retain(|(_, other)| !overlaps(other));
            selected.push((score, mention));
            selected.sort_unstable_by_key(|(score, mention)| (Reverse(*score), mention.key));
            selected.truncate(MAX_IDENTITY_MENTIONS_PER_SIDE);
            // Later Memory IDs lose every tie. A full set at the maximum score is final.
            if selected.len() == MAX_IDENTITY_MENTIONS_PER_SIDE
                && selected
                    .last()
                    .is_some_and(|(score, _)| *score == MAX_IDENTITY_MENTION_CONTEXT_WORDS)
            {
                break;
            }
        }
        selected.sort_unstable_by_key(|(_, mention)| mention.key);
        Ok(selected.into_iter().map(|(_, mention)| mention).collect())
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

/// A bounded slice of the same field, anchored to the retained mention's exact bytes.
/// Replay validates this window for mention-bearing receipts. Preserve the
/// historical window when changing the selection algorithm.
pub(super) fn identity_mention_excerpt(
    body: &MemoryBody,
    key: MemoryEntityMentionKey,
) -> Option<(IdentitySpan, &str)> {
    let text = text_field(body, key.field);
    let mention_start = key.start_byte as usize;
    let mention_end = key.end_byte as usize;
    let surface = text.get(mention_start..mention_end)?;
    if surface.trim().is_empty() || surface.len() > MAX_IDENTITY_MENTION_QUOTE_BYTES {
        return None;
    }

    // Give both sides of the mention room, then spend any spare bytes on either side.
    // The window stays on the same line and rounds inward at UTF-8 boundaries.
    let prefix = &text[..mention_start];
    let suffix = &text[mention_end..];
    let budget = MAX_IDENTITY_MENTION_QUOTE_BYTES - surface.len();
    let before = prefix
        .as_bytes()
        .iter()
        .rev()
        .take(budget)
        .take_while(|&&byte| !matches!(byte, b'\r' | b'\n'))
        .count();
    let after = suffix
        .as_bytes()
        .iter()
        .take(budget)
        .take_while(|&&byte| !matches!(byte, b'\r' | b'\n'))
        .count();
    let left = before.min(budget / 2 + (budget - budget / 2).saturating_sub(after));
    let right = after.min(budget - left);
    let start = text.ceil_char_boundary(mention_start - left);
    let end = text.floor_char_boundary(mention_end + right);
    Some((
        IdentitySpan {
            field: key.field,
            start_byte: start.try_into().ok()?,
            end_byte: end.try_into().ok()?,
        },
        &text[start..end],
    ))
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
    fn mention_excerpt_is_bounded_utf8_context_at_the_exact_key() {
        let prefix = "🦀".repeat(180);
        let body = MemoryBody {
            title: "Élodie".into(),
            content: format!("{prefix} Élodie signed the Meridian Contract.\nAn unrelated note."),
        };
        let start = (prefix.len() + 1) as u32;
        let key = MemoryEntityMentionKey {
            memory_id: [3; 32],
            field: MemoryTextField::Content,
            start_byte: start,
            end_byte: start + "Élodie".len() as u32,
        };
        let (span, quote) = identity_mention_excerpt(&body, key).unwrap();
        assert_eq!(span.field, key.field);
        assert!(quote.ends_with(" Élodie signed the Meridian Contract."));
        assert_eq!(
            &quote[(key.start_byte - span.start_byte) as usize
                ..(key.end_byte - span.start_byte) as usize],
            "Élodie",
        );
        assert_eq!(
            &body.content[span.start_byte as usize..span.end_byte as usize],
            quote
        );
        assert!(quote.len() <= MAX_IDENTITY_MENTION_QUOTE_BYTES);
        assert!(
            identity_mention_excerpt(
                &body,
                MemoryEntityMentionKey {
                    start_byte: key.start_byte + 1,
                    ..key
                }
            )
            .is_none()
        );
        let title_key = MemoryEntityMentionKey {
            field: MemoryTextField::Title,
            start_byte: 0,
            end_byte: "Élodie".len() as u32,
            ..key
        };
        let (_, quote) = identity_mention_excerpt(&body, title_key).unwrap();
        assert_eq!(quote, "Élodie");
    }

    #[test]
    fn capture_prefers_distinct_context_and_drops_corrected_or_withdrawn_mentions() {
        use super::super::identity_receipt::tests::fixture_entity;
        use crate::UserMemoryCorrection;
        use reliquary_memory_core::MemoryDraft;

        let scratch = Path::new(env!("CARGO_MANIFEST_DIR")).join(".test-tmp");
        std::fs::create_dir_all(&scratch).unwrap();
        let directory = tempfile::tempdir_in(scratch).unwrap();
        let path = directory.path().join("mention-evidence.rel");
        let mut writer = RelWriter::create_current_path(&path, [21; 16]).unwrap();
        let endpoint = fixture_entity(&mut writer, "Mira");
        let owner = writer.owner_id();
        let publish = |writer: &mut RelWriter, token: &str, source_node: &str, text: &str| {
            writer
                .publish_direct_memory(
                    "principal-a",
                    MemoryDraft {
                        title: "Mira / MIRA | referent".into(),
                        content: text.into(),
                        category: "fact".into(),
                        memory_type: "project".into(),
                        authority_kind: "direct".into(),
                        temporal_status: "current".into(),
                        scope: "project".into(),
                        lifecycle_state: "current".into(),
                        archived: false,
                        parent_id: None,
                        superseded_by: None,
                        source_node_id: None,
                        content_source_conversation_id: None,
                        content_source_node_id: None,
                        grounding_source_conversation_id: None,
                        grounding_source_node_id: None,
                        source_episode_id: None,
                        source_ref: Some(MemorySourceRef {
                            owner_id: owner_hex(&owner),
                            principal_id: Some("principal-a".into()),
                            source_episode_id: [0; 32],
                            source_node_id: source_node.into(),
                            content_source_conversation_id: None,
                            content_source_node_id: None,
                            grounding_source_conversation_id: None,
                            grounding_source_node_id: None,
                        }),
                        source_time_ns: Some(10),
                        temporal_inference: None,
                        mutation_id: token.into(),
                        created_at_ns: 10,
                        updated_at_ns: 10,
                    },
                )
                .unwrap()
                .record
        };
        let resolve = |writer: &mut RelWriter, memory: &MemoryRecord, field, surface: &str| {
            let key = MemoryEntityMentionKey {
                memory_id: memory.id.0,
                field,
                start_byte: 0,
                end_byte: surface.len() as u32,
            };
            writer
                .commit_entity_mention(EntityMentionCommit {
                    owner_id: owner,
                    memory_revision: memory.revision,
                    body_id: memory.body_id,
                    key,
                    surface: surface.into(),
                    expected_revision: 0,
                    updated_at_ns: 11,
                    status: MemoryEntityResolutionStatus::Resolved {
                        entity_id: endpoint.id,
                        reason: EntityResolutionReason::ContextMatch,
                    },
                    target_revisions: vec![(endpoint.id, 1)],
                    new_entity: None,
                })
                .unwrap();
            key
        };
        for number in 0..6 {
            let token = format!("name-only-{number}");
            let memory = publish(&mut writer, &token, &token, "Other material.");
            resolve(&mut writer, &memory, MemoryTextField::Title, "Mira");
        }
        let first = publish(
            &mut writer,
            "independent-primary",
            "primary-turn",
            "Lantern Keeper signed the Meridian Contract.",
        );
        resolve(&mut writer, &first, MemoryTextField::Title, "Mira");
        let first_key = resolve(
            &mut writer,
            &first,
            MemoryTextField::Content,
            "Lantern Keeper",
        );
        let copy = publish(
            &mut writer,
            "primary-copy",
            "primary-turn",
            "Mira signed it.",
        );
        resolve(&mut writer, &copy, MemoryTextField::Content, "Mira");
        let second = publish(
            &mut writer,
            "independent-secondary",
            "secondary-turn",
            "Mira led the Meridian expedition.",
        );
        let second_key = resolve(&mut writer, &second, MemoryTextField::Content, "Mira");

        let capture = |writer: &RelWriter| {
            writer
                .session
                .identity_evidence(endpoint.id)
                .unwrap()
                .unwrap()
        };
        let before = capture(&writer);
        assert_eq!(before.mentions.len(), MAX_IDENTITY_MENTIONS_PER_SIDE);
        assert_eq!(
            before
                .mentions
                .iter()
                .filter(|mention| mention.context_score(&before.endpoint) > 0)
                .count(),
            2
        );
        assert!(
            before
                .mentions
                .iter()
                .any(|mention| mention.key == first_key)
        );
        assert!(
            before
                .mentions
                .iter()
                .any(|mention| mention.key == second_key)
        );
        assert!(
            !before
                .mentions
                .iter()
                .any(|mention| mention.key.memory_id == copy.id.0)
        );

        writer
            .withdraw_entity_mention(WithdrawEntityMentionRequest {
                owner_id: owner,
                memory_id: second.id,
                memory_revision: second.revision,
                body_id: second.body_id,
                key: second_key,
                source_surface: "Mira".into(),
                expected_mention_revision: 1,
                target_revisions: vec![(endpoint.id, 1)],
                withdrawn_at_ns: 12,
            })
            .unwrap();
        assert!(
            !capture(&writer)
                .mentions
                .iter()
                .any(|mention| mention.key == second_key)
        );

        writer
            .correct_user_memory(
                "principal-a",
                format!("management:{}", "5".repeat(64)),
                [5; 32],
                UserMemoryCorrection {
                    id: first.id,
                    revision: first.revision,
                    body: first.body_id,
                    title: "Mira".into(),
                    content: "Mira signed the Beacon Contract.".into(),
                    created_at_ns: 13,
                },
            )
            .unwrap();
        assert!(
            !capture(&writer)
                .mentions
                .iter()
                .any(|mention| mention.key.memory_id == first.id.0)
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
