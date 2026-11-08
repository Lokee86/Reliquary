use crate::{MAX_INGEST_EVIDENCE_BYTES, MAX_INGEST_EVIDENCE_TURNS};
use reliquary_insomnia::{EvidenceRequest, EvidenceResult, EvidenceTurn};
use reliquary_owner_session::{RelInsomniaClaimInput, RelSession, model::EpisodeOrigin};

pub struct ResolvedNativeInsomniaEvidence {
    pub(crate) results: Vec<EvidenceResult>,
    pub(crate) turns: Vec<EvidenceTurn>,
}

/// Resolve only the single bounded round against the authenticated REL mount.
pub fn resolve_native_insomnia_evidence(
    rel: &RelSession,
    claim: &RelInsomniaClaimInput,
    requests: &[EvidenceRequest],
) -> Result<ResolvedNativeInsomniaEvidence, String> {
    resolve_scoped_evidence(rel, claim, requests, None)
}

pub fn resolve_import_insomnia_evidence(
    rel: &RelSession,
    import_id: &str,
    claim: &RelInsomniaClaimInput,
    requests: &[EvidenceRequest],
) -> Result<ResolvedNativeInsomniaEvidence, String> {
    resolve_scoped_evidence(rel, claim, requests, Some(import_id))
}

fn resolve_scoped_evidence(
    rel: &RelSession,
    claim: &RelInsomniaClaimInput,
    requests: &[EvidenceRequest],
    import_id: Option<&str>,
) -> Result<ResolvedNativeInsomniaEvidence, String> {
    if rel.owner_id() != claim.owner_id
        || rel.episode(claim.episode.id) != Some(&claim.episode)
        || !matches!(
            (claim.episode.origin, import_id),
            (EpisodeOrigin::Live, None) | (EpisodeOrigin::Import, Some(_))
        )
        || requests.len() > reliquary_insomnia::MAX_INSOMNIA_EVIDENCE_REQUESTS
    {
        return Err("evidence requires the claimed native REL Episode".into());
    }

    let through = claim.episode.source_through_ns;
    let mut results = Vec::with_capacity(requests.len());
    let mut turns: Vec<EvidenceTurn> = Vec::new();
    let mut bytes = 0;
    for request in requests {
        let conversation = request.conversation_id.trim();
        let mut truncated = false;
        let found = match request.kind.as_str() {
            "turn" => rel
                .archive_evidence_turn(conversation, request.node_id.trim())
                .map(|turn| {
                    turn.into_iter()
                        .map(|turn| (conversation.to_owned(), turn))
                        .collect()
                }),
            "conversation_range" => rel
                .archive_evidence_range(
                    conversation,
                    request.start_node_id.trim(),
                    request.end_node_id.trim(),
                )
                .map(|turns| {
                    turns
                        .into_iter()
                        .map(|turn| (conversation.to_owned(), turn))
                        .collect()
                }),
            "archive_search" => rel
                .archive_evidence_search(
                    Some(conversation).filter(|id| !id.is_empty()),
                    request.query.trim(),
                    if request.limit == 0 { 5 } else { request.limit },
                    Some(through),
                )
                .map(|search| {
                    truncated = search.truncated;
                    search.turns
                }),
            _ => return Err("unsupported Insomnia evidence request".into()),
        };
        let mut result = EvidenceResult {
            kind: request.kind.clone(),
            turns: Vec::new(),
            error: None,
            truncated,
        };
        match found {
            Err(_) => result.error = Some("requested Archive evidence is unavailable".into()),
            Ok(found) => {
                if let Some(id) = import_id
                    && request.kind != "archive_search"
                    && found.iter().any(|(conversation, turn)| {
                        !rel.native_import_source_contains(id, conversation, &turn.node_id)
                    })
                {
                    result.error = Some("requested Archive evidence is unavailable".into());
                    results.push(result);
                    continue;
                }
                for (conversation_id, turn) in found {
                    if import_id.is_some_and(|id| {
                        !rel.native_import_source_contains(id, &conversation_id, &turn.node_id)
                    }) {
                        result.truncated = true;
                        continue;
                    }
                    if turn.timestamp_ns > through {
                        result.truncated = true;
                        continue;
                    }
                    let fact = EvidenceTurn {
                        conversation_id,
                        node_id: turn.node_id,
                        role: turn.role,
                        timestamp_ns: turn.timestamp_ns,
                        content: turn.content,
                    };
                    if !turns.contains(&fact) {
                        if turns.len() == MAX_INGEST_EVIDENCE_TURNS
                            || bytes + fact.content.len() > MAX_INGEST_EVIDENCE_BYTES
                        {
                            result.truncated = true;
                            break;
                        }
                        bytes += fact.content.len();
                        turns.push(fact.clone());
                    }
                    result.turns.push(fact);
                }
            }
        }
        results.push(result);
    }
    Ok(ResolvedNativeInsomniaEvidence { results, turns })
}
