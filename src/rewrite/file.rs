use super::{
    FreshnessRewriteMode, IdentityPolicy, RewriteError, RewritePlan, RewriteReport, RewriteRequest,
};
use crate::{
    Container, Cva, FileKind, HistoryGenerationId, Memory, MemoryDraft, MemoryId, Phylactery,
};
use std::{collections::BTreeSet, path::Path};
use uuid::Uuid;

pub fn plan_rewrite_file(
    source: impl AsRef<Path>,
    request: &RewriteRequest,
) -> Result<RewritePlan, RewriteError> {
    let source = source.as_ref();
    let container =
        Container::open_read_only(source).map_err(|error| RewriteError(error.to_string()))?;
    match container.identity().map(|identity| identity.file_kind) {
        Some(FileKind::Reliquary) | None => {
            let mut rel =
                Cva::open_read_only(source).map_err(|error| RewriteError(error.to_string()))?;
            rel.plan_rewrite(request)
        }
        Some(FileKind::Phylactery) => {
            let mut phy = Phylactery::open_read_only(source)
                .map_err(|error| RewriteError(error.to_string()))?;
            phy.plan_rewrite(request)
        }
    }
}

pub fn rewrite_file(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    request: &RewriteRequest,
) -> Result<RewriteReport, RewriteError> {
    let plan = plan_rewrite_file(source.as_ref(), request)?;
    rewrite_from_plan(source, output, &plan)
}

pub fn rewrite_from_plan(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    plan: &RewritePlan,
) -> Result<RewriteReport, RewriteError> {
    let source = source.as_ref();
    let output = output.as_ref();
    // Hold coherent source evidence through emission and publication. A mounted
    // writer is refused rather than raced or borrowed as read authority.
    let mut source_guard =
        Container::open_read_only(source).map_err(|error| RewriteError(error.to_string()))?;
    source_guard
        .validate_path_identity()
        .map_err(|error| RewriteError(error.to_string()))?;
    if source_guard
        .backing_file_identity()
        .map_err(|error| RewriteError(error.to_string()))?
        != Some(plan.source_file_identity)
    {
        return Err(RewriteError(
            "source file was replaced after rewrite planning".into(),
        ));
    }
    if !output.is_file() && output.exists() {
        return Err(RewriteError("rewrite output must be a file path".into()));
    }
    if output.exists() {
        return Err(RewriteError("rewrite output already exists".into()));
    }
    let source_abs = source
        .canonicalize()
        .map_err(|e| RewriteError(e.to_string()))?;
    let output_abs = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| RewriteError(e.to_string()))?
            .join(output)
    };
    if source_abs == output_abs {
        return Err(RewriteError("rewrite source and output must differ".into()));
    }
    let actual_head = source_guard
        .physical_digest()
        .map_err(|error| RewriteError(error.to_string()))?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if actual_head != plan.source_head_digest {
        return Err(RewriteError("source changed after rewrite planning".into()));
    }
    let expected_plan = plan_rewrite_file(source, &plan.request)?;
    if expected_plan != *plan {
        return Err(RewriteError(
            "frozen rewrite plan differs from canonical source selection".into(),
        ));
    }
    let identity = Container::open_read_only(source)
        .map_err(|e| RewriteError(e.to_string()))?
        .identity()
        .map(|identity| identity.file_kind);
    let request = &plan.request;
    let generation = HistoryGenerationId::from_bytes(*Uuid::new_v4().as_bytes());
    let source_owner = plan
        .source_owner_uuid
        .as_deref()
        .map(parse_uuid_hex)
        .transpose()?;
    let destination_owner = match request.identity {
        IdentityPolicy::PreserveOwner => source_owner.unwrap_or_else(|| *Uuid::new_v4().as_bytes()),
        IdentityPolicy::NewOwner => *Uuid::new_v4().as_bytes(),
    };
    let temp_path = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".rewrite-{}.tmp", Uuid::new_v4()));
    let mut staging_guard = None;
    let result = match identity {
        Some(FileKind::Phylactery) => emit_phy(
            source,
            &temp_path,
            request,
            plan,
            destination_owner,
            generation,
            &mut staging_guard,
        ),
        Some(FileKind::Reliquary) | None => emit_rel(
            source,
            &temp_path,
            request,
            plan,
            destination_owner,
            generation,
            &mut staging_guard,
        ),
    };
    let container = match result {
        Ok(container) => container,
        Err(error) => {
            remove_owned_staging(&temp_path, staging_guard.as_ref());
            return Err(error);
        }
    };
    let publication_guard = match validate_emitted(container, plan, destination_owner, generation) {
        Ok(container) => container,
        Err(error) => {
            remove_owned_staging(&temp_path, staging_guard.as_ref());
            return Err(error);
        }
    };
    let final_head = source_guard.physical_digest().map_err(|error| {
        remove_owned_staging(&temp_path, staging_guard.as_ref());
        RewriteError(error.to_string())
    })?;
    let final_head = final_head
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if final_head != plan.source_head_digest {
        remove_owned_staging(&temp_path, staging_guard.as_ref());
        return Err(RewriteError(
            "source changed during rewrite emission".into(),
        ));
    }
    source_guard.validate_path_identity().map_err(|error| {
        remove_owned_staging(&temp_path, staging_guard.as_ref());
        RewriteError(error.to_string())
    })?;
    // Retain the verified output's writer lease across atomic no-clobber
    // publication. Hard-link creation refuses any existing destination entry;
    // keeping the staging file in the destination directory avoids cross-volume
    // moves. Filesystems without hard links fail closed.
    publication_guard
        .validate_path_identity()
        .map_err(|error| RewriteError(error.to_string()))?;
    std::fs::hard_link(&temp_path, output).map_err(|error| {
        remove_owned_staging(&temp_path, staging_guard.as_ref());
        RewriteError(format!("publish rewritten output: {error}"))
    })?;
    publication_guard
        .validate_path_identity()
        .map_err(|error| RewriteError(error.to_string()))?;
    publication_guard
        .validate_alias_identity(output)
        .map_err(|error| RewriteError(format!("rewritten output identity changed: {error}")))?;
    std::fs::remove_file(&temp_path)
        .map_err(|error| RewriteError(format!("remove rewrite staging link: {error}")))?;
    drop(publication_guard);
    drop(staging_guard);
    #[cfg(unix)]
    std::fs::File::open(output.parent().unwrap_or_else(|| Path::new(".")))
        .and_then(|directory| directory.sync_all())
        .map_err(|error| RewriteError(format!("sync rewrite publication directory: {error}")))?;
    Ok(RewriteReport {
        plan: plan.clone(),
        output_path: output.to_string_lossy().into_owned(),
        destination_owner_uuid: hex16(&destination_owner),
        destination_generation: generation.to_string(),
    })
}

fn remove_owned_staging(path: &Path, guard: Option<&crate::container::PhysicalReadSource>) {
    if guard.is_some_and(|guard| guard.validate_path_identity().is_ok()) {
        let _ = std::fs::remove_file(path);
    }
}

fn validate_emitted(
    probe: Container,
    plan: &RewritePlan,
    owner: [u8; 16],
    generation: HistoryGenerationId,
) -> Result<Container, RewriteError> {
    if probe.owner_uuid() != Some(owner) {
        return Err(RewriteError(
            "rewritten owner identity did not validate".into(),
        ));
    }
    match probe.identity().map(|identity| identity.file_kind) {
        Some(FileKind::Phylactery) => {
            let phy =
                Phylactery::open_from_container(probe).map_err(|e| RewriteError(e.to_string()))?;
            let expected_active = plan
                .memory_decisions
                .iter()
                .filter(|decision| decision.retain && !decision.tombstoned)
                .count();
            let expected_revisions: usize = plan
                .selected_revisions
                .iter()
                .map(|selection| selection.source_revisions.len())
                .sum();
            if phy.history_generation() != generation
                || phy.memory_ids().len() != expected_active
                || phy.memory_stats().revisions != expected_revisions
            {
                return Err(RewriteError(
                    "rewritten PHY failed semantic reopen validation".into(),
                ));
            }
            Ok(phy.container)
        }
        Some(FileKind::Reliquary) | None => {
            let rel = Cva::open_from_container(probe).map_err(|e| RewriteError(e.to_string()))?;
            let expected_active = plan
                .memory_decisions
                .iter()
                .filter(|decision| decision.retain && !decision.tombstoned)
                .count();
            let expected_revisions: usize = plan
                .selected_revisions
                .iter()
                .map(|selection| selection.source_revisions.len())
                .sum();
            if rel.history_generation() != generation
                || rel.memory_ids().len() != expected_active
                || rel.memory_stats().revisions != expected_revisions
            {
                return Err(RewriteError(
                    "rewritten REL failed semantic reopen validation".into(),
                ));
            }
            Ok(rel.container)
        }
    }
}

fn emit_rel(
    source_path: &Path,
    output_path: &Path,
    request: &RewriteRequest,
    plan: &RewritePlan,
    destination_owner: [u8; 16],
    generation: HistoryGenerationId,
    staging_guard: &mut Option<crate::container::PhysicalReadSource>,
) -> Result<Container, RewriteError> {
    let mut source = Cva::open_read_only(source_path).map_err(|e| RewriteError(e.to_string()))?;
    let keep = retained_ids(plan)?;
    let semantic = crate::rewrite::semantic_state::semantic_closure_rel(&source, &keep)?;
    let revision_sets = load_selected_revisions_rel(&mut source, plan)?;
    let selected: Vec<_> = revision_sets
        .iter()
        .map(|(_, _, revisions)| revisions.last().unwrap().clone())
        .collect();
    let all_selected: Vec<_> = revision_sets
        .iter()
        .flat_map(|(_, _, revisions)| revisions.iter().cloned())
        .collect();
    let metadata = source.rel_metadata();
    let source_cut = source.rel_turn_count();
    let source_policy = source.freshness_policy();
    let mut effective = Vec::new();
    if request.freshness == FreshnessRewriteMode::PreserveEffectiveCurrent {
        for memory in &selected {
            let source_record = source
                .freshness
                .records()
                .get(&memory.id)
                .copied()
                .unwrap_or_else(crate::FreshnessRecord::created);
            let source_carry = source.freshness.initial_decay_carry(memory.id);
            effective.push((memory.id, source_record, source_carry));
        }
    }
    let mut output =
        Cva::create_rel_with_uuid(output_path, destination_owner, metadata.type_label.clone())
            .map_err(|e| RewriteError(e.to_string()))?;
    *staging_guard = Some(
        output
            .container
            .read_source()
            .map_err(|error| RewriteError(error.to_string()))?,
    );
    append_genesis(
        &mut output.container,
        destination_owner,
        source_owner_from_plan(plan)?,
        generation,
        source_generation(plan)?,
        plan,
    )?;
    output
        .set_rel_metadata(metadata.type_label, metadata.dependencies)
        .map_err(|e| RewriteError(e.to_string()))?;
    let archive_memories = if request.provenance == super::ProvenancePolicy::Strip {
        &[][..]
    } else {
        all_selected.as_slice()
    };
    crate::cva_reconcile_archive::replay_rewrite_archive(
        &mut source,
        &mut output,
        archive_memories,
        &request.archive,
    )
    .map_err(|e| RewriteError(e.to_string()))?;
    let destination_cut = output.rel_turn_count();
    let baseline_source_cut = if request.freshness == FreshnessRewriteMode::RecomputeRetainedHistory
    {
        effective = recompute_retained_freshness(&source, &output, &selected)?;
        destination_cut
    } else {
        source_cut
    };
    output
        .freshness
        .set_policy_before_enrollment(source_policy)
        .map_err(RewriteError)?;

    emit_memories_rel(
        &mut output,
        revision_sets,
        &effective,
        request.freshness.clone(),
        request.provenance == super::ProvenancePolicy::Strip,
        generation,
        baseline_source_cut,
        destination_cut,
        parse_digest(&plan.policy_digest)?,
    )?;
    crate::cva_reconcile_archive::replay_rewrite_file_links(&source, &mut output, &keep)
        .map_err(|error| RewriteError(error.to_string()))?;
    copy_compact_profiles_rel(&mut source, &mut output, &all_selected, plan)?;
    let retained_graph_memories: Vec<_> = source
        .graph
        .node_memory_ids()
        .into_iter()
        .filter(|id| keep.contains(id))
        .collect();
    output
        .graph
        .publish_rewrite_baseline(
            &mut output.container,
            destination_owner,
            generation,
            parse_digest(&plan.policy_digest)?,
            &retained_graph_memories,
        )
        .map_err(|e| RewriteError(e.to_string()))?;
    crate::rewrite::semantic_state::copy_semantic_state_rel(
        &source,
        &mut output,
        &semantic,
        request.include_rel_ego_synthesis,
    )?;
    output.sync().map_err(|e| RewriteError(e.to_string()))?;
    Ok(output.container)
}

fn emit_phy(
    source_path: &Path,
    output_path: &Path,
    request: &RewriteRequest,
    plan: &RewritePlan,
    destination_owner: [u8; 16],
    generation: HistoryGenerationId,
    staging_guard: &mut Option<crate::container::PhysicalReadSource>,
) -> Result<Container, RewriteError> {
    if request.memories.minimum_effective_freshness.is_some() {
        return Err(RewriteError(
            "Phylactery has no REL Freshness clock to preserve".into(),
        ));
    }
    let mut source =
        Phylactery::open_read_only(source_path).map_err(|e| RewriteError(e.to_string()))?;
    let keep = retained_ids(plan)?;
    let semantic = crate::rewrite::semantic_state::semantic_closure_phy(&source, &keep)?;
    let revision_sets = load_selected_revisions_phy(&mut source, plan)?;
    let all_selected: Vec<_> = revision_sets
        .iter()
        .flat_map(|(_, _, revisions)| revisions.iter().cloned())
        .collect();
    let profile = source.profile();
    let mut output = Phylactery::create_with_uuid(output_path, destination_owner)
        .map_err(|e| RewriteError(e.to_string()))?;
    *staging_guard = Some(
        output
            .container
            .read_source()
            .map_err(|error| RewriteError(error.to_string()))?,
    );
    append_genesis(
        &mut output.container,
        destination_owner,
        source_owner_from_plan(plan)?,
        generation,
        source_generation(plan)?,
        plan,
    )?;
    output
        .set_profile(profile.display_name)
        .map_err(|e| RewriteError(e.to_string()))?;

    for (id, tombstoned, revisions) in revision_sets {
        let routing = revisions
            .last()
            .and_then(|memory| memory.routing_metadata.clone());
        for memory in revisions {
            let expected = output
                .memory(id)
                .map(|current| current.revision)
                .unwrap_or(0);
            let strip = request.provenance == super::ProvenancePolicy::Strip;
            output
                .memories
                .publish_with_source_ref_and_temporal_inference(
                    &mut output.container,
                    Some(id),
                    expected,
                    memory_draft(&memory, strip),
                    if strip {
                        None
                    } else {
                        memory.source_ref.clone()
                    },
                    memory.temporal_inference.clone(),
                )
                .map_err(|e| RewriteError(e.to_string()))?;
        }
        if let Some(metadata) = routing {
            output
                .put_memory_routing_metadata(metadata)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
        if tombstoned {
            let revision = output
                .memory(id)
                .map_err(|e| RewriteError(e.to_string()))?
                .revision;
            output
                .tombstone_memory(id, revision)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
    }
    copy_compact_profiles_phy(&mut source, &mut output, &all_selected, plan)?;
    let retained_graph_memories: Vec<_> = source
        .graph
        .node_memory_ids()
        .into_iter()
        .filter(|id| keep.contains(id))
        .collect();
    output
        .graph
        .publish_rewrite_baseline(
            &mut output.container,
            destination_owner,
            generation,
            parse_digest(&plan.policy_digest)?,
            &retained_graph_memories,
        )
        .map_err(|e| RewriteError(e.to_string()))?;
    crate::rewrite::semantic_state::copy_semantic_state_phy(&source, &mut output, &semantic)?;
    output.sync().map_err(|e| RewriteError(e.to_string()))?;
    let _ = generation;
    Ok(output.container)
}

fn copy_compact_profiles_rel(
    source: &mut Cva,
    output: &mut Cva,
    selected: &[Memory],
    plan: &RewritePlan,
) -> Result<(), RewriteError> {
    for profile in source.compatibility_profiles() {
        if !plan.selected_profiles.contains(&hex32(&profile.id.0)) {
            continue;
        }
        let retired = source.compatibility_profile_is_retired(profile.id);
        output
            .compatibility_profiles
            .put(&mut output.container, profile.clone())
            .map_err(|e| RewriteError(e.to_string()))?;
        if let Some((body_ids, schema, bytes)) =
            compact_memory_rows_rel(source, selected, profile.id)?
        {
            let packed = crate::PackedVectors::from_bytes(schema, bytes)
                .map_err(|e| RewriteError(e.to_string()))?;
            let packed_id = output
                .put_packed_vectors(packed)
                .map_err(|e| RewriteError(e.to_string()))?;
            output
                .put_memory_vectors(profile.id, packed_id, body_ids)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
        if !retired {
            let retained_fragments = output
                .fragments()
                .into_iter()
                .map(|fragment| fragment.id)
                .collect::<std::collections::HashSet<_>>();
            if let Some((fragment_ids, schema, bytes)) =
                compact_archive_rows_rel(source, &retained_fragments, profile.id)?
            {
                let packed = crate::PackedVectors::from_bytes(schema, bytes)
                    .map_err(|e| RewriteError(e.to_string()))?;
                let packed_id = output
                    .put_packed_vectors(packed)
                    .map_err(|e| RewriteError(e.to_string()))?;
                let archive_id = output
                    .put_archive_vectors(packed_id, fragment_ids)
                    .map_err(|e| RewriteError(e.to_string()))?;
                let archive_version = output.archive_version();
                output
                    .publish_vector_generation_without_live_semantic(
                        profile.id,
                        archive_id,
                        archive_version,
                    )
                    .map_err(|e| RewriteError(e.to_string()))?;
            }
        }
        if retired {
            output
                .compatibility_profiles
                .retire(&mut output.container, profile.id)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
    }
    Ok(())
}

fn compact_memory_rows_rel(
    source: &mut Cva,
    selected: &[Memory],
    profile: crate::CompatibilityProfileId,
) -> Result<Option<(Vec<crate::MemoryBodyId>, crate::VectorSchema, Vec<u8>)>, RewriteError> {
    let mut body_ids = Vec::new();
    let mut bytes = Vec::new();
    let mut schema: Option<crate::VectorSchema> = None;
    let mut seen = std::collections::HashSet::new();
    for memory in selected {
        let body = crate::memory_model::memory_body_id(&memory.title, &memory.content);
        if !seen.insert(body) {
            continue;
        }
        let location = source
            .memory_vector_location(profile, body)
            .ok_or_else(|| {
                RewriteError(format!(
                    "profile {} lost planned coverage",
                    hex32(&profile.0)
                ))
            })?;
        let set = source
            .memory_vectors(location.set_id)
            .map_err(|e| RewriteError(e.to_string()))?;
        if set.compatibility_profile_id != profile
            || set.memory_body_ids.get(location.row as usize) != Some(&body)
        {
            return Err(RewriteError("Memory-vector binding is inconsistent".into()));
        }
        let packed = source
            .packed_vectors(set.packed_vector_id)
            .map_err(|e| RewriteError(e.to_string()))?;
        let row = packed
            .row(location.row as usize)
            .ok_or_else(|| RewriteError("Memory-vector row is missing".into()))?;
        if schema.as_ref().is_some_and(|old| *old != packed.schema()) {
            return Err(RewriteError(
                "selected profile rows use inconsistent vector schemas".into(),
            ));
        }
        schema.get_or_insert_with(|| packed.schema().clone());
        bytes.extend_from_slice(row);
        body_ids.push(body);
    }
    Ok(schema.map(|schema| (body_ids, schema, bytes)))
}

fn compact_archive_rows_rel(
    source: &mut Cva,
    retained_fragments: &std::collections::HashSet<crate::FragmentId>,
    profile: crate::CompatibilityProfileId,
) -> Result<Option<(Vec<crate::FragmentId>, crate::VectorSchema, Vec<u8>)>, RewriteError> {
    if retained_fragments.is_empty() {
        return Ok(None);
    }
    let generation = source.current_vector_generation(profile).ok_or_else(|| {
        RewriteError(format!(
            "selected active profile {} has no complete current Archive vector generation",
            hex32(&profile.0)
        ))
    })?;
    let set = source
        .archive_vectors(generation.archive_vector_id)
        .map_err(|e| RewriteError(e.to_string()))?;
    let packed = source
        .packed_vectors(set.packed_vector_id)
        .map_err(|e| RewriteError(e.to_string()))?;
    let schema = packed.schema().clone();
    let mut fragment_ids = Vec::with_capacity(retained_fragments.len());
    let mut bytes = Vec::new();
    for (row, fragment_id) in set.fragment_ids.iter().copied().enumerate() {
        if !retained_fragments.contains(&fragment_id) {
            continue;
        }
        let vector = packed
            .row(row)
            .ok_or_else(|| RewriteError("Archive-vector row is missing".into()))?;
        fragment_ids.push(fragment_id);
        bytes.extend_from_slice(vector);
    }
    if fragment_ids.len() != retained_fragments.len() {
        return Err(RewriteError(format!(
            "selected profile {} lost planned Archive-vector coverage: expected {} fragments, found {}",
            hex32(&profile.0),
            retained_fragments.len(),
            fragment_ids.len()
        )));
    }
    Ok(Some((fragment_ids, schema, bytes)))
}

fn copy_compact_profiles_phy(
    source: &mut Phylactery,
    output: &mut Phylactery,
    selected: &[Memory],
    plan: &RewritePlan,
) -> Result<(), RewriteError> {
    for profile in source.compatibility_profiles() {
        if !plan.selected_profiles.contains(&hex32(&profile.id.0)) {
            continue;
        }
        let retired = source.compatibility_profile_is_retired(profile.id);
        output
            .compatibility_profiles
            .put(&mut output.container, profile.clone())
            .map_err(|e| RewriteError(e.to_string()))?;
        if let Some((body_ids, schema, bytes)) =
            compact_memory_rows_phy(source, selected, profile.id)?
        {
            let packed = crate::PackedVectors::from_bytes(schema, bytes)
                .map_err(|e| RewriteError(e.to_string()))?;
            let packed_id = output
                .put_packed_vectors(packed)
                .map_err(|e| RewriteError(e.to_string()))?;
            output
                .put_memory_vectors(profile.id, packed_id, body_ids)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
        if retired {
            output
                .compatibility_profiles
                .retire(&mut output.container, profile.id)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
    }
    Ok(())
}

fn compact_memory_rows_phy(
    source: &mut Phylactery,
    selected: &[Memory],
    profile: crate::CompatibilityProfileId,
) -> Result<Option<(Vec<crate::MemoryBodyId>, crate::VectorSchema, Vec<u8>)>, RewriteError> {
    let mut body_ids = Vec::new();
    let mut bytes = Vec::new();
    let mut schema: Option<crate::VectorSchema> = None;
    let mut seen = std::collections::HashSet::new();
    for memory in selected {
        let body = crate::memory_model::memory_body_id(&memory.title, &memory.content);
        if !seen.insert(body) {
            continue;
        }
        let location = source
            .memory_vector_location(profile, body)
            .ok_or_else(|| {
                RewriteError(format!(
                    "profile {} lost planned coverage",
                    hex32(&profile.0)
                ))
            })?;
        let set = source
            .memory_vectors(location.set_id)
            .map_err(|e| RewriteError(e.to_string()))?;
        if set.compatibility_profile_id != profile
            || set.memory_body_ids.get(location.row as usize) != Some(&body)
        {
            return Err(RewriteError("Memory-vector binding is inconsistent".into()));
        }
        let packed = source
            .packed_vectors(set.packed_vector_id)
            .map_err(|e| RewriteError(e.to_string()))?;
        let row = packed
            .row(location.row as usize)
            .ok_or_else(|| RewriteError("Memory-vector row is missing".into()))?;
        if schema.as_ref().is_some_and(|old| *old != packed.schema()) {
            return Err(RewriteError(
                "selected profile rows use inconsistent vector schemas".into(),
            ));
        }
        schema.get_or_insert_with(|| packed.schema().clone());
        bytes.extend_from_slice(row);
        body_ids.push(body);
    }
    Ok(schema.map(|schema| (body_ids, schema, bytes)))
}

fn emit_memories_rel(
    output: &mut Cva,
    selected: Vec<(MemoryId, bool, Vec<Memory>)>,
    effective: &[(MemoryId, crate::FreshnessRecord, u64)],
    mode: FreshnessRewriteMode,
    strip_provenance: bool,
    generation: HistoryGenerationId,
    source_cut: u64,
    destination_cut: u64,
    policy_digest: [u8; 32],
) -> Result<(), RewriteError> {
    let owner = output
        .owner_uuid()
        .ok_or_else(|| RewriteError("destination REL lacks owner UUID".into()))?;
    for (id, _, revisions) in &selected {
        for memory in revisions {
            let expected_revision = output
                .memory(*id)
                .map(|current| current.revision)
                .unwrap_or(0);
            output
                .memories
                .publish_with_source_ref_and_temporal_inference(
                    &mut output.container,
                    Some(*id),
                    expected_revision,
                    memory_draft(memory, strip_provenance),
                    if strip_provenance {
                        None
                    } else {
                        memory.source_ref.clone()
                    },
                    memory.temporal_inference.clone(),
                )
                .map_err(|e| RewriteError(e.to_string()))?;
        }
        if let Some(metadata) = revisions
            .last()
            .and_then(|memory| memory.routing_metadata.clone())
        {
            output
                .put_memory_routing_metadata(metadata)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
    }
    output
        .freshness
        .persist_policy_if_needed(&mut output.container, owner)
        .map_err(|e| RewriteError(e.to_string()))?;
    for (id, tombstoned, _) in selected {
        if mode == FreshnessRewriteMode::Reset {
            output
                .enroll_legacy_freshness_baseline(id)
                .map_err(|e| RewriteError(e.to_string()))?;
        } else {
            let (source_record, source_carry) = effective
                .iter()
                .find(|(memory_id, _, _)| *memory_id == id)
                .map(|(_, record, carry)| (*record, *carry))
                .unwrap_or_else(|| (crate::FreshnessRecord::created(), 0));
            let baseline =
                crate::freshness_storage::FreshnessRewriteBaseline::preserve_effective_with_carry(
                    owner,
                    generation,
                    id,
                    source_cut,
                    destination_cut,
                    source_record,
                    &output.freshness_policy(),
                    policy_digest,
                    source_carry,
                )?;
            let payload = baseline.encode()?;
            output
                .container
                .append(&payload)
                .map_err(|e| RewriteError(e.to_string()))?;
            output
                .freshness
                .ingest_rewrite_baseline(&payload)
                .map_err(RewriteError)?;
        }
        if tombstoned {
            let revision = output
                .memory(id)
                .map_err(|e| RewriteError(e.to_string()))?
                .revision;
            output
                .tombstone_memory(id, revision)
                .map_err(|e| RewriteError(e.to_string()))?;
        }
    }
    Ok(())
}

fn append_genesis(
    container: &mut Container,
    destination_owner: [u8; 16],
    predecessor_owner: [u8; 16],
    generation: HistoryGenerationId,
    predecessor_generation: HistoryGenerationId,
    plan: &RewritePlan,
) -> Result<(), RewriteError> {
    let genesis = crate::rewrite_lineage::RewriteGenesis {
        destination_owner,
        generation,
        predecessor_owner,
        predecessor_generation,
        source_head_digest: parse_digest(&plan.source_head_digest)?,
        policy_digest: parse_digest(&plan.policy_digest)?,
        created_at_unix_millis: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| RewriteError(e.to_string()))?
            .as_millis()
            .try_into()
            .map_err(|_| RewriteError("RewriteGenesis timestamp overflow".into()))?,
    };
    container
        .append(&genesis.encode())
        .map_err(|e| RewriteError(e.to_string()))?;
    Ok(())
}

fn source_owner_from_plan(plan: &RewritePlan) -> Result<[u8; 16], RewriteError> {
    plan.source_owner_uuid
        .as_deref()
        .map(parse_uuid_hex)
        .transpose()?
        .ok_or_else(|| {
            RewriteError("source owner identity is required for a rewritten lineage".into())
        })
}

fn source_generation(plan: &RewritePlan) -> Result<HistoryGenerationId, RewriteError> {
    Ok(HistoryGenerationId::from_bytes(parse_uuid_hex(
        &plan.source_generation,
    )?))
}

fn retained_ids(plan: &RewritePlan) -> Result<BTreeSet<MemoryId>, RewriteError> {
    plan.memory_decisions
        .iter()
        .filter(|decision| decision.retain)
        .map(|decision| Ok(MemoryId(parse_digest(&decision.id)?)))
        .collect()
}

fn load_selected_revisions_rel(
    source: &mut Cva,
    plan: &RewritePlan,
) -> Result<Vec<(MemoryId, bool, Vec<Memory>)>, RewriteError> {
    plan.selected_revisions
        .iter()
        .map(|selection| {
            let id = MemoryId(parse_digest(&selection.memory_id)?);
            let tombstoned = plan
                .memory_decisions
                .iter()
                .any(|decision| decision.id == selection.memory_id && decision.tombstoned);
            let revisions = selection
                .source_revisions
                .iter()
                .map(|revision| {
                    source
                        .memory_revision(id, *revision)
                        .map_err(|error| RewriteError(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if revisions.is_empty() {
                return Err(RewriteError(
                    "rewrite plan selected an empty revision set".into(),
                ));
            }
            Ok((id, tombstoned, revisions))
        })
        .collect()
}

fn load_selected_revisions_phy(
    source: &mut Phylactery,
    plan: &RewritePlan,
) -> Result<Vec<(MemoryId, bool, Vec<Memory>)>, RewriteError> {
    plan.selected_revisions
        .iter()
        .map(|selection| {
            let id = MemoryId(parse_digest(&selection.memory_id)?);
            let tombstoned = plan
                .memory_decisions
                .iter()
                .any(|decision| decision.id == selection.memory_id && decision.tombstoned);
            let revisions = selection
                .source_revisions
                .iter()
                .map(|revision| {
                    source
                        .memory_revision(id, *revision)
                        .map_err(|error| RewriteError(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if revisions.is_empty() {
                return Err(RewriteError(
                    "rewrite plan selected an empty revision set".into(),
                ));
            }
            Ok((id, tombstoned, revisions))
        })
        .collect()
}

fn memory_draft(memory: &Memory, strip_provenance: bool) -> MemoryDraft {
    MemoryDraft {
        category: memory.category.clone(),
        memory_type: memory.memory_type.clone(),
        authority_kind: memory.authority_kind.clone(),
        temporal_status: memory.temporal_status.clone(),
        title: memory.title.clone(),
        content: memory.content.clone(),
        scope: memory.scope.clone(),
        lifecycle_state: memory.lifecycle_state.clone(),
        archived: memory.archived,
        superseded_by: memory.superseded_by,
        parent_id: memory.parent_id,
        source_node_id: if strip_provenance {
            None
        } else {
            memory.source_node_id.clone()
        },
        content_source_conversation_id: if strip_provenance {
            None
        } else {
            memory.content_source_conversation_id.clone()
        },
        content_source_node_id: if strip_provenance {
            None
        } else {
            memory.content_source_node_id.clone()
        },
        grounding_source_conversation_id: if strip_provenance {
            None
        } else {
            memory.grounding_source_conversation_id.clone()
        },
        grounding_source_node_id: if strip_provenance {
            None
        } else {
            memory.grounding_source_node_id.clone()
        },
        source_episode_id: if strip_provenance {
            None
        } else {
            memory.source_episode_id
        },
        source_time_ns: if strip_provenance {
            None
        } else {
            memory.source_time_ns
        },
        mutation_id: memory.mutation_id.clone(),
        created_at_ns: memory.created_at_ns,
        updated_at_ns: memory.updated_at_ns,
    }
}
fn parse_digest(value: &str) -> Result<[u8; 32], RewriteError> {
    if value.len() != 64 {
        return Err(RewriteError("invalid rewrite digest".into()));
    }
    let mut out = [0; 32];
    for (i, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[i] = u8::from_str_radix(
            std::str::from_utf8(pair).map_err(|e| RewriteError(e.to_string()))?,
            16,
        )
        .map_err(|e| RewriteError(e.to_string()))?;
    }
    Ok(out)
}
fn parse_uuid_hex(value: &str) -> Result<[u8; 16], RewriteError> {
    if value.len() != 32 {
        return Err(RewriteError("invalid owner UUID".into()));
    }
    let mut out = [0; 16];
    for (i, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        out[i] = u8::from_str_radix(
            std::str::from_utf8(pair).map_err(|e| RewriteError(e.to_string()))?,
            16,
        )
        .map_err(|e| RewriteError(e.to_string()))?;
    }
    Ok(out)
}
fn hex16(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Rebuild scores from accepted journal facts whose Archive activity survives
/// selection. Prior rewrite baselines and superseded provisional events are not
/// retained history and do not seed this independent projection.
fn recompute_retained_freshness(
    source: &Cva,
    output: &Cva,
    memories: &[Memory],
) -> Result<Vec<(MemoryId, crate::FreshnessRecord, u64)>, RewriteError> {
    use crate::freshness_storage::FreshnessReplayEntry as E;
    use std::collections::BTreeMap;
    let policy = source.freshness_policy();
    let mut records: BTreeMap<_, _> = memories
        .iter()
        .map(|m| (m.id, crate::FreshnessRecord::created()))
        .collect();
    let turns: BTreeMap<_, _> = source
        .archive
        .nodes
        .iter()
        .filter_map(|node| {
            let before = source.activity_position_for_turn(&node.conversation_id, &node.id)?;
            let after = output.activity_position_for_turn(&node.conversation_id, &node.id)?;
            Some((before, after))
        })
        .collect();
    let map_turn = |turn| {
        if turn == 0 {
            Some(0)
        } else {
            turns.get(&turn).copied()
        }
    };
    let mut events = source
        .freshness
        .retained_history_entries()
        .map_err(RewriteError)?;
    events.sort_by_key(E::source_turn);
    for event in events {
        let Some(turn) = map_turn(event.source_turn()) else {
            continue;
        };
        match event {
            E::Initialize {
                memory_id,
                admitted_at_birth: true,
                ..
            }
            | E::Admit { memory_id, .. } => {
                if let Some(record) = records.get_mut(&memory_id) {
                    record
                        .admit(turn)
                        .map_err(|e| RewriteError(e.to_string()))?;
                }
            }
            E::Event {
                event_id, effects, ..
            } => {
                if source
                    .freshness
                    .import_settlements()
                    .iter()
                    .any(|s| s.superseded.contains(&event_id))
                {
                    continue;
                }
                for (id, delta) in effects {
                    if let Some(record) = records.get_mut(&id) {
                        record
                            .reinforce_with_policy(delta, turn, &policy)
                            .map_err(|e| RewriteError(e.to_string()))?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(records
        .into_iter()
        .map(|(id, record)| (id, record, 0))
        .collect())
}
