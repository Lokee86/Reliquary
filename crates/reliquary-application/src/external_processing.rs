use crate::{HydratedExternalMessage, verify_external_hydration};
use reliquary_insomnia::{InsomniaCandidate, InsomniaModelExtractor};
use reliquary_owner_session::{
    model::{Episode, EpisodeBoundary, EpisodeId, EpisodeOrigin, ResolvedTurn},
    records::external_index::{ExternalChunkRef, ExternalMessageDescriptor, ExternalSourceError},
};
use reliquary_switchboard::GeneralEndpoint;
use sha2::{Digest, Sha256};

/// A proposal with host-qualified provenance. The window never enters Archive.
pub struct ExternalMemoryProposal {
    pub candidate: InsomniaCandidate,
    pub authority: ExternalChunkRef,
    pub content: Option<ExternalChunkRef>,
    pub grounding: Option<ExternalChunkRef>,
    pub source_time_ns: i64,
}

pub fn prepare_external_memory<E: GeneralEndpoint>(
    endpoint: E,
    namespace: &str,
    expected: &[ExternalMessageDescriptor],
    hydrated: &[HydratedExternalMessage],
) -> Result<Vec<ExternalMemoryProposal>, ExternalSourceError> {
    verify_external_hydration(namespace, expected, hydrated)?;
    let first = &expected[0].reference;
    if hydrated
        .iter()
        .map(|message| message.content.len())
        .sum::<usize>()
        > 256 * 1024
    {
        return Err(ExternalSourceError::Bounds);
    }
    if expected
        .iter()
        .any(|item| item.reference.conversation_id != first.conversation_id)
        || expected
            .windows(2)
            .any(|pair| pair[0].timestamp_ns > pair[1].timestamp_ns)
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    let window_id = external_window_id(expected)?;
    let last = expected.last().ok_or(ExternalSourceError::Bounds)?;
    let window = Episode {
        id: EpisodeId(window_id),
        conversation_id: first.conversation_id.clone(),
        start_node_id: first.message_id.clone(),
        end_node_id: last.reference.message_id.clone(),
        origin: EpisodeOrigin::Live,
        boundary: EpisodeBoundary::Explicit,
        source_through_ns: last.timestamp_ns,
        finalized_at_ns: last.timestamp_ns,
    };
    let turns: Vec<_> = hydrated
        .iter()
        .map(|message| ResolvedTurn {
            node_id: message.descriptor.reference.message_id.clone(),
            role: message.descriptor.role.clone(),
            principal_id: message.descriptor.principal_id.clone(),
            timestamp_ns: message.descriptor.timestamp_ns,
            content: message.content.clone(),
        })
        .collect();
    let extractor = InsomniaModelExtractor::new(endpoint);
    let stage = extractor
        .start(&window, &turns)
        .map_err(|_| ExternalSourceError::Unavailable)?;
    if !stage.requests.is_empty() {
        return Err(ExternalSourceError::InvalidContract);
    }
    let extraction = extractor
        .finish_without_evidence(&window, &turns, stage)
        .map_err(|_| ExternalSourceError::Unavailable)?;
    if !extraction.rejected.is_empty() {
        return Err(ExternalSourceError::InvalidContract);
    }
    extraction
        .candidates
        .into_iter()
        .map(|candidate| {
            let (authority, user_time) =
                bind_quote(hydrated, &candidate.source_node_id, &candidate.source_quote)?;
            let content = candidate
                .authority_source
                .as_ref()
                .map(|source| bind_quote(hydrated, &source.node_id, &source.quote))
                .transpose()?;
            let grounding = candidate
                .grounding_source
                .as_ref()
                .map(|source| {
                    bind_quote(hydrated, &source.node_id, &source.quote)
                        .map(|(reference, _)| reference)
                })
                .transpose()?;
            let source_time_ns = content.as_ref().map_or(user_time, |(_, time)| *time);
            Ok(ExternalMemoryProposal {
                candidate,
                authority,
                content: content.map(|(reference, _)| reference),
                grounding,
                source_time_ns,
            })
        })
        .collect()
}

/// Stable identity of an ordered descriptor window, including content hashes.
pub fn external_window_id(
    expected: &[ExternalMessageDescriptor],
) -> Result<[u8; 32], ExternalSourceError> {
    let mut hash = Sha256::new();
    hash.update(b"reliquary-external-insomnia-window-v1");
    for descriptor in expected {
        let encoded =
            serde_json::to_vec(descriptor).map_err(|_| ExternalSourceError::InvalidContract)?;
        hash.update((encoded.len() as u64).to_le_bytes());
        hash.update(encoded);
    }
    Ok(hash.finalize().into())
}

impl ExternalMemoryProposal {
    /// Prepare a REL Memory draft. Host provenance is carried by the external
    /// publication record, independently of native Archive source fields.
    pub fn into_project_draft(
        self,
        principal: &str,
        window_id: [u8; 32],
        ordinal: usize,
    ) -> Result<
        (
            reliquary_owner_session::model::MemoryDraft,
            ExternalChunkRef,
            Option<ExternalChunkRef>,
            Option<ExternalChunkRef>,
        ),
        ExternalSourceError,
    > {
        if self.candidate.ownership != reliquary_insomnia::InsomniaOwnership::Project {
            return Err(ExternalSourceError::Unauthorized);
        }
        let mut hash = Sha256::new();
        hash.update(b"reliquary-external-memory-v1");
        hash.update((principal.len() as u64).to_le_bytes());
        hash.update(principal.as_bytes());
        hash.update(window_id);
        hash.update((ordinal as u64).to_le_bytes());
        let key = hash
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let draft = reliquary_owner_session::model::MemoryDraft {
            title: self.candidate.title,
            content: self.candidate.content,
            category: self.candidate.category,
            memory_type: self.candidate.memory_type,
            authority_kind: self.candidate.authority_kind,
            temporal_status: self.candidate.temporal_status,
            scope: "project".into(),
            lifecycle_state: "extracted".into(),
            archived: false,
            parent_id: None,
            superseded_by: None,
            source_node_id: None,
            content_source_conversation_id: None,
            content_source_node_id: None,
            grounding_source_conversation_id: None,
            grounding_source_node_id: None,
            source_episode_id: None,
            source_ref: None,
            source_time_ns: Some(self.source_time_ns),
            temporal_inference: None,
            mutation_id: format!("external-insomnia:{key}"),
            created_at_ns: self.source_time_ns,
            updated_at_ns: self.source_time_ns,
        };
        Ok((draft, self.authority, self.content, self.grounding))
    }
}

fn bind_quote(
    hydrated: &[HydratedExternalMessage],
    node_id: &str,
    quote: &str,
) -> Result<(ExternalChunkRef, i64), ExternalSourceError> {
    let message = hydrated
        .iter()
        .find(|message| message.descriptor.reference.message_id == node_id)
        .ok_or(ExternalSourceError::StaleEvidence)?;
    let start = message
        .content
        .find(quote)
        .ok_or(ExternalSourceError::StaleEvidence)?;
    if quote.is_empty() || message.content.rfind(quote) != Some(start) {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok((
        ExternalChunkRef {
            message: message.descriptor.reference.clone(),
            start: start as u64,
            end: (start + quote.len()) as u64,
        },
        message.descriptor.timestamp_ns,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_owner_session::records::external_index::ExternalMessageRef;
    use reliquary_switchboard::GeneralEndpointError;
    use serde_json::{Value, json};

    struct Fixture;
    impl GeneralEndpoint for Fixture {
        fn model(&self) -> &str {
            "external-fixture"
        }

        fn complete_json(
            &self,
            _: &str,
            _: &str,
            schema: &str,
            _: &Value,
        ) -> Result<Value, GeneralEndpointError> {
            Ok(match schema {
                "insomnia_authority_disposition_ledger" => json!({
                    "turns":{"u1":[{"disposition":"retain","authority_kind":"direct","category":"preference",
                    "type":"communication","lifecycle":"current","proposition":"The user prefers concise answers.",
                    "authority_source_node_id":"","grounding_source_node_id":"","reason":"direct assertion"}]},
                    "evidence_requests":[]
                }),
                "insomnia_memory_wording" => json!({
                    "candidates":[{"authority_kind":"direct","category":"preference","memory_type":"communication",
                    "temporal_status":"current","ownership":"project","title":"Concise answers",
                    "content":"The user prefers concise answers.","source_node_id":"u1",
                    "source_quote":"I prefer concise answers.","authority_source":null,
                    "grounding_source":null,"routing_mentions":[]}]
                }),
                other => panic!("unexpected model call {other}"),
            })
        }
    }

    #[test]
    fn external_memory_binds_model_quote_to_unique_host_byte_range() {
        let text = "🦀 I prefer concise answers.";
        let descriptor = ExternalMessageDescriptor {
            reference: ExternalMessageRef {
                source_namespace: "host".into(),
                conversation_id: "thread".into(),
                message_id: "u1".into(),
                content_hash: Sha256::digest(text.as_bytes()).into(),
            },
            role: "user".into(),
            principal_id: Some("alice".into()),
            timestamp_ns: 17,
            active: true,
        };
        let message = HydratedExternalMessage {
            descriptor: descriptor.clone(),
            content: text.into(),
        };
        let proposals =
            prepare_external_memory(Fixture, "host", &[descriptor.clone()], &[message.clone()])
                .unwrap();
        assert_eq!(proposals.len(), 1);
        let proposal = &proposals[0];
        assert_eq!(proposal.authority.message, descriptor.reference);
        assert_eq!(proposal.authority.start, 5);
        assert_eq!(proposal.authority.end as usize, text.len());
        assert_eq!(proposal.source_time_ns, 17);
        let window = external_window_id(std::slice::from_ref(&descriptor)).unwrap();
        let (draft, reference, content, grounding) = proposals
            .into_iter()
            .next()
            .unwrap()
            .into_project_draft("alice", window, 0)
            .unwrap();
        assert_eq!(draft.source_time_ns, Some(17));
        assert!(
            draft.source_ref.is_none()
                && draft.source_episode_id.is_none()
                && draft.source_node_id.is_none()
        );
        assert_eq!(reference.message, descriptor.reference);
        assert!(content.is_none() && grounding.is_none());
        let mut repeated = message.clone();
        repeated.content.push_str(" I prefer concise answers.");
        repeated.descriptor.reference.content_hash =
            Sha256::digest(repeated.content.as_bytes()).into();
        assert!(matches!(
            prepare_external_memory(Fixture, "host", &[repeated.descriptor.clone()], &[repeated]),
            Err(ExternalSourceError::InvalidContract)
        ));
        let mut changed = message;
        changed.content.push('!');
        assert!(matches!(
            prepare_external_memory(Fixture, "host", &[descriptor], &[changed]),
            Err(ExternalSourceError::StaleEvidence)
        ));
    }
}
