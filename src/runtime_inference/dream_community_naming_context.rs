use crate::{
    COMMUNITY_NAMING_CONTRACT_VERSION, Community, DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES,
    DreamCommunityNamingError, MAX_COMMUNITY_SEMANTIC_NAME_BYTES, Memory, MemoryId,
    MemoryRetrievalIndex,
};
use reliquary_vectors::compatibility_vector::cosine;
use serde_json::{Value, json};
use std::collections::HashSet;

const MAX_REPRESENTATIVE_TEXT_BYTES: usize = 2_048;
const REPRESENTATIVES_PER_SUBCENTROID: usize = 2;

pub(crate) fn build_payload(memories: &[Memory]) -> Result<String, DreamCommunityNamingError> {
    serde_json::to_string(&json!({
        "contract_version": COMMUNITY_NAMING_CONTRACT_VERSION,
        "representative_memories": memories
            .iter()
            .enumerate()
            .map(|(index, memory)| json!({
                "ordinal": index + 1,
                "title": truncate_utf8(&memory.title, MAX_REPRESENTATIVE_TEXT_BYTES / 4),
                "content": truncate_utf8(&memory.content, MAX_REPRESENTATIVE_TEXT_BYTES),
            }))
            .collect::<Vec<_>>()
    }))
    .map_err(|error| DreamCommunityNamingError::InvalidOutput(error.to_string()))
}

pub(crate) fn representative_ids(
    community: &Community,
    index: &MemoryRetrievalIndex,
) -> Vec<MemoryId> {
    let member_set: HashSet<_> = community.members.iter().copied().collect();
    let centroids: Vec<_> = index
        .subcentroids
        .iter()
        .filter(|centroid| centroid.community_id == community.id)
        .collect();
    let mut selected = Vec::with_capacity(DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES);

    for centroid in &centroids {
        let mut ranked = ranked_members_for_centroid(community, index, &centroid.vector);
        let mut admitted = 0;
        for (memory_id, _) in ranked.drain(..) {
            if !selected.contains(&memory_id) {
                selected.push(memory_id);
                admitted += 1;
            }
            if admitted == REPRESENTATIVES_PER_SUBCENTROID
                || selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES
            {
                break;
            }
        }
        if selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
            break;
        }
    }

    if selected.len() < DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
        let mut remaining: Vec<_> = index
            .vectors
            .iter()
            .filter(|(memory_id, _)| member_set.contains(memory_id))
            .filter_map(|(memory_id, vector)| {
                let score = centroids
                    .iter()
                    .filter_map(|centroid| cosine(&centroid.vector, vector))
                    .max_by(f64::total_cmp)?;
                Some((*memory_id, score))
            })
            .collect();
        sort_ranked(&mut remaining);
        for (memory_id, _) in remaining {
            if selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
                break;
            }
            if !selected.contains(&memory_id) {
                selected.push(memory_id);
            }
        }
    }

    for memory_id in &community.members {
        if selected.len() == DEFAULT_COMMUNITY_NAMING_REPRESENTATIVES {
            break;
        }
        if !selected.contains(memory_id) {
            selected.push(*memory_id);
        }
    }
    selected.sort_by_key(|memory_id| memory_id.0);
    selected
}

fn ranked_members_for_centroid(
    community: &Community,
    index: &MemoryRetrievalIndex,
    centroid: &[f32],
) -> Vec<(MemoryId, f64)> {
    let mut ranked: Vec<_> = community
        .members
        .iter()
        .filter_map(|memory_id| {
            let vector = index.vectors.get(memory_id)?;
            Some((*memory_id, cosine(centroid, vector)?))
        })
        .collect();
    sort_ranked(&mut ranked);
    ranked
}

fn sort_ranked(ranked: &mut [(MemoryId, f64)]) {
    ranked.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.0.cmp(&right.0.0))
    });
}

pub(crate) fn parse_name(output: &Value) -> Result<String, DreamCommunityNamingError> {
    let raw = output.get("name").and_then(Value::as_str).ok_or_else(|| {
        DreamCommunityNamingError::InvalidOutput("missing string field name".into())
    })?;
    let name = raw.trim();
    if name.is_empty() {
        return Err(DreamCommunityNamingError::InvalidOutput(
            "name is empty".into(),
        ));
    }
    if name.len() > MAX_COMMUNITY_SEMANTIC_NAME_BYTES {
        return Err(DreamCommunityNamingError::InvalidOutput(
            "name is too long".into(),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(DreamCommunityNamingError::InvalidOutput(
            "name contains control characters".into(),
        ));
    }
    Ok(name.to_owned())
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

#[cfg(test)]
mod tests {
    use super::{representative_ids, truncate_utf8};
    use crate::memory_retrieval_index::CommunitySubcentroid;
    use crate::{Community, CommunityId, CompatibilityProfileId, MemoryId, MemoryRetrievalIndex};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn truncation_preserves_utf8_boundaries() {
        assert_eq!(truncate_utf8("aéz", 2), "a");
        assert_eq!(truncate_utf8("aéz", 3), "aé");
    }

    #[test]
    fn representative_selection_uses_two_memories_per_subcentroid() {
        let community_id = CommunityId([7; 32]);
        let members: Vec<_> = (1_u8..=10).map(|tag| MemoryId([tag; 32])).collect();
        let community = Community {
            id: community_id,
            members: members.clone(),
        };
        let vectors = HashMap::from([
            (members[0], vec![1.0, 0.0]),
            (members[1], vec![1.0, 0.0]),
            (members[2], vec![0.0, 1.0]),
            (members[3], vec![0.0, 1.0]),
            (members[4], vec![-1.0, 0.0]),
            (members[5], vec![-1.0, 0.0]),
            (members[6], vec![0.0, -1.0]),
            (members[7], vec![0.0, -1.0]),
            (members[8], vec![1.0, 1.0]),
            (members[9], vec![-1.0, -1.0]),
        ]);
        let index = MemoryRetrievalIndex {
            compatibility_profile_id: CompatibilityProfileId([1; 32]),
            memory_version: 0,
            memory_graph_version: 0,
            community_generation: Some(1),
            vector_bindings: vectors.len(),
            subcentroids_per_community: 4,
            dimensions: 2,
            indexed_memories: vectors.len(),
            routing_vectors: 4,
            vectors,
            searchable: HashSet::new(),
            memberships: HashMap::new(),
            subcentroids: vec![
                CommunitySubcentroid {
                    community_id,
                    routing_key: members[0],
                    vector: vec![1.0, 0.0],
                },
                CommunitySubcentroid {
                    community_id,
                    routing_key: members[0],
                    vector: vec![0.0, 1.0],
                },
                CommunitySubcentroid {
                    community_id,
                    routing_key: members[0],
                    vector: vec![-1.0, 0.0],
                },
                CommunitySubcentroid {
                    community_id,
                    routing_key: members[0],
                    vector: vec![0.0, -1.0],
                },
            ],
        };

        assert_eq!(representative_ids(&community, &index), members[..8]);
    }
}
