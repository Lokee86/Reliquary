//! Owner-derived retrieval inputs. Every lane uses the same immutable read cut.
use crate::{AgenticMemory, OwnerMemorySelection, SelectedMemory};
use reliquary_owner_session::{RelRetrievalSnapshot, model::PayloadState};
use reliquary_retrieval::memory_retrieval::{
    MemoryRetrievalConfig, MemoryRetrievalResult, MemoryRetrievalSnapshot, retrieve_memories,
};
use reliquary_retrieval::{
    RetrievalConfig, combine_hybrid_scores, lexical_score, lexical_terms, validate_hybrid_request,
};

pub fn agentic_memories(snapshot: &RelRetrievalSnapshot) -> Vec<AgenticMemory> {
    snapshot
        .memories
        .iter()
        .filter_map(|memory| {
            let revision = &memory.revision;
            let PayloadState::Available(body) = &revision.payload else {
                return None;
            };
            Some(AgenticMemory {
                id: revision.record.id,
                revision: revision.record.revision,
                body_id: revision.record.body_id,
                body: body.clone(),
                freshness: memory.freshness.as_ref().map_or(0, |record| record.score),
                vector: snapshot.vectors.get(&revision.record.body_id).cloned(),
            })
        })
        .collect()
}

/// Hybrid rank over the complete owner-local population, before bounded selection.
pub fn rank_owner_snapshot(
    snapshot: &RelRetrievalSnapshot,
    query: &str,
    vector: Option<&[f32]>,
    config: RetrievalConfig,
) -> Result<OwnerMemorySelection, String> {
    let weights = validate_hybrid_request(query, config).map_err(|e| format!("{e:?}"))?;
    if vector.is_some_and(|v| {
        v.is_empty() || v.iter().any(|x| !x.is_finite()) || v.iter().all(|x| *x == 0.0)
    }) {
        return Err("invalid query vector".into());
    }
    let terms = lexical_terms(query);
    let mut selected = Vec::new();
    for memory in agentic_memories(snapshot) {
        let lexical = lexical_score(
            &format!("{}\n{}", memory.body.title, memory.body.content),
            &terms,
        );
        let semantic = match (vector, memory.vector.as_deref()) {
            (Some(a), Some(b)) => crate::agentic_retrieval::cosine(a, b)?,
            _ => 0.0,
        };
        if lexical <= 0.0 && semantic <= 0.0 {
            continue;
        }
        selected.push(SelectedMemory {
            memory_id: memory.id,
            revision: memory.revision,
            body_id: memory.body_id,
            body: memory.body,
            lexical_score: (lexical > 0.0).then_some(lexical),
            semantic_score: (semantic > 0.0).then_some(semantic),
            combined_score: combine_hybrid_scores(lexical.max(0.0), semantic.max(0.0), weights),
        });
    }
    selected.sort_by(|a, b| {
        b.combined_score
            .total_cmp(&a.combined_score)
            .then_with(|| a.memory_id.cmp(&b.memory_id))
    });
    selected.truncate(config.candidate_limit);
    selected.truncate(config.result_limit);
    Ok(OwnerMemorySelection {
        owner_id: snapshot.owner_id,
        selected,
    })
}

pub fn route_owner_snapshot(
    snapshot: &RelRetrievalSnapshot,
    query: &[f32],
    config: MemoryRetrievalConfig,
) -> Result<MemoryRetrievalResult, String> {
    let memories = agentic_memories(snapshot);
    let input = MemoryRetrievalSnapshot {
        graph_version: snapshot.graph_version,
        community_snapshot: snapshot.community.clone(),
        searchable: memories.iter().map(|m| m.id).collect(),
        vectors: memories
            .into_iter()
            .filter_map(|m| m.vector.map(|v| (m.id, v)))
            .collect(),
    };
    retrieve_memories(&input, query, config, snapshot.graph_version).map_err(|e| format!("{e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reliquary_owner_session::{
        RelRetrievalMemory,
        model::{MemoryBody, MemoryBodyId, MemoryId, MemoryRecord, MemoryRevision},
    };

    fn snapshot() -> RelRetrievalSnapshot {
        let memories = (1..=35)
            .map(|id| RelRetrievalMemory {
                revision: MemoryRevision {
                    record: MemoryRecord {
                        id: MemoryId([id; 32]),
                        body_id: MemoryBodyId([id; 32]),
                        revision: 2,
                        category: "fact".into(),
                        memory_type: "semantic".into(),
                        authority_kind: "user".into(),
                        temporal_status: "current".into(),
                        scope: "project".into(),
                        lifecycle_state: "accepted".into(),
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
                        source_time_ns: None,
                        temporal_inference: None,
                        mutation_id: format!("fixture-{id}"),
                        created_at_ns: 1,
                        updated_at_ns: 2,
                        memory_version: 35,
                    },
                    payload: PayloadState::Available(MemoryBody {
                        title: "needle".into(),
                        content: format!("source body {id}"),
                    }),
                },
                freshness: None,
            })
            .collect();
        RelRetrievalSnapshot {
            owner_id: [7; 16],
            rel_turn: 4,
            memory_version: 35,
            graph_version: 0,
            memories,
            active_relations: vec![],
            community: None,
            vectors: (1..=35)
                .map(|id| {
                    (
                        MemoryBodyId([id; 32]),
                        if id == 35 {
                            vec![1.0, 0.0]
                        } else {
                            vec![0.0, 1.0]
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn hybrid_ranks_full_owner_cut_and_preserves_exact_body_and_ties() {
        let mut facts = snapshot();
        let result = rank_owner_snapshot(
            &facts,
            "needle",
            Some(&[1.0, 0.0]),
            RetrievalConfig::default(),
        )
        .unwrap();
        assert_eq!(result.selected.len(), 10);
        assert_eq!(result.selected[0].memory_id, MemoryId([35; 32]));
        assert_eq!(result.selected[0].body.content, "source body 35");
        assert_eq!(result.selected[0].revision, 2);
        assert_eq!(result.selected[1].memory_id, MemoryId([1; 32]));
        facts.memories.reverse();
        assert_eq!(
            result,
            rank_owner_snapshot(
                &facts,
                "needle",
                Some(&[1.0, 0.0]),
                RetrievalConfig::default()
            )
            .unwrap()
        );
        assert_eq!(agentic_memories(&facts)[0].freshness, 0);
        assert!(
            rank_owner_snapshot(
                &facts,
                "needle",
                Some(&[0.0, 0.0]),
                RetrievalConfig::default()
            )
            .is_err()
        );
        let routed =
            route_owner_snapshot(&facts, &[1.0, 0.0], MemoryRetrievalConfig::default()).unwrap();
        assert!(routed.used_global_fallback);
        assert_eq!(routed.memories, vec![MemoryId([35; 32])]);
    }
}
