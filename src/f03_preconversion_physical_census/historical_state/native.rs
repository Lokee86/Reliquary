//! Proof projections obtained from the historical owner stores.
use super::row_commitment::{RowCommitment, collect};
use super::{ArtifactSink, BatchWriter, ToolResult, VerifiedCopy, lower_hex, sha256_bytes};
use crate::historical_fingerprint::HistoricalFingerprint;
use crate::historical_graph_fingerprint::GraphHistory;
use crate::owner_lifecycle::LifecycleState;
use crate::{Container, Cva, Phylactery, RewriteGenesis};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn fingerprint(value: HistoricalFingerprint) -> Value {
    json!({
        "domain": value.domain,
        "local_cut": value.local_cut.to_string(),
        "event_cut": value.event_cut.map(|n| n.to_string()),
        "rows": value.rows.to_string(),
        "sha256": lower_hex(&value.sha256),
    })
}

fn lineage(genesis: Option<&RewriteGenesis>) -> Value {
    genesis.map_or(Value::Null, |g| {
        json!({
            "destination_owner": lower_hex(&g.destination_owner),
            "generation": g.generation.to_string(),
            "predecessor_owner": lower_hex(&g.predecessor_owner),
            "predecessor_generation": g.predecessor_generation.to_string(),
            "source_head_digest": lower_hex(&g.source_head_digest),
            "policy_digest": lower_hex(&g.policy_digest),
            "created_at_unix_millis": g.created_at_unix_millis.to_string(),
        })
    })
}

struct Common<'a> {
    memories: &'a crate::memory_store::MemoryStore,
    entities: &'a crate::entity_store::EntityStore,
    relationships: &'a crate::relationship_store::RelationshipStore,
    graph: &'a crate::graph_store::GraphStore,
    ego: &'a crate::ego_store::EgoStore,
}

type Timeline = BTreeMap<&'static str, Vec<(u64, u64)>>;

fn add_terminal(history: &mut Vec<(u64, u64)>, state: LifecycleState) -> ToolResult<()> {
    if let LifecycleState::Tombstoned(coordinates) = state {
        history.push((
            coordinates
                .global_version
                .ok_or("terminal record lacks a global coordinate")?,
            coordinates
                .domain_version
                .ok_or("terminal record lacks a local coordinate")?,
        ));
    }
    Ok(())
}

fn common(
    container: &mut Container,
    stores: Common<'_>,
    projections: &mut Vec<Value>,
) -> ToolResult<Timeline> {
    let Common {
        memories,
        entities,
        relationships,
        graph,
        ego,
    } = stores;
    let memory = crate::historical_memory_fingerprint::MemoryHistory::read(container, memories)?;
    projections.push(fingerprint(
        memory.at(memories.memory_version())?.fingerprint,
    ));
    projections.push(fingerprint(
        crate::historical_entity_fingerprint::entity_cut(
            container,
            entities,
            entities.entity_version(),
        )?
        .fingerprint,
    ));
    projections.push(fingerprint(
        crate::historical_relationship_fingerprint::relationship_cut(
            container,
            relationships,
            relationships.relationship_version(),
        )?
        .fingerprint,
    ));
    let history = GraphHistory::read(container, graph)?;
    let memory_graph_positions = history.memory_graph_positions();
    let memory_graph_head = *memory_graph_positions
        .last()
        .ok_or("MemoryGraph has no empty cut")?;
    projections.push(fingerprint(
        history.at(graph.graph_version(), false)?.fingerprint,
    ));
    projections.push(fingerprint(
        history.at(memory_graph_head, true)?.fingerprint,
    ));
    let ego_history = crate::historical_ego_fingerprint::EgoHistory::read(container, ego)?;
    projections.push(fingerprint(ego_history.at(ego.version())?.fingerprint));

    let mut memory_events: Vec<_> = memories
        .records()
        .iter()
        .map(|r| (r.global_version, r.memory_version))
        .collect();
    for id in memories.tombstoned_ids() {
        add_terminal(
            &mut memory_events,
            memories
                .lifecycle_state(id)
                .map_err(|_| "invalid Memory lifecycle")?,
        )?;
    }
    let mut entity_events: Vec<_> = entities
        .records()
        .iter()
        .map(|r| (r.global_version, r.entity_version))
        .collect();
    for id in entities
        .records()
        .iter()
        .map(|r| r.id)
        .collect::<BTreeSet<_>>()
    {
        add_terminal(
            &mut entity_events,
            entities
                .lifecycle_state(id)
                .map_err(|_| "invalid Entity lifecycle")?,
        )?;
    }
    let mut relationship_events: Vec<_> = relationships
        .records()
        .iter()
        .map(|r| (r.global_version, r.relationship_version))
        .collect();
    for id in relationships
        .records()
        .iter()
        .map(|r| r.id)
        .collect::<BTreeSet<_>>()
    {
        add_terminal(
            &mut relationship_events,
            relationships
                .lifecycle_state(id)
                .map_err(|_| "invalid Relationship lifecycle")?,
        )?;
    }
    let graph_events: Vec<_> = graph
        .transaction_global_versions()
        .iter()
        .enumerate()
        .map(|(i, &global)| (global, i as u64 + 1))
        .collect();
    let memory_graph_events = memory_graph_positions
        .into_iter()
        .skip(1)
        .map(|local| (graph_events[(local - 1) as usize].0, local))
        .collect();
    Ok(BTreeMap::from([
        ("memory", memory_events),
        ("entity", entity_events),
        ("relationship", relationship_events),
        ("graph", graph_events),
        ("memory_graph", memory_graph_events),
    ]))
}

fn emit_global(
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
    container: &Container,
    mut timeline: Timeline,
) -> ToolResult<Value> {
    for values in timeline.values_mut() {
        values.sort_unstable();
        if values
            .iter()
            .any(|(g, _)| *g == 0 || *g > container.latest_version())
        {
            return Err("native timeline event exceeds its global bounds".into());
        }
    }
    let mut stream = BatchWriter::new(sink, copy, "global");
    let mut next: BTreeMap<_, _> = timeline
        .into_iter()
        .map(|(name, events)| (name, (events.into_iter().peekable(), 0_u64)))
        .collect();
    for global in 0..=container.latest_version() {
        let mut versions = BTreeMap::new();
        for (&name, (events, local)) in &mut next {
            while let Some(&(_, version)) = events.next_if(|(g, _)| *g <= global).as_ref() {
                *local = (*local).max(version);
            }
            versions.insert(name, local.to_string());
        }
        stream.push(json!({
            "global_version": global.to_string(),
            "domain_versions": versions,
            "transaction_time_ns": container.transaction_time_ns(global).map(|v| v.to_string()),
        }))?;
    }
    stream.finish()
}

fn freshness(owner: &Cva, copy: &VerifiedCopy, sink: &mut ArtifactSink<'_>) -> ToolResult<Value> {
    use crate::freshness_storage::FreshnessReplayEntry;
    use crate::historical_freshness_fingerprint::freshness_at_event;
    let turn_head = owner.rel_turn_count();

    let mut cuts = BTreeMap::new();
    for entry in owner
        .freshness
        .retained_history_entries()
        .map_err(|_| "accepted Freshness journal could not be read")?
    {
        if let FreshnessReplayEntry::Event {
            event_id,
            source_turn,
            proof,
            ..
        } = &entry
        {
            if *source_turn > turn_head || proof.graph_version > owner.memory_graph_version() {
                return Err("accepted Freshness event exceeds retained heads".into());
            }
            let key = (*source_turn, event_id.clone());
            if let Some(existing) = cuts.insert(key, entry.clone()) {
                if existing != entry {
                    return Err("conflicting accepted Freshness event identity".into());
                }
            }
        }
    }

    let mut stream = BatchWriter::new(sink, copy, "freshness");
    for settlement in owner.freshness.import_settlements() {
        let bytes = settlement
            .try_encode()
            .map_err(|_| "accepted FRS07 authority could not be encoded")?;
        let semantic = settlement
            .semantic
            .as_ref()
            .ok_or("accepted FRS07 scope is unavailable")?;
        stream.push(json!({
            "kind": "settlement",
            "import_id": lower_hex(&settlement.import_id),
            "base_activity": settlement.base_cut.to_string(),
            "settlement_activity": settlement.settlement_cut.to_string(),
            "global_version": settlement.global_version.to_string(),
            "archive_version": settlement.archive_version.to_string(),
            "graph_version": settlement.graph_version.to_string(),
            "memory_graph_version": settlement.memory_graph_version.to_string(),
            "authority_sha256": sha256_bytes(&bytes),
            "settled_record_count": settlement.records.len().to_string(),
            "accepted_events": semantic.accepted_events.len().to_string(),
        }))?;
    }

    for ((activity, event_id), entry) in cuts {
        let FreshnessReplayEntry::Event { proof, .. } = entry else {
            return Err("non-event reached the Freshness event catalog".into());
        };
        let cut = crate::FreshnessEventCut {
            rel_turn: activity,
            event_id_inclusive: Some(event_id.clone()),
            graph_version: proof.graph_version,
        };
        let state = match freshness_at_event(owner, &cut) {
            Ok(state) => {
                json!({"availability": "supported", "proof": fingerprint(state.fingerprint)})
            }
            Err(error)
                if error
                    == "import settlement cut does not retain an intra-settlement trajectory" =>
            {
                json!({"availability": "unavailable", "reason": "intra_settlement_trajectory_not_retained"})
            }
            Err(_) => {
                return Err("accepted Freshness event cut failed for an unknown reason".into());
            }
        };
        stream.push(json!({
            "kind": "accepted_event_cut",
            "activity": activity.to_string(),
            "event_id_sha256": sha256_bytes(event_id.as_bytes()),
            "memory_graph_version": proof.graph_version.to_string(),
            "state": state,
        }))?;
    }
    stream.finish()
}

fn emit_records(
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
    rows: Vec<RowCommitment>,
) -> ToolResult<Value> {
    let mut stream = BatchWriter::new(sink, copy, "records");
    let mut seen = BTreeSet::new();
    for row in rows {
        if !seen.insert((row.domain.clone(), row.family.clone(), row.key_sha256)) {
            return Err("duplicate retained native record commitment".into());
        }
        stream.push(json!({
            "projection_domain": row.domain,
            "family": row.family,
            "stable_key_sha256": lower_hex(&row.key_sha256),
            "canonical_value_sha256": lower_hex(&row.value_sha256),
            "canonical_byte_length": row.canonical_bytes.to_string(),
            "origin_coordinate": row.coordinate,
            "availability": row.availability,
        }))?;
    }
    stream.finish()
}

fn archive_origins(owner: &mut Cva) -> ToolResult<HistoricalFingerprint> {
    let uuid = owner
        .owner_uuid()
        .ok_or("Archive origins need typed owner UUID")?;
    let mut proof = crate::historical_fingerprint::OwnerProjection::new(
        uuid,
        "archive_publication_origins",
        owner.archive_version(),
        None,
    )?;
    for version in owner.archive.record_versions().to_vec() {
        let (_, applying) = owner
            .archive
            .read_publication(&mut owner.container, version)
            .map_err(|_| "Archive first-applying origin could not be resolved")?;
        proof.insert(
            "publication_origin",
            version.archive_version.to_be_bytes(),
            json!({
                "archive_version": version.archive_version,
                "global_version": version.global_version,
                "first_applying_archive_version": applying.archive_version,
                "first_applying_global_version": applying.global_version,
                "rel_turn": owner.archive.activity_cut_at_archive_version(version.archive_version)
                    .map_err(|_| "Archive origin activity coordinate could not be resolved")?,
            }),
        )?;
    }
    proof.fingerprint()
}

pub(super) fn rel(
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
    container: Container,
) -> ToolResult<Value> {
    let mut owner = Cva::open_from_container(container)
        .map_err(|_| "REL native read-only replay rejected the source")?;
    owner.freshness = owner
        .freshness
        .with_cached_oracle_journal()
        .map_err(|_| "accepted Freshness journal could not be cached")?;
    let ((mut timeline, projections), rows) = collect(|| {
        let mut projections = Vec::new();
        let timeline = common(
            &mut owner.container,
            Common {
                memories: &owner.memories,
                entities: &owner.entities,
                relationships: &owner.relationships,
                graph: &owner.graph,
                ego: &owner.ego,
            },
            &mut projections,
        )
        .map_err(|e| format!("common history: {e}"))?;

        let archive_head = owner.archive_version();
        let vector_head = owner.vector_version();
        let archive = crate::historical_archive_fingerprint::archive_cut(&mut owner, archive_head)
            .map_err(|_| "native Archive historical projection failed")?;
        projections.push(fingerprint(archive.fingerprint));
        projections.push(fingerprint(archive_origins(&mut owner)?));
        let vector =
            crate::historical_vector_generation_fingerprint::VectorHistory::read(&mut owner)
                .and_then(|h| h.at(vector_head))
                .map_err(|_| "native vector generation proof failed")?;
        projections.push(fingerprint(vector.fingerprint));
        projections.push(fingerprint(
            crate::historical_dream_pair_fingerprint::reliquary_dream_pairs(&owner)?.fingerprint,
        ));
        projections.push(fingerprint(
            crate::historical_dream_processing_fingerprint::reliquary_dream_processing(&owner)?
                .fingerprint,
        ));
        projections.push(fingerprint(
            crate::historical_dream_proof_fingerprint::dream_proof_state(&owner)
                .map_err(|_| "native Dream proof replay failed")?,
        ));

        projections.push(fingerprint(
            crate::historical_freshness_fingerprint::freshness_at_turn(
                &owner,
                owner.rel_turn_count(),
            )
            .map_err(|_| "historical Freshness head proof failed")?
            .fingerprint,
        ));
        projections.push(fingerprint(super::vector_objects::observe(
            &owner.container,
            &owner.packed_vectors,
            &owner.memory_vectors,
            &owner.compatibility_profiles,
            Some(&owner.archive_vectors),
        )?));
        projections.push(fingerprint(super::project::observe(&owner)?));
        projections.push(fingerprint(super::insomnia::observe(&owner)?));
        projections.push(fingerprint(super::clock_neutral::observe(
            &owner.container,
        )?));
        Ok((timeline, projections))
    })?;
    let records = emit_records(copy, sink, rows)?;
    let archive_events = owner
        .archive
        .record_versions()
        .iter()
        .map(|r| (r.global_version, r.archive_version))
        .collect::<Vec<_>>();
    let mut activity_events = Vec::with_capacity(archive_events.len());
    for &(global, local) in &archive_events {
        activity_events.push((
            global,
            owner
                .archive
                .activity_cut_at_archive_version(local)
                .map_err(|_| "archive activity cut is unavailable")?,
        ));
    }
    timeline.insert("archive", archive_events);
    timeline.insert("rel_activity", activity_events);
    timeline.insert(
        "vector_generation",
        owner
            .vector_generations
            .generations()
            .iter()
            .map(|r| (r.global_version, r.vector_version))
            .collect(),
    );
    let global = emit_global(copy, sink, &owner.container, timeline)?;
    let freshness = freshness(&owner, copy, sink)?;
    let metadata = owner.rel_metadata();
    let type_sha256 = metadata
        .type_label
        .map(|value| sha256_bytes(value.as_bytes()));
    let metadata_dependencies: Vec<_> = metadata
        .dependencies
        .iter()
        .map(|value| sha256_bytes(value.as_bytes()))
        .collect();
    Ok(json!({
        "owner_kind": "reliquary",
        "history_generation": owner.history_generation().to_string(),
        "rewrite_genesis": lineage(owner.rewrite_genesis()),
        "metadata_type_sha256": type_sha256,
        "metadata_dependencies_sha256": metadata_dependencies,
        "rel_activity_head": owner.rel_turn_count().to_string(),
        "global_stream": global,
        "record_stream": records,
        "freshness_stream": freshness,
        "native_history_projections": projections,
    }))
}

pub(super) fn phy(
    copy: &VerifiedCopy,
    sink: &mut ArtifactSink<'_>,
    container: Container,
) -> ToolResult<Value> {
    let mut owner = Phylactery::open_from_container(container)
        .map_err(|_| "PHY native read-only replay rejected the source")?;
    let ((timeline, projections), rows) = collect(|| {
        let mut projections = Vec::new();
        let timeline = common(
            &mut owner.container,
            Common {
                memories: &owner.memories,
                entities: &owner.entities,
                relationships: &owner.relationships,
                graph: &owner.graph,
                ego: &owner.ego,
            },
            &mut projections,
        )
        .map_err(|e| format!("common history: {e}"))?;
        projections.push(fingerprint(
            crate::historical_dream_pair_fingerprint::phylactery_dream_pairs(&owner)?.fingerprint,
        ));
        projections.push(fingerprint(
            crate::historical_dream_processing_fingerprint::phylactery_dream_processing(&owner)?
                .fingerprint,
        ));
        projections.push(fingerprint(super::vector_objects::observe(
            &owner.container,
            &owner.packed_vectors,
            &owner.memory_vectors,
            &owner.compatibility_profiles,
            None,
        )?));
        projections.push(fingerprint(super::clock_neutral::observe(
            &owner.container,
        )?));
        Ok((timeline, projections))
    })?;
    let records = emit_records(copy, sink, rows)?;
    let global = emit_global(copy, sink, &owner.container, timeline)?;
    Ok(json!({
        "owner_kind": "phylactery",
        "history_generation": owner.history_generation().to_string(),
        "rewrite_genesis": lineage(owner.rewrite_genesis()),
        "global_stream": global,
        "record_stream": records,
        "freshness_stream": {"availability": "unsupported", "reason": "REL_only"},
        "native_history_projections": projections,
    }))
}
