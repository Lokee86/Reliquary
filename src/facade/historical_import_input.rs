//! Canonical corpus and complete native grouped-publication/provenance evidence.
use super::{encoding::Value, fail};
use crate::{Cva, CvaError, EpisodeOrigin, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn source_key(conversation: &str, node: &str) -> Value {
    Value::map([
        ("conversation_id", Value::text(conversation)),
        ("node_id", Value::text(node)),
    ])
}
pub(super) fn required_nodes(
    cva: &Cva,
    through: u64,
) -> Result<BTreeMap<(String, String), crate::Node>, CvaError> {
    let mut required = BTreeMap::new();
    for node in cva.archive.nodes.iter().filter(|n| {
        cva.activity_position_for_turn(&n.conversation_id, &n.id)
            .is_some_and(|p| p <= through)
    }) {
        let mut cursor = Some(node.clone());
        let mut seen = BTreeSet::new();
        while let Some(node) = cursor {
            let key = (node.conversation_id.clone(), node.id.clone());
            if !seen.insert(key.clone()) {
                return Err(fail("historical source ancestry cycle"));
            }
            if required.contains_key(&key) {
                break;
            }
            required.insert(key, node.clone());
            cursor = match node.parent_id.as_deref() {
                Some(parent) => Some(
                    cva.archive
                        .nodes
                        .get(&node.conversation_id, parent)
                        .cloned()
                        .ok_or_else(|| fail("missing owner-local historical parent"))?,
                ),
                None => None,
            };
        }
    }
    Ok(required)
}
pub(super) fn node_binding(node: &crate::Node) -> Value {
    Value::map([
        ("conversation_id", Value::text(&node.conversation_id)),
        ("node_id", Value::text(&node.id)),
        (
            "parent_id",
            node.parent_id.as_ref().map_or(Value::Null, Value::text),
        ),
        ("role", Value::text(&node.role)),
        (
            "principal_id",
            node.principal_id.as_ref().map_or(Value::Null, Value::text),
        ),
        ("timestamp_ns", Value::Signed(node.timestamp_ns)),
        ("content_id", Value::bytes(node.content_id.0)),
    ])
}
pub(super) fn corpus_digest(cva: &mut Cva, through: u64) -> Result<[u8; 32], CvaError> {
    let nodes = required_nodes(cva, through)?;
    for node in nodes.values() {
        let bytes = cva
            .archive
            .content_bytes(&mut cva.container, node.content_id)?;
        if crate::archive_store::hash_content(&bytes) != node.content_id {
            return Err(fail("import source content proof mismatch"));
        }
    }
    Value::Array(nodes.values().map(node_binding).collect())
        .digest("reliquary:historical-corpus:v1")
}
pub(super) struct AdmissionProof {
    pub initial: BTreeMap<MemoryId, FreshnessRecord>,
    pub memories: Vec<Value>,
    pub cohorts: Vec<Value>,
}
pub(super) fn admission_proof(cva: &mut Cva) -> Result<AdmissionProof, CvaError> {
    let owner = cva
        .owner_uuid()
        .ok_or_else(|| fail("missing import owner"))?;
    let ids: BTreeSet<_> = cva.memory_ids().into_iter().collect();
    let mut birth_episodes = BTreeMap::new();
    let mut completed = BTreeSet::new();
    let mut publications = BTreeMap::<[u8; 32], BTreeSet<[u8; 32]>>::new();
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path())?;
    scan.visit_payloads::<CvaError>(|_, payload| {
        if let Some(completion) =
            crate::insomnia::completion::decode_completion(payload).map_err(|e| fail(e))?
        {
            let episode = cva
                .episode(completion.episode_id)
                .ok_or_else(|| fail("import completion lacks Episode"))?;
            if episode.origin != EpisodeOrigin::Import {
                return Err(fail("history includes a non-import completion"));
            }
            completed.insert(episode.id.0);
            publications
                .entry(episode.id.0)
                .or_default()
                .insert(Sha256::digest(payload).into());
            for id in completion.freshness_birth_memory_ids {
                let record = completion
                    .records
                    .iter()
                    .find(|r| r.id == id && r.revision == 1)
                    .ok_or_else(|| fail("import birth lacks first revision"))?;
                if cva.memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
                    || record.source_episode_id != Some(episode.id)
                {
                    return Err(fail("import publication conflicts with Memory birth"));
                }
                if birth_episodes
                    .insert(id, episode.id)
                    .is_some_and(|old| old != episode.id)
                {
                    return Err(fail("conflicting import birth cohort"));
                }
            }
        }
        Ok(())
    })?;
    if ids.is_empty() || birth_episodes.keys().any(|id| !ids.contains(id)) {
        return Err(fail("incomplete native import birth cohort"));
    }
    let mut covered = BTreeSet::new();
    let mut episode_keys = BTreeMap::new();
    let mut episode_proofs = BTreeMap::new();
    for episode in cva.episodes() {
        if episode.origin != EpisodeOrigin::Import || !completed.contains(&episode.id.0) {
            return Err(fail("import corpus has incomplete or non-import Episodes"));
        }
        let end = cva
            .archive
            .nodes
            .get(&episode.conversation_id, &episode.end_node_id)
            .ok_or_else(|| fail("missing Episode endpoint"))?;
        if end.timestamp_ns != episode.source_through_ns {
            return Err(fail(
                "Episode source-through timestamp conflicts with endpoint",
            ));
        }
        let turns = cva.episode_turns(episode.id)?;
        if turns
            .first()
            .is_none_or(|t| t.node_id != episode.start_node_id)
            || turns
                .last()
                .is_none_or(|t| t.node_id != episode.end_node_id)
        {
            return Err(fail("Episode path endpoint mismatch"));
        }
        let mut keys = BTreeSet::new();
        let mut path = Vec::new();
        for turn in turns {
            let node = cva
                .archive
                .nodes
                .get(&episode.conversation_id, &turn.node_id)
                .ok_or_else(|| fail("missing Episode path Node"))?;
            if node.content_id != crate::archive_store::hash_content(turn.content.as_bytes()) {
                return Err(fail("Episode retained content binding conflict"));
            }
            path.push(node_binding(node));
            if let Some(position) =
                cva.activity_position_for_turn(&episode.conversation_id, &turn.node_id)
            {
                covered.insert(position);
                keys.insert((episode.conversation_id.clone(), turn.node_id));
            }
        }
        episode_keys.insert(episode.id.0, keys);
        episode_proofs.insert(
            episode.id.0,
            Value::map([
                ("episode_id", Value::bytes(episode.id.0)),
                ("conversation_id", Value::text(&episode.conversation_id)),
                ("start_node_id", Value::text(&episode.start_node_id)),
                ("end_node_id", Value::text(&episode.end_node_id)),
                (
                    "source_through_ns",
                    Value::Signed(episode.source_through_ns),
                ),
                ("path", Value::Array(path)),
                (
                    "publication_payload_digests",
                    Value::Array(
                        publications[&episode.id.0]
                            .iter()
                            .map(Value::bytes)
                            .collect(),
                    ),
                ),
            ]),
        );
    }
    if covered != (1..=cva.rel_turn_count()).collect() {
        return Err(fail("import has unprocessed Archive tail/branch"));
    }
    let mut members = BTreeMap::new();
    let mut cohort_origins = BTreeMap::new();
    let mut cohort_members = BTreeMap::<[u8; 32], Vec<MemoryId>>::new();
    let mut cohort_positions = BTreeMap::<[u8; 32], Option<u64>>::new();
    for id in ids {
        let memory = cva.memory(id)?;
        let first = cva.memory_revision(id, 1)?;
        let grouped = birth_episodes.get(&id).copied();
        if memory.source_episode_id != first.source_episode_id
            || grouped.is_some_and(|episode| memory.source_episode_id != Some(episode))
        {
            return Err(fail("import cohort origin rewritten"));
        }
        let episode = memory
            .source_episode_id
            .map(|episode| {
                cva.episode(episode)
                    .cloned()
                    .ok_or_else(|| fail("missing owner Episode"))
            })
            .transpose()?;
        let (cohort_key, origin) = if let Some(episode) = grouped {
            (
                Value::map([
                    ("owner_uuid", Value::bytes(owner)),
                    ("episode_id", Value::bytes(episode.0)),
                ])
                .digest("reliquary:historical-episode-cohort:v1")?,
                episode_proofs[&episode.0].clone(),
            )
        } else {
            if !cva.freshness_is_publication_birth(id)
                || cva.freshness_publication_cohort(id)? != BTreeSet::from([id])
                || cva.memories.first_mutation_id(id) != Some(first.mutation_id.as_str())
            {
                return Err(fail("unverified direct singleton publication"));
            }
            let key = Value::map([
                ("owner_uuid", Value::bytes(owner)),
                ("memory_id", Value::bytes(id.0)),
            ])
            .digest("reliquary:historical-singleton-cohort:v1")?;
            (
                key,
                Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    ("first_mutation_id", Value::text(&first.mutation_id)),
                    (
                        "first_body_id",
                        Value::bytes(
                            crate::memory_model::memory_body_id(&first.title, &first.content).0,
                        ),
                    ),
                    ("first_revision", Value::Unsigned(first.revision)),
                    ("global_version", Value::Unsigned(first.global_version)),
                    ("publication", Value::text("verified-native-singleton-v1")),
                ]),
            )
        };
        if cohort_origins
            .insert(cohort_key, origin.clone())
            .is_some_and(|old| old != origin)
        {
            return Err(fail("conflicting publication cohort proof"));
        }
        if let Some(source) = &memory.source_ref {
            if Some(&source.owner_id) != cva.owner_id().as_ref()
                || Some(source.source_episode_id) != memory.source_episode_id
                || Some(&source.source_node_id) != memory.source_node_id.as_ref()
                || source.content_source_conversation_id != memory.content_source_conversation_id
                || source.content_source_node_id != memory.content_source_node_id
                || source.grounding_source_conversation_id
                    != memory.grounding_source_conversation_id
                || source.grounding_source_node_id != memory.grounding_source_node_id
            {
                return Err(fail("foreign or conflicting structured import provenance"));
            }
            let node = cva
                .archive
                .nodes
                .get(
                    &episode
                        .as_ref()
                        .ok_or_else(|| fail("structured source lacks Episode"))?
                        .conversation_id,
                    &source.source_node_id,
                )
                .ok_or_else(|| fail("missing structured source"))?;
            if source.principal_id != node.principal_id {
                return Err(fail("structured source principal conflict"));
            }
        }
        let mut necessary = match &episode {
            Some(episode) => episode_keys
                .get(&episode.id.0)
                .cloned()
                .ok_or_else(|| fail("direct publication Episode is outside completed corpus"))?,
            None => BTreeSet::new(),
        };
        if let Some(node) = &memory.source_node_id {
            necessary.insert((
                episode
                    .as_ref()
                    .ok_or_else(|| fail("declared source node lacks Episode namespace"))?
                    .conversation_id
                    .clone(),
                node.clone(),
            ));
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
                    necessary.insert((c.clone(), n.clone()));
                }
                (None, None) => {}
                _ => return Err(fail("partial import provenance pair")),
            }
        }
        let mut latest = None;
        for (conversation, node) in &necessary {
            let position = cva
                .activity_position_for_turn(conversation, node)
                .ok_or_else(|| {
                    fail("declared import source lacks owner-local accepted activity")
                })?;
            latest = Some(latest.unwrap_or(0).max(position));
        }
        let position = cohort_positions.entry(cohort_key).or_insert(Some(0));
        *position = match (*position, latest) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
        cohort_members.entry(cohort_key).or_default().push(id);
        members.insert(id, (memory, necessary, cohort_key));
    }
    let mut cohorts = Vec::new();
    for (key, ids) in &cohort_members {
        cohorts.push((
            *key,
            Value::map([
                ("cohort_key", Value::bytes(key)),
                ("origin", cohort_origins[key].clone()),
                (
                    "members",
                    Value::Array(ids.iter().map(|id| Value::bytes(id.0)).collect()),
                ),
                (
                    "availability",
                    cohort_positions[key].map_or(Value::Null, Value::Unsigned),
                ),
            ]),
        ));
    }
    cohorts.sort_by_key(|(key, _)| *key);
    let mut initial = BTreeMap::new();
    let mut memories = Vec::new();
    for (id, (memory, necessary, cohort_key)) in members {
        let available = cohort_positions[&cohort_key];
        let eligible = !memory.archived
            && memory.superseded_by.is_none()
            && matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical");
        let status = if available.is_none() {
            "unsupported"
        } else if eligible {
            "supported"
        } else {
            "credit-excluded"
        };
        if eligible && let Some(at) = available {
            let mut record = FreshnessRecord::created();
            record.admit(at).map_err(|e| fail(&e.to_string()))?;
            initial.insert(id, record);
        }
        let body = cva.memory_body_id(id)?;
        if crate::memory_model::memory_body_id(&memory.title, &memory.content) != body {
            return Err(fail("historical BodyId content conflict"));
        }
        let current = cva
            .freshness_record(id)
            .ok_or_else(|| fail("missing provisional scoring record"))?;
        memories.push(Value::map([
            ("memory_id", Value::bytes(id.0)),
            ("body_id", Value::bytes(body.0)),
            ("revision", Value::Unsigned(memory.revision)),
            ("lifecycle", Value::text(&memory.lifecycle_state)),
            ("archived", Value::Bool(memory.archived)),
            (
                "superseded_by",
                memory
                    .superseded_by
                    .map_or(Value::Null, |id| Value::bytes(id.0)),
            ),
            (
                "necessary_sources",
                Value::Array(necessary.iter().map(|(c, n)| source_key(c, n)).collect()),
            ),
            (
                "availability",
                available.map_or(Value::Null, Value::Unsigned),
            ),
            ("cohort_key", Value::bytes(cohort_key)),
            ("status", Value::text(status)),
            (
                "reason",
                Value::text(if available.is_none() {
                    "cohort-missing-usable-provenance"
                } else if eligible {
                    "verified-active-native-publication"
                } else {
                    "final-lifecycle-excluded"
                }),
            ),
            ("mutation_id", Value::text(&memory.mutation_id)),
            (
                "source_episode_id",
                memory
                    .source_episode_id
                    .map_or(Value::Null, |id| Value::bytes(id.0)),
            ),
            (
                "scoring_proof_digest",
                super::manifest::record_value(current)
                    .digest("reliquary:historical-provisional-record:v1")
                    .map(Value::bytes)?,
            ),
        ]));
    }
    Ok(AdmissionProof {
        initial,
        memories,
        cohorts: cohorts.into_iter().map(|(_, proof)| proof).collect(),
    })
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
    for id in cp.cohort.difference(&births) {
        let first = cva.memory_revision(*id, 1)?;
        if first.global_version > cp.global_version
            || !cva.freshness_is_publication_birth(*id)
            || cva.freshness_publication_cohort(*id)? != BTreeSet::from([*id])
            || cva.memories.first_mutation_id(*id) != Some(first.mutation_id.as_str())
        {
            return Err(fail(
                "checkpoint lacks verified native singleton publication",
            ));
        }
    }
    if !births.is_subset(&cp.cohort) {
        return Err(fail(
            "import checkpoint does not cover a proven native import",
        ));
    }
    let nodes = required_nodes(cva, cp.cut)?;
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
