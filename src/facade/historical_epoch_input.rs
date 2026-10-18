//! Import-epoch-scoped source, birth, and pre-start recipient proof.
use super::{AdmissionProof, HistoricalMemoryInput, Value, fail, node_binding, source_key};
use crate::{Cva, CvaError, EpisodeOrigin, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn scoped_corpus_digest(
    cva: &mut Cva,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<[u8; 32], CvaError> {
    let mut nodes = Vec::with_capacity(source_nodes.len());
    for (conversation, id) in source_nodes {
        let node = cva
            .archive
            .nodes
            .get(conversation, id)
            .ok_or_else(|| fail("missing import-scoped source Node"))?;
        let bytes = cva
            .archive
            .content_bytes(&mut cva.container, node.content_id)?;
        if crate::archive_store::hash_content(&bytes) != node.content_id {
            return Err(fail("import-scoped source content proof mismatch"));
        }
        nodes.push(node_binding(node));
    }
    Value::Array(nodes).digest("reliquary:historical-corpus:v1")
}

pub(super) fn admission_proof_for_scope(
    cva: &mut Cva,
    scope: &crate::imports::ImportEpochScope,
) -> Result<AdmissionProof, CvaError> {
    let fail = |message: &str| CvaError::Freshness(message.into());
    let owner = cva
        .owner_uuid()
        .ok_or_else(|| fail("missing import owner"))?;
    let base = cva.historical_import_base(scope)?;
    if owner != scope.start.owner_uuid {
        return Err(fail("import scope belongs to another owner"));
    }
    if scope.cohort.iter().any(|id| base.contains_key(id)) {
        return Err(fail(
            "epoch birth overlaps the pre-start recipient population",
        ));
    }

    let mut completed = BTreeSet::new();
    let mut births = BTreeMap::<MemoryId, crate::EpisodeId>::new();
    let mut publications = BTreeMap::<[u8; 32], BTreeSet<[u8; 32]>>::new();
    let mut scan = crate::Container::open_read_only_for_scan(cva.container.path())?;
    scan.visit_payloads::<CvaError>(|object, payload| {
        let location = object.legacy_bytes();
        let offset = u64::from_le_bytes(location[..8].try_into().unwrap());
        if offset < scope.marker_end {
            return Ok(());
        }
        let Some(completion) = crate::insomnia::completion::decode_completion(payload)
            .map_err(|error| fail(&error))?
        else {
            return Ok(());
        };
        if !scope.episodes.contains(&completion.episode_id.0) {
            return Ok(());
        }
        let episode = cva
            .episode(completion.episode_id)
            .ok_or_else(|| fail("scoped import completion lacks Episode"))?;
        if episode.origin != EpisodeOrigin::Import {
            return Err(fail("scoped import completion has non-import Episode"));
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
                .ok_or_else(|| fail("scoped import birth lacks first revision"))?;
            if record.source_episode_id != Some(episode.id)
                || record.global_version <= scope.start.pre.global_version
                || cva.memories.first_mutation_id(id) != Some(record.mutation_id.as_str())
            {
                return Err(fail(
                    "scoped import birth proof conflicts with Memory history",
                ));
            }
            if births
                .insert(id, episode.id)
                .is_some_and(|old| old != episode.id)
            {
                return Err(fail("conflicting scoped import birth cohort"));
            }
        }
        Ok(())
    })?;

    if births.keys().copied().collect::<BTreeSet<_>>() != scope.cohort
        || completed != scope.episodes
    {
        return Err(fail(
            "scoped import completion does not exactly cover its cohort and Episodes",
        ));
    }

    let mut episode_keys = BTreeMap::<[u8; 32], BTreeSet<(String, String)>>::new();
    let mut episode_proofs = BTreeMap::<[u8; 32], Value>::new();
    let mut covered_sources = BTreeSet::new();
    for episode in cva
        .episodes()
        .into_iter()
        .filter(|episode| scope.episodes.contains(&episode.id.0))
    {
        if episode.origin != EpisodeOrigin::Import || !completed.contains(&episode.id.0) {
            return Err(fail("scoped Episode is incomplete or non-import"));
        }
        let end = cva
            .archive
            .nodes
            .get(&episode.conversation_id, &episode.end_node_id)
            .ok_or_else(|| fail("missing scoped Episode endpoint"))?;
        if end.timestamp_ns != episode.source_through_ns {
            return Err(fail("scoped Episode endpoint timestamp conflict"));
        }
        let turns = cva.episode_turns(episode.id)?;
        if turns.is_empty()
            || turns
                .first()
                .is_none_or(|turn| turn.node_id != episode.start_node_id)
            || turns
                .last()
                .is_none_or(|turn| turn.node_id != episode.end_node_id)
        {
            return Err(fail("scoped Episode path endpoint mismatch"));
        }
        let mut keys = BTreeSet::new();
        let mut path = Vec::new();
        for turn in turns {
            let key = (episode.conversation_id.clone(), turn.node_id.clone());
            if !scope.source_nodes.contains(&key) {
                return Err(fail("scoped Episode path escapes the receipt source set"));
            }
            let node = cva
                .archive
                .nodes
                .get(&episode.conversation_id, &turn.node_id)
                .ok_or_else(|| fail("missing scoped Episode path Node"))?;
            if node.content_id != crate::archive_store::hash_content(turn.content.as_bytes()) {
                return Err(fail("scoped Episode retained content binding conflict"));
            }
            path.push(node_binding(node));
            if let Some(position) =
                cva.activity_position_for_turn(&episode.conversation_id, &turn.node_id)
            {
                keys.insert(key.clone());
                if position > scope.base_cut {
                    covered_sources.insert(key);
                }
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
    let expected_sources: BTreeSet<_> = scope
        .source_nodes
        .iter()
        .filter(|(conversation, node)| {
            cva.activity_position_for_turn(conversation, node)
                .is_some_and(|position| position > scope.base_cut)
        })
        .cloned()
        .collect();
    if covered_sources != expected_sources {
        return Err(fail(
            "scoped Episodes do not cover the import source activity",
        ));
    }

    let mut initial = BTreeMap::new();
    let mut memory_inputs = BTreeMap::new();
    let mut memories = Vec::new();
    let mut cohort_members = BTreeMap::<[u8; 32], Vec<MemoryId>>::new();
    let mut cohort_origins = BTreeMap::<[u8; 32], Value>::new();
    let mut cohort_availability = BTreeMap::<[u8; 32], Option<u64>>::new();
    let mut recipient_ids = BTreeSet::new();
    for (&id, &record) in &base {
        match cva.memory_lifecycle_state(id)? {
            crate::owner_lifecycle::LifecycleState::Active => {
                recipient_ids.insert(id);
            }
            crate::owner_lifecycle::LifecycleState::Tombstoned(_) => {
                // The base keeps terminal history. Authenticate its exclusion
                // without hydrating a body through the active-only lookup.
                let retained = cva.memory_retention_descriptor(id)?;
                memories.push(Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    ("status", Value::text("tombstoned")),
                    ("base_record", super::super::manifest::record_value(record)),
                    (
                        "tombstone_revision",
                        Value::Unsigned(retained.tombstone_revision),
                    ),
                    (
                        "tombstone_global_version",
                        Value::Unsigned(retained.tombstone_global_version),
                    ),
                    (
                        "tombstone_memory_version",
                        Value::Unsigned(retained.tombstone_memory_version),
                    ),
                    (
                        "retained_revisions",
                        Value::Array(
                            retained
                                .revisions
                                .iter()
                                .map(|revision| {
                                    Value::map([
                                        ("revision", Value::Unsigned(revision.revision)),
                                        ("body_id", Value::bytes(revision.body_id.0)),
                                        ("mutation_id", Value::text(&revision.mutation_id)),
                                        (
                                            "global_version",
                                            Value::Unsigned(revision.global_version),
                                        ),
                                        (
                                            "memory_version",
                                            Value::Unsigned(revision.memory_version),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]));
            }
        }
    }
    recipient_ids.extend(scope.cohort.iter().copied());
    let mut birth_available = BTreeMap::<[u8; 32], Option<u64>>::new();
    for &episode_id in births.values() {
        let positions = episode_keys
            .get(&episode_id.0)
            .ok_or_else(|| fail("scoped birth Episode lacks source path proof"))?;
        let available = positions
            .iter()
            .map(|(conversation, node)| {
                cva.activity_position_for_turn(conversation, node)
                    .ok_or_else(|| fail("scoped birth source lacks accepted activity"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max();
        let cohort_key = Value::map([
            ("owner_uuid", Value::bytes(owner)),
            ("episode_id", Value::bytes(episode_id.0)),
        ])
        .digest("reliquary:historical-episode-cohort:v1")?;
        if available.is_none() {
            return Err(fail("scoped import birth has no accepted source activity"));
        }
        if birth_available
            .insert(cohort_key, available)
            .is_some_and(|old| old != available)
        {
            return Err(fail("scoped import Episode has conflicting availability"));
        }
    }

    for id in recipient_ids {
        let memory = cva.memory(id)?;
        let body = cva.memory_body_id(id)?;
        if crate::memory_model::memory_body_id(&memory.title, &memory.content) != body {
            return Err(fail("scoped Memory BodyId content conflict"));
        }
        let current_revision = cva.memory_revision(id, memory.revision)?;
        if current_revision != memory {
            return Err(fail(
                "scoped Memory current revision differs from retained body",
            ));
        }
        let eligible = !memory.archived
            && memory.superseded_by.is_none()
            && matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical");
        let existing = base.get(&id);
        let born_episode = births.get(&id).copied();
        if existing.is_none() && born_episode.is_none() {
            return Err(fail("scoped recipient lacks base or import-birth evidence"));
        }
        let (cohort_key, origin, availability, necessary) = if let Some(episode_id) = born_episode {
            cva.episode(episode_id)
                .ok_or_else(|| fail("scoped birth Episode disappeared"))?;
            let key = Value::map([
                ("owner_uuid", Value::bytes(owner)),
                ("episode_id", Value::bytes(episode_id.0)),
            ])
            .digest("reliquary:historical-episode-cohort:v1")?;
            let origin = episode_proofs
                .get(&episode_id.0)
                .ok_or_else(|| fail("scoped birth proof lacks Episode evidence"))?
                .clone();
            let mut sources = episode_keys[&episode_id.0].clone();
            validate_memory_provenance(cva, &memory, Some(&episode_id), &mut sources)?;
            (key, origin, birth_available[&key], sources)
        } else {
            let base_record = existing.expect("checked above");
            let mut sources = BTreeSet::new();
            validate_memory_provenance(
                cva,
                &memory,
                memory.source_episode_id.as_ref(),
                &mut sources,
            )?;
            // Eligibility exclusions use the entire generating Episode, not
            // merely the source Node named by this individual output.
            if let Some(episode_id) = memory.source_episode_id {
                let conversation = cva
                    .episode(episode_id)
                    .ok_or_else(|| fail("missing recipient Episode"))?
                    .conversation_id
                    .clone();
                for turn in cva.episode_turns(episode_id)? {
                    sources.insert((conversation.clone(), turn.node_id));
                }
            }
            let key = Value::map([
                ("owner_uuid", Value::bytes(owner)),
                ("memory_id", Value::bytes(id.0)),
            ])
            .digest("reliquary:historical-base-recipient:v1")?;
            let origin = Value::map([
                ("memory_id", Value::bytes(id.0)),
                ("body_id", Value::bytes(body.0)),
                ("revision", Value::Unsigned(memory.revision)),
                (
                    "base_record",
                    super::super::manifest::record_value(*base_record),
                ),
            ]);
            (key, origin, base_record.admitted_at_turn, sources)
        };
        let mut record = existing.copied().unwrap_or_else(FreshnessRecord::created);
        if existing.is_none() && eligible {
            if let Some(at) = availability {
                record.admit(at).map_err(|e| fail(&e.to_string()))?;
            }
        }
        initial.insert(id, record);
        let status = if availability.is_none() {
            "unsupported"
        } else if eligible {
            "supported"
        } else {
            "credit-excluded"
        };
        memory_inputs.insert(
            id,
            HistoricalMemoryInput {
                body_id: body,
                cohort_key,
                availability,
                credit_eligible: eligible,
                necessary_sources: necessary
                    .iter()
                    .map(|(conversation_id, node_id)| {
                        crate::freshness::historical_planner::SourceKey {
                            conversation_id: conversation_id.clone(),
                            node_id: node_id.clone(),
                        }
                    })
                    .collect(),
            },
        );
        let proof_record = existing.copied().or_else(|| cva.freshness_record(id));
        let Some(proof_record) = proof_record else {
            return Err(fail("scoped Memory lacks its provisional scoring record"));
        };
        let source_ref = memory.source_ref.as_ref().map_or(Value::Null, |source| {
            Value::map([
                ("owner_id", Value::text(&source.owner_id)),
                (
                    "principal_id",
                    source
                        .principal_id
                        .as_ref()
                        .map_or(Value::Null, Value::text),
                ),
                (
                    "source_episode_id",
                    Value::bytes(source.source_episode_id.0),
                ),
                ("source_node_id", Value::text(&source.source_node_id)),
            ])
        });
        memories.push(Value::map([
            ("memory_id", Value::bytes(id.0)),
            ("body_id", Value::bytes(body.0)),
            ("revision", Value::Unsigned(memory.revision)),
            ("category", Value::text(&memory.category)),
            ("memory_type", Value::text(&memory.memory_type)),
            ("authority_kind", Value::text(&memory.authority_kind)),
            ("temporal_status", Value::text(&memory.temporal_status)),
            ("scope", Value::text(&memory.scope)),
            ("lifecycle", Value::text(&memory.lifecycle_state)),
            ("archived", Value::Bool(memory.archived)),
            (
                "superseded_by",
                memory
                    .superseded_by
                    .map_or(Value::Null, |value| Value::bytes(value.0)),
            ),
            (
                "necessary_sources",
                Value::Array(necessary.iter().map(|(c, n)| source_key(c, n)).collect()),
            ),
            (
                "source_node_id",
                memory
                    .source_node_id
                    .as_ref()
                    .map_or(Value::Null, Value::text),
            ),
            (
                "content_source",
                match (
                    &memory.content_source_conversation_id,
                    &memory.content_source_node_id,
                ) {
                    (Some(conversation), Some(node)) => source_key(conversation, node),
                    _ => Value::Null,
                },
            ),
            (
                "grounding_source",
                match (
                    &memory.grounding_source_conversation_id,
                    &memory.grounding_source_node_id,
                ) {
                    (Some(conversation), Some(node)) => source_key(conversation, node),
                    _ => Value::Null,
                },
            ),
            ("source_ref", source_ref),
            (
                "source_time_ns",
                memory.source_time_ns.map_or(Value::Null, Value::Signed),
            ),
            (
                "parent_id",
                memory
                    .parent_id
                    .map_or(Value::Null, |value| Value::bytes(value.0)),
            ),
            ("global_version", Value::Unsigned(memory.global_version)),
            ("memory_version", Value::Unsigned(memory.memory_version)),
            ("created_at_ns", Value::Signed(memory.created_at_ns)),
            ("updated_at_ns", Value::Signed(memory.updated_at_ns)),
            (
                "availability",
                availability.map_or(Value::Null, Value::Unsigned),
            ),
            ("cohort_key", Value::bytes(cohort_key)),
            ("status", Value::text(status)),
            (
                "reason",
                Value::text(if existing.is_some() {
                    "pre-start-base-recipient"
                } else {
                    "verified-import-birth"
                }),
            ),
            ("mutation_id", Value::text(&memory.mutation_id)),
            (
                "source_episode_id",
                memory
                    .source_episode_id
                    .map_or(Value::Null, |value| Value::bytes(value.0)),
            ),
            (
                "scoring_proof_digest",
                super::super::manifest::record_value(proof_record)
                    .digest("reliquary:historical-provisional-record:v1")
                    .map(Value::bytes)?,
            ),
        ]));
        if cohort_origins
            .insert(cohort_key, origin.clone())
            .is_some_and(|old| old != origin)
            || cohort_availability
                .insert(cohort_key, availability)
                .is_some_and(|old| old != availability)
        {
            return Err(fail("conflicting scoped recipient cohort proof"));
        }
        cohort_members.entry(cohort_key).or_default().push(id);
    }

    let cohorts = cohort_members
        .into_iter()
        .map(|(key, mut members)| {
            members.sort();
            Value::map([
                ("cohort_key", Value::bytes(key)),
                ("origin", cohort_origins[&key].clone()),
                (
                    "members",
                    Value::Array(members.iter().map(|id| Value::bytes(id.0)).collect()),
                ),
                (
                    "availability",
                    cohort_availability[&key].map_or(Value::Null, Value::Unsigned),
                ),
            ])
        })
        .collect();

    Ok(AdmissionProof {
        memory_inputs,
        initial,
        memories,
        cohorts,
    })
}

fn validate_memory_provenance(
    cva: &mut Cva,
    memory: &crate::Memory,
    expected_episode: Option<&crate::EpisodeId>,
    necessary: &mut BTreeSet<(String, String)>,
) -> Result<(), CvaError> {
    let fail = |message: &str| CvaError::Freshness(message.into());
    if expected_episode.is_some_and(|expected| memory.source_episode_id != Some(*expected)) {
        return Err(fail(
            "Memory birth Episode conflicts with scoped publication",
        ));
    }
    if let Some(source) = &memory.source_ref {
        if Some(&source.owner_id) != cva.owner_id().as_ref()
            || Some(source.source_episode_id) != memory.source_episode_id
            || Some(&source.source_node_id) != memory.source_node_id.as_ref()
            || source.content_source_conversation_id != memory.content_source_conversation_id
            || source.content_source_node_id != memory.content_source_node_id
            || source.grounding_source_conversation_id != memory.grounding_source_conversation_id
            || source.grounding_source_node_id != memory.grounding_source_node_id
        {
            return Err(fail("foreign or conflicting structured Memory provenance"));
        }
        let episode = cva
            .episode(source.source_episode_id)
            .cloned()
            .ok_or_else(|| fail("structured Memory source lacks Episode"))?;
        if expected_episode.is_some_and(|expected| *expected != episode.id) {
            return Err(fail(
                "Memory birth Episode conflicts with scoped publication",
            ));
        }
        let node = cva
            .archive
            .nodes
            .get(&episode.conversation_id, &source.source_node_id)
            .ok_or_else(|| fail("missing structured Memory source Node"))?;
        if source.principal_id != node.principal_id {
            return Err(fail("structured Memory source principal conflict"));
        }
    }
    if let Some(node) = &memory.source_node_id {
        let episode_id = memory
            .source_episode_id
            .ok_or_else(|| fail("declared Memory source Node lacks Episode"))?;
        let episode = cva
            .episode(episode_id)
            .cloned()
            .ok_or_else(|| fail("declared Memory source Episode is missing"))?;
        necessary.insert((episode.conversation_id, node.clone()));
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
            (Some(conversation), Some(node)) => {
                necessary.insert((conversation.clone(), node.clone()));
            }
            (None, None) => {}
            _ => return Err(fail("partial Memory provenance pair")),
        }
    }
    for (conversation, node) in necessary.iter() {
        if cva.activity_position_for_turn(conversation, node).is_none() {
            return Err(fail("Memory source lacks owner-local accepted activity"));
        }
    }
    Ok(())
}
