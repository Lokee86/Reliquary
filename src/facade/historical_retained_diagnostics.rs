//! Read-only prerequisite diagnosis; no gate is bypassed for publication.
use super::*;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn diagnose(cva: &mut Cva) -> Value {
    let mut regressions = Vec::new();
    let mut ancestry_clock_violations = 0;
    for node in cva
        .archive
        .nodes
        .iter()
        .filter(|node| matches!(node.role.as_str(), "user" | "assistant"))
    {
        let mut parent = node.parent_id.as_deref();
        let mut seen = BTreeSet::new();
        while let Some(parent_id) = parent {
            if !seen.insert(parent_id) {
                break;
            }
            let Some(ancestor) = cva.archive.nodes.get(&node.conversation_id, parent_id) else {
                break;
            };
            if matches!(ancestor.role.as_str(), "user" | "assistant") {
                let a = cva
                    .activity_position_for_turn(&ancestor.conversation_id, &ancestor.id)
                    .unwrap();
                let b = cva
                    .activity_position_for_turn(&node.conversation_id, &node.id)
                    .unwrap();
                ancestry_clock_violations += usize::from(a >= b);
                if ancestor.timestamp_ns > node.timestamp_ns {
                    regressions.push(json!({"conversation_id":node.conversation_id,"ancestor_id":ancestor.id,
                        "descendant_id":node.id,"ancestor_timestamp_ns":ancestor.timestamp_ns,
                        "descendant_timestamp_ns":node.timestamp_ns,"regression_ns":i128::from(ancestor.timestamp_ns)-i128::from(node.timestamp_ns),
                        "ancestor_activity_position":a,"descendant_activity_position":b}));
                }
                break;
            }
            parent = ancestor.parent_id.as_deref();
        }
    }
    let chronology = match cva.historical_freshness_timeline() {
        Ok(timeline) => {
            let mismatches: Vec<_> = timeline
                .activity_order
                .iter()
                .enumerate()
                .filter_map(|(i, key)| {
                    let actual = cva.activity_position_for_turn(&key.conversation_id, &key.node_id);
                    let expected = i as u64 + 1;
                    (actual != Some(expected)).then(|| {
                        json!({"conversation_id":key.conversation_id,"node_id":key.node_id,
                    "expected":expected,"actual":actual})
                    })
                })
                .collect();
            json!({"timeline_planned":true,"positions":timeline.activity_order.len(),"clock_mismatch_count":mismatches.len(),
                "first_mismatches":mismatches.into_iter().take(8).collect::<Vec<_>>()})
        }
        Err(error) => json!({"timeline_planned":false,"error":error.to_string()}),
    };
    let mut completed = BTreeSet::new();
    let mut births = BTreeSet::new();
    let mut first_revisions = BTreeSet::new();
    let mut verified_legacy_births = BTreeSet::new();
    let mut completion_versions = BTreeMap::<String, usize>::new();
    let mut decoded = 0;
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path()).unwrap();
    scan.visit_payloads::<crate::CvaError>(|_, payload| {
        if let Some(record) =
            crate::insomnia::completion::decode_completion(payload).map_err(|e| fail(e))?
        {
            decoded += 1;
            *completion_versions
                .entry(String::from_utf8_lossy(&payload[..8]).into_owned())
                .or_default() += 1;
            completed.insert(record.episode_id.0);
            births.extend(record.freshness_birth_memory_ids);
            first_revisions.extend(
                record
                    .records
                    .iter()
                    .filter(|record| record.revision == 1)
                    .map(|record| record.id),
            );
            for first in record.records.iter().filter(|first| first.revision == 1) {
                if first.source_episode_id == Some(record.episode_id)
                    && cva.memories.first_mutation_id(first.id) == Some(first.mutation_id.as_str())
                {
                    verified_legacy_births.insert(first.id);
                }
            }
        }
        Ok(())
    })
    .unwrap();
    let ids = cva.memory_ids();
    let mut publication_births = 0;
    let mut source_episodes = 0;
    let mut structured_refs = 0;
    let mut settled_passes = 0;
    let mut episode_origins = BTreeMap::<String, usize>::new();
    let mut complete_coverage = BTreeSet::new();
    let mut pending_episodes = Vec::new();
    let mut canonical_path = BTreeSet::new();
    let mut other_path = BTreeSet::new();
    for branch in cva.archive.branches.iter() {
        let positions = if branch.canonical {
            &mut canonical_path
        } else {
            &mut other_path
        };
        let mut cursor = Some(branch.leaf_node_id.as_str());
        let mut seen = BTreeSet::new();
        while let Some(id) = cursor {
            if !seen.insert(id) {
                break;
            }
            let Some(node) = cva.archive.nodes.get(&branch.conversation_id, id) else {
                break;
            };
            if let Some(position) = cva.activity_position_for_turn(&branch.conversation_id, id) {
                positions.insert(position);
            }
            cursor = node.parent_id.as_deref();
        }
    }
    for episode in cva.episodes() {
        *episode_origins
            .entry(format!("{:?}", episode.origin))
            .or_default() += 1;
        let turns = cva.episode_turns(episode.id).unwrap();
        let positions: Vec<_> = turns
            .iter()
            .filter_map(|turn| {
                cva.activity_position_for_turn(&episode.conversation_id, &turn.node_id)
            })
            .collect();
        if completed.contains(&episode.id.0) {
            complete_coverage.extend(positions.iter().copied());
        }
        if cva
            .insomnia_work(episode.id)
            .is_some_and(|work| work.state != crate::InsomniaWorkState::Complete)
        {
            let output_count = ids
                .iter()
                .filter(|&&id| cva.memory(id).unwrap().source_episode_id == Some(episode.id))
                .count();
            pending_episodes.push(json!({"episode_id":hex(&episode.id.0),"state":format!("{:?}",cva.insomnia_work(episode.id).unwrap().state),
                "activity_positions":positions,"current_memory_outputs":output_count}));
        }
    }
    for &id in &ids {
        let memory = cva.memory(id).unwrap();
        publication_births += usize::from(cva.freshness_is_publication_birth(id));
        source_episodes += usize::from(memory.source_episode_id.is_some());
        structured_refs += usize::from(memory.source_ref.is_some());
        settled_passes += usize::from(cva.initial_dream_pass_settled(
            &crate::dream_freshness::initial_pass_id(cva.owner_uuid().unwrap(), id),
        ));
    }
    let owner_gate = cva
        .historical_reconstruction_owner()
        .map(|_| "passed".to_owned())
        .unwrap_or_else(|e| e.to_string());
    let admission_gate = input::admission_proof(cva)
        .map(|_| "passed".to_owned())
        .unwrap_or_else(|e| e.to_string());
    let uncovered: BTreeSet<_> = (1..=cva.rel_turn_count())
        .filter(|position| !complete_coverage.contains(position))
        .collect();
    let uncovered_canonical = uncovered.intersection(&canonical_path).count();
    let uncovered_other = uncovered
        .difference(&canonical_path)
        .filter(|position| other_path.contains(position))
        .count();
    let relations = cva.graph_relations();
    let dream_origin_relations = relations
        .iter()
        .filter(|r| r.origin == crate::GraphRelationOrigin::Dream)
        .count();
    let memory_ids = ids.iter().copied().collect();
    let community_gate = manifest::community_proof(
        cva.community_snapshot(),
        cva.memory_graph_version(),
        &memory_ids,
    )
    .map(|_| "passed".to_owned())
    .unwrap_or_else(|e| e.to_string());
    json!({"owner_generation_gate":owner_gate,"admission_cohort_gate":admission_gate,"chronology":chronology,
        "timestamp_regression_count":regressions.len(),"timestamp_regressions":regressions,
        "accepted_clock_ancestry_violations":ancestry_clock_violations,"completion_payloads":decoded,
        "episodes_with_completion_payload":completed.len(),"completion_birth_memory_ids":births.len(),
        "completion_format_versions":completion_versions,"completion_first_revision_memory_ids":first_revisions.len(),
        "current_memories_with_completion_first_revision":ids.iter().filter(|id|first_revisions.contains(id)).count(),
        "current_memories_with_matching_legacy_first_revision_and_episode":ids.iter().filter(|id|verified_legacy_births.contains(id)).count(),
        "current_memories_with_native_birth_proof":publication_births,"current_memories_with_source_episode":source_episodes,
        "current_memories_with_settled_native_first_cycle":settled_passes,"dream_origin_memory_relations":dream_origin_relations,
        "community_gate":community_gate,
        "current_memories_with_structured_source_ref":structured_refs,"episode_origins":episode_origins,
        "completed_episode_activity_coverage":complete_coverage.len(),"total_activity":cva.rel_turn_count(),
        "uncovered_activity":uncovered.len(),"uncovered_canonical_branch_activity":uncovered_canonical,
        "uncovered_noncanonical_only_activity":uncovered_other,
        "uncovered_activity_outside_current_branches":uncovered.len()-uncovered_canonical-uncovered_other,
        "unfinished_episodes":pending_episodes})
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
