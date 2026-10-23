//! Canonical corpus and complete native grouped-publication/provenance evidence.
use super::{encoding::Value, fail};
use crate::{Cva, CvaError, EpisodeOrigin, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) use crate::history::source_proof::{node_binding, required_nodes, source_key};
pub(super) fn corpus_digest(cva: &mut Cva, through: u64) -> Result<[u8; 32], CvaError> {
    let nodes = required_nodes(&cva.archive, through)?;
    let mut bindings = Vec::with_capacity(nodes.len());
    for node in nodes.values() {
        let bytes = cva
            .archive
            .content_bytes(&mut cva.container, node.content_id)?;
        if crate::archive_store::hash_content(&bytes) != node.content_id {
            return Err(fail("import source content proof mismatch"));
        }
        bindings.push(node_binding(node));
    }
    crate::history::source_proof::corpus_digest(bindings).map_err(CvaError::from)
}
pub(super) fn scoped_corpus_digest(
    cva: &mut Cva,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<[u8; 32], CvaError> {
    epoch::scoped_corpus_digest(cva, source_nodes)
}

#[derive(Clone, Debug)]
pub(super) struct HistoricalMemoryInput {
    pub body_id: crate::MemoryBodyId,
    pub cohort_key: [u8; 32],
    pub availability: Option<u64>,
    pub credit_eligible: bool,
    pub necessary_sources: BTreeSet<crate::freshness::historical_planner::SourceKey>,
}
pub(super) struct AdmissionProof {
    pub memory_inputs: BTreeMap<MemoryId, HistoricalMemoryInput>,
    pub initial: BTreeMap<MemoryId, FreshnessRecord>,
    pub memories: Vec<Value>,
    pub cohorts: Vec<Value>,
}
pub(super) fn admission_proof(cva: &mut Cva) -> Result<AdmissionProof, CvaError> {
    admission_proof_legacy(cva)
}

pub(super) fn admission_proof_scoped(
    cva: &mut Cva,
    scope: Option<&crate::imports::ImportEpochScope>,
) -> Result<AdmissionProof, CvaError> {
    match scope {
        Some(scope) => epoch::admission_proof_for_scope(cva, scope),
        None => admission_proof_legacy(cva),
    }
}

fn admission_proof_legacy(cva: &mut Cva) -> Result<AdmissionProof, CvaError> {
    let owner = cva
        .owner_uuid()
        .ok_or_else(|| fail("missing import owner"))?;
    let ids: BTreeSet<_> = cva.memory_ids().into_iter().collect();
    let mut birth_episodes = BTreeMap::new();
    let mut completed = BTreeSet::new();
    let mut publications = BTreeMap::<[u8; 32], BTreeSet<[u8; 32]>>::new();
    let scan = &cva.container;
    {
        let mut ingest_frame = |frame: crate::container::PhysicalFrame| -> Result<(), CvaError> {
            let payload = frame.payload.as_slice();
            {
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
            }
        };
        let mut cursor = scan.physical_cursor()?;
        (|| {
            while let Some(frame) = cursor.next_frame()? {
                ingest_frame(frame)?;
            }
            Ok::<(), CvaError>(())
        })()
    }?;
    if ids.is_empty() || birth_episodes.keys().any(|id| !ids.contains(id)) {
        return Err(fail("incomplete native import birth cohort"));
    }
    let mut covered = BTreeSet::new();
    let mut episode_keys = BTreeMap::new();
    let mut episode_proofs = BTreeMap::new();
    for episode in cva.episodes() {
        if episode.origin != EpisodeOrigin::Import || !completed.contains(&episode.id.0) {
            return Err(fail("import epoch has incomplete or non-import Episodes"));
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
    let mut memory_inputs = BTreeMap::new();
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
        if let Some(at) = available {
            let mut record = FreshnessRecord::created();
            if eligible {
                record.admit(at).map_err(|e| fail(&e.to_string()))?;
            }
            initial.insert(id, record);
        }
        let body = cva.memory_body_id(id)?;
        if crate::memory_model::memory_body_id(&memory.title, &memory.content) != body {
            return Err(fail("historical BodyId content conflict"));
        }
        memory_inputs.insert(
            id,
            HistoricalMemoryInput {
                body_id: body,
                cohort_key,
                availability: available,
                credit_eligible: eligible,
                necessary_sources: necessary
                    .iter()
                    .map(|(c, n)| crate::freshness::historical_planner::SourceKey {
                        conversation_id: c.clone(),
                        node_id: n.clone(),
                    })
                    .collect(),
            },
        );
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
        memory_inputs,
        initial,
        memories,
        cohorts: cohorts.into_iter().map(|(_, proof)| proof).collect(),
    })
}

pub(super) fn validate_settlement(
    cva: &mut Cva,
    cp: &crate::freshness_storage::HistoricalImportFreshnessSettlement,
) -> Result<(), CvaError> {
    let fail = |s: &str| CvaError::Freshness(s.into());
    if cva.owner_uuid() != Some(cp.owner)
        || cp.settlement_cut > cva.rel_turn_count()
        || cp.global_version > cva.latest_global_version()
        || cp.graph_version > cva.graph_version()
        || cp.memory_graph_version > cva.memory_graph_version()
        || cp.archive_version > cva.archive_version()
    {
        return Err(fail("import checkpoint input cut is unavailable"));
    }
    {
        let scope = cp
            .semantic
            .as_ref()
            .ok_or_else(|| fail("accepted settlement lacks semantic authority"))?;
        cp.validate().map_err(CvaError::Freshness)?;
        cva.validate_semantic_import_scope(cp, scope)?;
        cva.freshness
            .validate_semantic_import_evidence(scope)
            .map_err(CvaError::Freshness)?;
        let import_id: String = cp
            .import_id
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if cva.import_epoch_status(&import_id).is_some() {
            return Ok(());
        }
        // An initial unframed V6 settlement retains the same exact publication
        // cohort and birth-proof requirements as its persisted predecessors.
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
    let source = crate::owner_lifecycle::HistorySource::new(&cva.container)
        .map_err(|error| crate::owner_lifecycle::OwnerError::Container(error.0))?;
    let births = crate::history::legacy_import_births(
        &source,
        &cva.archive,
        &cva.memories,
        cp.global_version,
        &cp.cohort,
    )?;
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
    let nodes = required_nodes(&cva.archive, cp.settlement_cut)?;
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
    let positions = nodes
        .values()
        .filter_map(|node| {
            cva.activity_position_for_turn(&node.conversation_id, &node.id)
                .map(|position| {
                    (
                        SourceKey {
                            conversation_id: node.conversation_id.clone(),
                            node_id: node.id.clone(),
                        },
                        position,
                    )
                })
        })
        .collect();
    HistoricalTimeline::from_accepted_archive(sources, &positions, cp.settlement_cut)
        .map_err(|e| fail(&e.to_string()))?;
    if corpus_digest(cva, cp.settlement_cut)? != cp.corpus_digest {
        return Err(fail(
            "import settlement corpus identity differs from its source prefix",
        ));
    }
    Ok(())
}

#[path = "historical_epoch_input.rs"]
mod epoch;
