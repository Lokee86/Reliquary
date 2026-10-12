//! Canonical source-corpus identity and native import admission proof.
use crate::{Cva, CvaError, EpisodeOrigin, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
fn bytes(out: &mut Vec<u8>, tag: u8, data: &[u8]) {
    out.push(tag);
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
}
fn text(out: &mut Vec<u8>, s: &str) {
    bytes(out, 5, s.as_bytes());
}
fn nullable(out: &mut Vec<u8>, s: Option<&str>) {
    if let Some(s) = s {
        text(out, s)
    } else {
        out.push(0)
    }
}
fn required_nodes(cva: &Cva, through: u64) -> BTreeMap<(String, String), crate::Node> {
    let mut required = BTreeMap::new();
    for node in cva.archive.nodes.iter().filter(|node| {
        cva.activity_position_for_turn(&node.conversation_id, &node.id)
            .is_some_and(|position| position <= through)
    }) {
        let mut cursor = Some(node.clone());
        while let Some(node) = cursor {
            if required
                .insert(
                    (node.conversation_id.clone(), node.id.clone()),
                    node.clone(),
                )
                .is_some()
            {
                break;
            }
            cursor = node
                .parent_id
                .as_ref()
                .and_then(|id| cva.archive.nodes.get(&node.conversation_id, id))
                .cloned();
        }
    }
    required
}
pub(super) fn corpus_digest(cva: &mut Cva, through: u64) -> Result<[u8; 32], CvaError> {
    let required = required_nodes(cva, through);
    let mut encoded = vec![6];
    encoded.extend((required.len() as u32).to_le_bytes());
    for node in required.values() {
        let content = cva
            .archive
            .content_bytes(&mut cva.container, node.content_id)?;
        if crate::archive_store::hash_content(&content) != node.content_id {
            return Err(CvaError::Freshness(
                "import source content proof mismatch".into(),
            ));
        }
        encoded.push(7);
        encoded.extend(7u32.to_le_bytes());
        text(&mut encoded, "content_id");
        bytes(&mut encoded, 4, &node.content_id.0);
        text(&mut encoded, "conversation_id");
        text(&mut encoded, &node.conversation_id);
        text(&mut encoded, "node_id");
        text(&mut encoded, &node.id);
        text(&mut encoded, "parent_id");
        nullable(&mut encoded, node.parent_id.as_deref());
        text(&mut encoded, "principal_id");
        nullable(&mut encoded, node.principal_id.as_deref());
        text(&mut encoded, "role");
        text(&mut encoded, &node.role);
        text(&mut encoded, "timestamp_ns");
        encoded.push(2);
        encoded.extend(node.timestamp_ns.to_le_bytes());
    }
    let mut hash = Sha256::new();
    hash.update(b"reliquary:historical-corpus:v1\0");
    hash.update(encoded);
    Ok(hash.finalize().into())
}
pub(super) fn initial_import_records(
    cva: &mut Cva,
) -> Result<BTreeMap<MemoryId, FreshnessRecord>, CvaError> {
    let ids: BTreeSet<_> = cva.memory_ids().into_iter().collect();
    let mut birth_episodes = BTreeMap::new();
    let mut born = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path())?;
    scan.visit_payloads::<CvaError>(|_, payload| {
        if let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|e| CvaError::Freshness(e.into()))?
        {
            let episode = cva
                .episode(completion.episode_id)
                .ok_or_else(|| CvaError::Freshness("import completion lacks Episode".into()))?;
            if episode.origin != EpisodeOrigin::Import {
                return Err(CvaError::Freshness(
                    "history includes a non-import completion".into(),
                ));
            }
            completed.insert(episode.id.0);
            for id in completion.freshness_birth_memory_ids {
                let record = completion
                    .records
                    .iter()
                    .find(|r| r.id == id && r.revision == 1)
                    .ok_or_else(|| {
                        CvaError::Freshness("import birth lacks an accepted first revision".into())
                    })?;
                if cva.memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                    || record.source_episode_id != Some(episode.id)
                {
                    return Err(CvaError::Freshness(
                        "import publication proof differs from its Memory birth".into(),
                    ));
                }
                birth_episodes.insert(id, episode.id);
                born.insert(id);
            }
        }
        Ok(())
    })?;
    if ids.is_empty() || ids != born {
        return Err(CvaError::Freshness("checkpoint requires a complete native import birth cohort; ordinary enrollment is unsupported".into()));
    }
    let mut covered = BTreeSet::new();
    let mut by_episode = BTreeMap::new();
    for episode in cva.episodes() {
        if episode.origin != EpisodeOrigin::Import || !completed.contains(&episode.id.0) {
            return Err(CvaError::Freshness(
                "import corpus has incomplete or non-import Episodes".into(),
            ));
        }
        let turns = cva.episode_turns(episode.id)?;
        let mut latest = 0;
        for turn in turns {
            if let Some(position) =
                cva.activity_position_for_turn(&episode.conversation_id, &turn.node_id)
            {
                covered.insert(position);
                latest = latest.max(position);
            }
        }
        by_episode.insert(episode.id.0, latest);
    }
    if covered.len() as u64 != cva.rel_turn_count() {
        return Err(CvaError::Freshness(
            "import has an unprocessed Archive tail/branch".into(),
        ));
    }
    let mut members = Vec::new();
    let mut cohorts = BTreeMap::<[u8; 32], u64>::new();
    for id in ids {
        let memory = cva.memory(id)?;
        if memory.source_episode_id != Some(birth_episodes[&id]) {
            return Err(CvaError::Freshness(
                "import cohort origin was rewritten".into(),
            ));
        }
        if memory.source_ref.is_some() {
            return Err(CvaError::Freshness(
                "structured/cross-owner import provenance conversion is unsupported".into(),
            ));
        }
        let ep = memory
            .source_episode_id
            .and_then(|id| cva.episode(id))
            .cloned()
            .ok_or_else(|| CvaError::Freshness("import Memory lacks local Episode proof".into()))?;
        let mut latest = *by_episode.get(&ep.id.0).ok_or_else(|| {
            CvaError::Freshness("import Memory Episode is outside the corpus".into())
        })?;
        if let Some(node) = &memory.source_node_id {
            latest = latest.max(
                cva.activity_position_for_turn(&ep.conversation_id, node)
                    .ok_or_else(|| {
                        CvaError::Freshness("import source lacks accepted activity".into())
                    })?,
            );
        }
        for (conversation, node) in [
            (
                &memory.content_source_conversation_id,
                &memory.content_source_node_id,
            ),
            (
                &memory.grounding_source_conversation_id,
                &memory.grounding_source_node_id,
            ),
        ] {
            match (conversation, node) {
                (Some(c), Some(n)) => {
                    latest = latest.max(cva.activity_position_for_turn(c, n).ok_or_else(|| {
                        CvaError::Freshness("import authority/grounding lacks activity".into())
                    })?)
                }
                (None, None) => {}
                _ => return Err(CvaError::Freshness("partial import provenance pair".into())),
            }
        }
        if latest == 0 {
            return Err(CvaError::Freshness(
                "import birth has no verified historical position".into(),
            ));
        }
        cohorts
            .entry(ep.id.0)
            .and_modify(|position| *position = (*position).max(latest))
            .or_insert(latest);
        let eligible = !memory.archived
            && memory.superseded_by.is_none()
            && matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical");
        members.push((id, ep.id.0, eligible));
    }
    let mut initial = BTreeMap::new();
    for (id, ep, eligible) in members {
        if !eligible {
            continue;
        }
        let mut record = FreshnessRecord::created();
        if eligible {
            record
                .admit(cohorts[&ep])
                .map_err(|e| CvaError::Freshness(e.to_string()))?;
        }
        initial.insert(id, record);
    }
    Ok(initial)
}

pub(super) fn validate_checkpoint(
    cva: &mut Cva,
    cp: &crate::freshness_storage::ImportCheckpoint,
) -> Result<(), CvaError> {
    let fail = |s: &str| CvaError::Freshness(s.into());
    if cva.owner_uuid() != Some(cp.owner)
        || cp.cut > cva.rel_turn_count()
        || cp.global_version > cva.latest_global_version()
        || cp.graph_version > cva.graph_version()
        || cp.memory_graph_version > cva.memory_graph_version()
        || cp.archive_version > cva.archive_version()
    {
        return Err(fail("import checkpoint input cut is unavailable"));
    }
    let original: BTreeSet<_> = cva
        .memory_ids()
        .into_iter()
        .filter(|id| {
            cva.memories
                .first_global_version(*id)
                .is_some_and(|version| version <= cp.global_version)
        })
        .collect();
    if original != cp.cohort {
        return Err(fail(
            "import checkpoint cohort differs from its accepted Memory prefix",
        ));
    }
    let mut births = BTreeSet::new();
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path())?;
    scan.visit_payloads::<CvaError>(|_, payload| {
        if let Some(completion) =
            crate::insomnia::completion::decode_completion(payload).map_err(|e| fail(e))?
        {
            for id in completion.freshness_birth_memory_ids {
                if !cp.cohort.contains(&id) {
                    continue;
                }
                let record = completion
                    .records
                    .iter()
                    .find(|r| r.id == id && r.revision == 1)
                    .ok_or_else(|| fail("import checkpoint lacks first-publication proof"))?;
                let episode = cva
                    .episode(completion.episode_id)
                    .ok_or_else(|| fail("import checkpoint Episode is unavailable"))?;
                if record.global_version > cp.global_version
                    || record.source_episode_id != Some(episode.id)
                    || episode.origin != EpisodeOrigin::Import
                    || cva.memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                {
                    return Err(fail("import checkpoint native birth proof conflicts"));
                }
                births.insert(id);
            }
        }
        Ok(())
    })?;
    if births != cp.cohort {
        return Err(fail(
            "import checkpoint does not cover a proven native import",
        ));
    }
    let nodes = required_nodes(cva, cp.cut);
    use crate::freshness::historical_planner::{HistoricalSource, HistoricalTimeline, SourceKey};
    let sources = nodes
        .values()
        .map(|node| HistoricalSource {
            key: SourceKey {
                conversation_id: node.conversation_id.clone(),
                node_id: node.id.clone(),
            },
            parent_id: node.parent_id.clone(),
            role: node.role.clone(),
            timestamp_ns: node.timestamp_ns,
        })
        .collect();
    let timeline = HistoricalTimeline::plan(sources).map_err(|e| fail(&e.to_string()))?;
    let positions = timeline
        .activity_order
        .iter()
        .filter_map(|key| {
            cva.activity_position_for_turn(&key.conversation_id, &key.node_id)
                .map(|position| (key.clone(), position))
        })
        .collect();
    timeline
        .validate_archive_clock(&positions, cp.cut)
        .map_err(|e| fail(&e.to_string()))?;
    if corpus_digest(cva, cp.cut)? != cp.corpus_digest {
        return Err(fail(
            "import checkpoint corpus identity differs from its source prefix",
        ));
    }
    Ok(())
}
