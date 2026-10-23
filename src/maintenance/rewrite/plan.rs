use super::model::*;
use crate::{HistoryGenerationId, MemoryId, compatibility_profile_model::CompatibilityProfileId};
use sha2::{Digest, Sha256};

use super::RewriteError;
use super::plan_source::{PhylacteryRewritePlanSource, ReliquaryRewritePlanSource};

fn parse_id(value: &str, label: &str) -> Result<[u8; 32], RewriteError> {
    if value.len() != 64 {
        return Err(RewriteError(format!(
            "{label} must contain 64 hexadecimal characters"
        )));
    }
    let mut bytes = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text =
            std::str::from_utf8(pair).map_err(|_| RewriteError(format!("invalid {label}")))?;
        bytes[index] =
            u8::from_str_radix(text, 16).map_err(|_| RewriteError(format!("invalid {label}")))?;
    }
    Ok(bytes)
}
fn id_hex(id: &[u8; 32]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}
fn uuid_hex(id: Option<[u8; 16]>) -> Option<String> {
    id.map(|id| id.iter().map(|b| format!("{b:02x}")).collect())
}

fn apply_explicit_selection(
    candidates: &mut [(MemoryId, bool, String, Vec<u8>)],
    request: &RewriteRequest,
) -> Result<(), RewriteError> {
    let included = request
        .memories
        .include_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let excluded = request
        .memories
        .exclude_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let all: std::collections::BTreeSet<_> = candidates.iter().map(|(id, ..)| id.0).collect();
    if let Some(id) = included.iter().find(|id| !all.contains(*id)) {
        return Err(RewriteError(format!(
            "explicitly included Memory {} does not exist in the source",
            id_hex(id)
        )));
    }
    if let Some(id) = excluded.iter().find(|id| !all.contains(*id)) {
        return Err(RewriteError(format!(
            "explicitly excluded Memory {} does not exist in the source",
            id_hex(id)
        )));
    }
    for (id, retain, reason, _) in candidates {
        if excluded.contains(&id.0) {
            *retain = false;
            *reason = "explicitly excluded".into();
        } else if included.contains(&id.0) {
            *retain = true;
            *reason = "explicitly included".into();
        }
    }
    Ok(())
}

fn apply_semantic_closure(
    candidates: &mut [(MemoryId, bool, String, Vec<u8>)],
    expanded: &std::collections::BTreeSet<MemoryId>,
    request: &RewriteRequest,
) -> Result<(), RewriteError> {
    let excluded = request
        .memories
        .exclude_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    for id in expanded {
        if excluded.contains(&id.0) {
            return Err(RewriteError(format!(
                "explicitly excluded Memory {} is required by retained semantic evidence",
                id_hex(&id.0)
            )));
        }
        let Some((_, retain, reason, _)) = candidates
            .iter_mut()
            .find(|(candidate, ..)| candidate == id)
        else {
            return Err(RewriteError(format!(
                "semantic evidence requires unavailable Memory {}",
                id_hex(&id.0)
            )));
        };
        if !*retain {
            *retain = true;
            *reason = "required by retained semantic evidence closure".into();
        }
    }
    Ok(())
}

fn memory_link_closure(
    retained: &std::collections::BTreeSet<MemoryId>,
    mut memory: impl FnMut(MemoryId) -> Result<crate::Memory, RewriteError>,
) -> Result<std::collections::BTreeSet<MemoryId>, RewriteError> {
    let mut closed = retained.clone();
    let mut pending: Vec<_> = retained.iter().copied().collect();
    while let Some(id) = pending.pop() {
        let value = memory(id)?;
        for reference in [value.parent_id, value.superseded_by].into_iter().flatten() {
            if closed.insert(reference) {
                pending.push(reference);
            }
        }
    }
    Ok(closed)
}

fn memory_has_provenance(memory: &crate::Memory) -> bool {
    memory.source_ref.is_some()
        || memory.source_node_id.is_some()
        || memory.content_source_node_id.is_some()
        || memory.content_source_conversation_id.is_some()
        || memory.grounding_source_node_id.is_some()
        || memory.grounding_source_conversation_id.is_some()
        || memory.source_episode_id.is_some()
        || memory.source_time_ns.is_some()
}

fn revision_numbers(
    current_revision: u64,
    policy: &RevisionPolicy,
    mut global_version: impl FnMut(u64) -> Result<u64, RewriteError>,
) -> Result<Vec<u64>, RewriteError> {
    match policy {
        RevisionPolicy::CurrentOnly => Ok(vec![current_revision]),
        RevisionPolicy::AllRetained => Ok((1..=current_revision).collect()),
        RevisionPolicy::SinceGlobalVersion(cut) => {
            let mut selected = Vec::new();
            for revision in 1..=current_revision {
                if global_version(revision)? >= *cut {
                    selected.push(revision);
                }
            }
            Ok(selected)
        }
    }
}

fn latest_rel_memory(
    source: &mut ReliquaryRewritePlanSource<'_>,
    id: MemoryId,
) -> Result<crate::Memory, RewriteError> {
    match source.memory(id) {
        Ok(memory) => Ok(memory),
        Err(_) => {
            let descriptor = source.memory_retention_descriptor(id).map_err(|error| {
                RewriteError(format!("tombstoned Memory is unavailable: {error}"))
            })?;
            let revision = descriptor
                .revisions
                .last()
                .map(|entry| entry.revision)
                .ok_or_else(|| RewriteError("tombstoned Memory has no retained revision".into()))?;
            match source
                .historical_memory_revision(id, revision)
                .map_err(|error| {
                    RewriteError(format!("tombstoned Memory payload is unavailable: {error}"))
                })? {
                crate::owner_lifecycle::HistoricalPayload::Available(memory)
                | crate::owner_lifecycle::HistoricalPayload::Tombstoned(memory) => Ok(memory),
                crate::owner_lifecycle::HistoricalPayload::Pruned { .. } => {
                    Err(RewriteError("tombstoned Memory payload was pruned".into()))
                }
            }
        }
    }
}

fn latest_phy_memory(
    source: &mut PhylacteryRewritePlanSource<'_>,
    id: MemoryId,
) -> Result<crate::Memory, RewriteError> {
    match source.memory(id) {
        Ok(memory) => Ok(memory),
        Err(_) => {
            let descriptor = source.memory_retention_descriptor(id).map_err(|error| {
                RewriteError(format!("tombstoned Memory is unavailable: {error}"))
            })?;
            let revision = descriptor
                .revisions
                .last()
                .map(|entry| entry.revision)
                .ok_or_else(|| RewriteError("tombstoned Memory has no retained revision".into()))?;
            source.memory_revision(id, revision).map_err(|error| {
                RewriteError(format!("tombstoned Memory payload is unavailable: {error}"))
            })
        }
    }
}

fn selected_revisions_rel(
    source: &mut ReliquaryRewritePlanSource<'_>,
    candidates: &[(MemoryId, bool, String, Vec<u8>)],
    request: &RewriteRequest,
) -> Result<Vec<MemoryRevisionSelection>, RewriteError> {
    let ids: Vec<_> = candidates
        .iter()
        .filter(|(_, retain, ..)| *retain)
        .map(|(id, ..)| *id)
        .collect();
    let mut selected = Vec::with_capacity(ids.len());
    for id in ids {
        let memory = latest_rel_memory(source, id)?;
        let revisions =
            revision_numbers(memory.revision, &request.memories.revisions, |revision| {
                source
                    .memory_revision(id, revision)
                    .map(|record| record.global_version)
                    .map_err(|error| {
                        RewriteError(format!("cannot inspect Memory revision: {error}"))
                    })
            })?;
        if revisions.is_empty() {
            return Err(RewriteError(format!(
                "retained Memory {} has no revision at the requested global-version cut",
                id_hex(&id.0)
            )));
        }
        selected.push(MemoryRevisionSelection {
            memory_id: id_hex(&id.0),
            source_revisions: revisions,
        });
    }
    Ok(selected)
}

fn selected_revisions_phy(
    source: &mut PhylacteryRewritePlanSource<'_>,
    candidates: &[(MemoryId, bool, String, Vec<u8>)],
    request: &RewriteRequest,
) -> Result<Vec<MemoryRevisionSelection>, RewriteError> {
    let ids: Vec<_> = candidates
        .iter()
        .filter(|(_, retain, ..)| *retain)
        .map(|(id, ..)| *id)
        .collect();
    let mut selected = Vec::with_capacity(ids.len());
    for id in ids {
        let memory = latest_phy_memory(source, id)?;
        let revisions =
            revision_numbers(memory.revision, &request.memories.revisions, |revision| {
                source
                    .memory_revision(id, revision)
                    .map(|record| record.global_version)
                    .map_err(|error| {
                        RewriteError(format!("cannot inspect Memory revision: {error}"))
                    })
            })?;
        if revisions.is_empty() {
            return Err(RewriteError(format!(
                "retained Memory {} has no revision at the requested global-version cut",
                id_hex(&id.0)
            )));
        }
        selected.push(MemoryRevisionSelection {
            memory_id: id_hex(&id.0),
            source_revisions: revisions,
        });
    }
    Ok(selected)
}

fn finish_plan(
    kind: &str,
    owner: Option<[u8; 16]>,
    global: u64,
    memory_version: u64,
    mut candidates: Vec<(MemoryId, bool, String, Vec<u8>)>,
    request: &RewriteRequest,
    profiles: Vec<CompatibilityProfileId>,
    coverage: Vec<(CompatibilityProfileId, usize)>,
    selected_revisions: Vec<MemoryRevisionSelection>,
    tombstoned_ids: std::collections::BTreeSet<MemoryId>,
    source_digest: [u8; 32],
    source_file_identity: (u64, u64),
    generation: HistoryGenerationId,
) -> Result<RewritePlan, RewriteError> {
    let included = request
        .memories
        .include_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let excluded = request
        .memories
        .exclude_ids
        .iter()
        .map(|s| parse_id(s, "Memory ID"))
        .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
    let all_ids: std::collections::BTreeSet<_> = candidates.iter().map(|(id, ..)| id.0).collect();
    if let Some(missing) = included.iter().find(|id| !all_ids.contains(*id)) {
        return Err(RewriteError(format!(
            "explicitly included Memory {} does not exist in the source",
            id_hex(missing)
        )));
    }
    if let Some(missing) = excluded.iter().find(|id| !all_ids.contains(*id)) {
        return Err(RewriteError(format!(
            "explicitly excluded Memory {} does not exist in the source",
            id_hex(missing)
        )));
    }
    for (id, retain, reason, _) in &mut candidates {
        let key = id.0;
        if excluded.contains(&key) && !reason.starts_with("required by retained semantic evidence")
        {
            *retain = false;
            *reason = "explicitly excluded".into();
        } else if included.contains(&key) {
            *retain = true;
            *reason = "explicitly included".into();
        }
    }
    candidates.sort_by_key(|(id, ..)| id.0);
    let memory_decisions = candidates
        .iter()
        .map(|(id, retain, reason, _)| MemoryRewriteDecision {
            id: id_hex(&id.0),
            retain: *retain,
            reason: reason.clone(),
            tombstoned: tombstoned_ids.contains(id),
        })
        .collect::<Vec<_>>();
    let selected_profiles = profiles.iter().map(|id| id_hex(&id.0)).collect::<Vec<_>>();
    let profile_missing_memory_rows = coverage
        .into_iter()
        .map(|(id, count)| (id_hex(&id.0), count))
        .collect();
    let retained_memories = memory_decisions.iter().filter(|item| item.retain).count();
    let dropped_memories = memory_decisions.len() - retained_memories;
    let policy_digest = String::new();
    Ok(RewritePlan {
        source_kind: kind.into(),
        source_owner_uuid: uuid_hex(owner),
        source_generation: generation.to_string(),
        source_global_version: global,
        source_memory_version: memory_version,
        source_head_digest: id_hex(&source_digest),
        source_file_identity,
        request: request.clone(),
        memory_decisions,
        selected_revisions,
        selected_profiles,
        profile_missing_memory_rows,
        retained_memories,
        dropped_memories,
        archive_conversations: Vec::new(),
        retained_archive_fragments: 0,
        profile_retained_memory_rows: Vec::new(),
        closure_expansions: Vec::new(),
        lossy_decisions: Vec::new(),
        policy_digest,
    })
}

fn validate_common(request: &RewriteRequest) -> Result<Vec<CompatibilityProfileId>, RewriteError> {
    let mut ids = Vec::new();
    for value in &request.profiles {
        ids.push(CompatibilityProfileId(parse_id(
            value,
            "compatibility profile ID",
        )?));
    }
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    Ok(ids)
}

impl ReliquaryRewritePlanSource<'_> {
    pub fn plan_rewrite(&mut self, request: &RewriteRequest) -> Result<RewritePlan, RewriteError> {
        let requested_profiles = validate_common(request)?;
        let all_profiles = self.compatibility_profiles.profiles();
        if let ArchiveSelection::Conversations(requested) = &request.archive {
            let known: std::collections::BTreeSet<_> = self
                .conversation_summaries()
                .into_iter()
                .map(|summary| summary.conversation_id)
                .collect();
            if let Some(missing) = requested.iter().find(|id| !known.contains(*id)) {
                return Err(RewriteError(format!(
                    "requested Archive conversation {missing:?} does not exist"
                )));
            }
        }
        for id in &requested_profiles {
            if !all_profiles.iter().any(|profile| profile.id == *id) {
                return Err(RewriteError(format!(
                    "requested compatibility profile {} does not exist",
                    id_hex(&id.0)
                )));
            }
        }
        let selected_ids: std::collections::BTreeSet<_> =
            requested_profiles.iter().map(|id| id.0).collect();
        let mut candidates = Vec::new();
        let mut source_ids = self.memories.current_ids();
        let tombstoned_ids: std::collections::BTreeSet<_> = if request.memories.include_tombstoned {
            self.memories.tombstoned_ids().into_iter().collect()
        } else {
            std::collections::BTreeSet::new()
        };
        source_ids.extend(tombstoned_ids.iter().copied());
        source_ids.sort_by_key(|id| id.0);
        source_ids.dedup();
        for id in source_ids {
            let memory = latest_rel_memory(self, id)?;
            let mut retain = request.memories.base == MemoryBase::AllCurrent
                || memory.lifecycle_state == "active";
            let mut reason = if retain {
                "current Memory"
            } else {
                "not active under base population"
            }
            .to_owned();
            if memory.archived && !request.memories.include_archived {
                retain = false;
                reason = "archived Memory excluded".into();
            }
            if memory.superseded_by.is_some() && !request.memories.include_superseded {
                retain = false;
                reason = "superseded Memory excluded".into();
            }
            if request.memories.drop_known_duplicates && memory.superseded_by.is_some() {
                retain = false;
                reason = "known superseded duplicate".into();
            }
            if request
                .memories
                .created_after_ns
                .is_some_and(|cut| memory.created_at_ns < cut)
            {
                retain = false;
                reason = "created before requested bound".into();
            }
            if request
                .memories
                .updated_after_ns
                .is_some_and(|cut| memory.updated_at_ns < cut)
            {
                retain = false;
                reason = "updated before requested bound".into();
            }
            if request
                .memories
                .source_after_ns
                .is_some_and(|cut| memory.source_time_ns.is_none_or(|time| time < cut))
            {
                retain = false;
                reason = "source time before requested bound".into();
            }
            if let RevisionPolicy::SinceGlobalVersion(cut) = &request.memories.revisions {
                if memory.global_version < *cut {
                    retain = false;
                    reason = "no Memory revision at requested global-version cut".into();
                }
            }
            if let Some(minimum) = request.memories.minimum_effective_freshness {
                let cut = self.rel_turn_count();
                let score = self
                    .freshness_record_at(id, cut)
                    .map_err(|error| RewriteError(error.to_string()))?
                    .map(|record| record.score_at_with_policy(cut, &self.freshness_policy()))
                    .transpose()
                    .map_err(|error| RewriteError(error.to_string()))?;
                if score.is_none_or(|score| score < minimum) {
                    retain = false;
                    reason = "below minimum effective Freshness".into();
                }
            }
            candidates.push((id, retain, reason, memory.title.into_bytes()));
        }
        apply_explicit_selection(&mut candidates, request)?;
        loop {
            let retained: std::collections::BTreeSet<_> = candidates
                .iter()
                .filter(|(_, keep, ..)| *keep)
                .map(|(id, ..)| *id)
                .collect();
            let mut expanded = memory_link_closure(&retained, |id| latest_rel_memory(self, id))?;
            let semantic = self.semantic_closure(&expanded)?;
            expanded.extend(semantic.expanded_memory_ids);
            apply_semantic_closure(&mut candidates, &expanded, request)?;
            if expanded == retained {
                break;
            }
        }
        if request.provenance == ProvenancePolicy::Reject {
            for (id, retain, ..) in &candidates {
                if *retain && memory_has_provenance(&latest_rel_memory(self, *id)?) {
                    return Err(RewriteError(format!(
                        "Memory {} requires source provenance",
                        id_hex(&id.0)
                    )));
                }
            }
        }
        let profiles = all_profiles
            .into_iter()
            .filter(|profile| selected_ids.contains(&profile.id.0))
            .collect::<Vec<_>>();
        let selected_revisions = selected_revisions_rel(self, &candidates, request)?;
        let mut selected_body_ids = Vec::new();
        for selection in &selected_revisions {
            let id = MemoryId(parse_id(&selection.memory_id, "Memory ID")?);
            for revision in &selection.source_revisions {
                let memory = self
                    .memory_revision(id, *revision)
                    .map_err(|e| RewriteError(e.to_string()))?;
                selected_body_ids.push((
                    id,
                    crate::memory_model::memory_body_id(&memory.title, &memory.content),
                ));
            }
        }
        let mut coverage = Vec::new();
        for profile in &profiles {
            let bodies: Vec<_> = selected_body_ids
                .iter()
                .filter(|(id, _)| candidates.iter().any(|(cid, keep, ..)| cid == id && *keep))
                .map(|(_, body)| *body)
                .collect();
            let missing = self
                .memory_vectors
                .missing_body_ids(profile.id, &bodies)
                .len();
            if missing > 0 {
                return Err(RewriteError(format!(
                    "profile {} lacks coverage for {missing} retained Memories",
                    id_hex(&profile.id.0)
                )));
            }
            coverage.push((profile.id, missing));
        }
        let mut plan = finish_plan(
            "rel",
            self.owner_uuid(),
            self.latest_global_version(),
            self.memory_version(),
            candidates,
            request,
            profiles.iter().map(|p| p.id).collect(),
            coverage,
            selected_revisions,
            tombstoned_ids,
            self.container
                .physical_digest()
                .map_err(|error| RewriteError(error.to_string()))?,
            self.container
                .backing_file_identity()
                .map_err(|error| RewriteError(error.to_string()))?
                .ok_or_else(|| RewriteError("source physical identity is unavailable".into()))?,
            self.history_generation(),
        )?;
        complete_plan_report(&mut plan);
        let body_count = selected_body_ids
            .iter()
            .map(|(_, body)| body.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        for (_, rows) in &mut plan.profile_retained_memory_rows {
            *rows = body_count;
        }
        let mut selected = Vec::new();
        for choice in &plan.selected_revisions {
            let id = MemoryId(parse_id(&choice.memory_id, "Memory ID")?);
            for revision in &choice.source_revisions {
                selected.push(
                    self.memory_revision(id, *revision)
                        .map_err(|e| RewriteError(e.to_string()))?,
                );
            }
        }
        let provenance = if request.provenance == ProvenancePolicy::Strip {
            &[][..]
        } else {
            selected.as_slice()
        };
        let conversations = super::archive_selection::rewrite_archive_conversations(
            self.archive,
            self.container,
            provenance,
            &request.archive,
        )
        .map_err(|e| RewriteError(e.to_string()))?;
        plan.retained_archive_fragments = self
            .fragments()
            .iter()
            .filter(|f| conversations.contains(&f.conversation_id))
            .count();
        plan.archive_conversations = conversations.into_iter().collect();
        if request.provenance == ProvenancePolicy::PreserveClosure {
            plan.closure_expansions.extend(
                plan.archive_conversations
                    .iter()
                    .map(|id| format!("retained source conversation {id}")),
            );
        }
        finalize_plan_digest(&mut plan)?;
        Ok(plan)
    }
}

impl PhylacteryRewritePlanSource<'_> {
    pub fn plan_rewrite(&mut self, request: &RewriteRequest) -> Result<RewritePlan, RewriteError> {
        if request.include_rel_ego_synthesis {
            return Err(RewriteError(
                "REL synthesized Ego selection is not applicable to Phylactery owners".into(),
            ));
        }
        if request.memories.minimum_effective_freshness.is_some() {
            return Err(RewriteError(
                "Freshness threshold filtering is unavailable for Phylactery owners".into(),
            ));
        }
        let requested_profiles = validate_common(request)?;
        let all_profiles = self.compatibility_profiles.profiles();
        for id in &requested_profiles {
            if !all_profiles.iter().any(|profile| profile.id == *id) {
                return Err(RewriteError(format!(
                    "requested compatibility profile {} does not exist",
                    id_hex(&id.0)
                )));
            }
        }
        let selected_ids: std::collections::BTreeSet<_> =
            requested_profiles.iter().map(|id| id.0).collect();
        let mut candidates = Vec::new();
        let mut source_ids = self.memories.current_ids();
        let tombstoned_ids: std::collections::BTreeSet<_> = if request.memories.include_tombstoned {
            self.memories.tombstoned_ids().into_iter().collect()
        } else {
            std::collections::BTreeSet::new()
        };
        source_ids.extend(tombstoned_ids.iter().copied());
        source_ids.sort_by_key(|id| id.0);
        source_ids.dedup();
        for id in source_ids {
            let memory = latest_phy_memory(self, id)?;
            let mut retain = request.memories.base == MemoryBase::AllCurrent
                || memory.lifecycle_state == "active";
            let mut reason = if retain {
                "current Memory"
            } else {
                "not active under base population"
            }
            .to_owned();
            if memory.archived && !request.memories.include_archived {
                retain = false;
                reason = "archived Memory excluded".into();
            }
            if memory.superseded_by.is_some() && !request.memories.include_superseded {
                retain = false;
                reason = "superseded Memory excluded".into();
            }
            if request.memories.drop_known_duplicates && memory.superseded_by.is_some() {
                retain = false;
                reason = "known superseded duplicate".into();
            }
            if memory.created_at_ns < request.memories.created_after_ns.unwrap_or(i64::MIN) {
                retain = false;
                reason = "created before requested bound".into();
            }
            if memory.updated_at_ns < request.memories.updated_after_ns.unwrap_or(i64::MIN) {
                retain = false;
                reason = "updated before requested bound".into();
            }
            if request
                .memories
                .source_after_ns
                .is_some_and(|cut| memory.source_time_ns.is_none_or(|time| time < cut))
            {
                retain = false;
                reason = "source time before requested bound".into();
            }
            if let RevisionPolicy::SinceGlobalVersion(cut) = &request.memories.revisions {
                if memory.global_version < *cut {
                    retain = false;
                    reason = "no Memory revision at requested global-version cut".into();
                }
            }
            candidates.push((id, retain, reason, memory.title.into_bytes()));
        }
        apply_explicit_selection(&mut candidates, request)?;
        loop {
            let retained: std::collections::BTreeSet<_> = candidates
                .iter()
                .filter(|(_, keep, ..)| *keep)
                .map(|(id, ..)| *id)
                .collect();
            let mut expanded = memory_link_closure(&retained, |id| latest_phy_memory(self, id))?;
            let semantic = self.semantic_closure(&expanded)?;
            expanded.extend(semantic.expanded_memory_ids);
            apply_semantic_closure(&mut candidates, &expanded, request)?;
            if expanded == retained {
                break;
            }
        }
        if request.provenance == ProvenancePolicy::Reject {
            for (id, retain, ..) in &candidates {
                if *retain && memory_has_provenance(&latest_phy_memory(self, *id)?) {
                    return Err(RewriteError(format!(
                        "Memory {} requires source provenance",
                        id_hex(&id.0)
                    )));
                }
            }
        }
        let profiles = all_profiles
            .into_iter()
            .filter(|profile| selected_ids.contains(&profile.id.0))
            .collect::<Vec<_>>();
        let selected_revisions = selected_revisions_phy(self, &candidates, request)?;
        let mut selected_body_ids = Vec::new();
        for selection in &selected_revisions {
            let id = MemoryId(parse_id(&selection.memory_id, "Memory ID")?);
            for revision in &selection.source_revisions {
                let memory = self
                    .memory_revision(id, *revision)
                    .map_err(|e| RewriteError(e.to_string()))?;
                selected_body_ids.push((
                    id,
                    crate::memory_model::memory_body_id(&memory.title, &memory.content),
                ));
            }
        }
        let mut coverage = Vec::new();
        for profile in &profiles {
            let bodies: Vec<_> = selected_body_ids
                .iter()
                .filter(|(id, _)| candidates.iter().any(|(cid, keep, ..)| cid == id && *keep))
                .map(|(_, body)| *body)
                .collect();
            let missing = self
                .memory_vectors
                .missing_body_ids(profile.id, &bodies)
                .len();
            if missing > 0 {
                return Err(RewriteError(format!(
                    "profile {} lacks coverage for {missing} retained Memories",
                    id_hex(&profile.id.0)
                )));
            }
            coverage.push((profile.id, missing));
        }
        let mut plan = finish_plan(
            "phy",
            self.owner_uuid(),
            self.latest_global_version(),
            self.memory_version(),
            candidates,
            request,
            profiles.iter().map(|p| p.id).collect(),
            coverage,
            selected_revisions,
            tombstoned_ids,
            self.container
                .physical_digest()
                .map_err(|error| RewriteError(error.to_string()))?,
            self.container
                .backing_file_identity()
                .map_err(|error| RewriteError(error.to_string()))?
                .ok_or_else(|| RewriteError("source physical identity is unavailable".into()))?,
            self.history_generation(),
        )?;
        complete_plan_report(&mut plan);
        let body_count = selected_body_ids
            .iter()
            .map(|(_, body)| body.0)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        for (_, rows) in &mut plan.profile_retained_memory_rows {
            *rows = body_count;
        }
        finalize_plan_digest(&mut plan)?;
        Ok(plan)
    }
}

fn complete_plan_report(plan: &mut RewritePlan) {
    plan.closure_expansions = plan
        .memory_decisions
        .iter()
        .filter(|d| d.retain && d.reason.contains("closure"))
        .map(|d| format!("Memory {}: {}", d.id, d.reason))
        .collect();
    plan.lossy_decisions = plan
        .memory_decisions
        .iter()
        .filter(|d| !d.retain)
        .map(|d| format!("Memory {}: {}", d.id, d.reason))
        .collect();
    plan.lossy_decisions.push("unselected profiles, obsolete transactions, processing receipts, indexes, Communities and derived caches are omitted".into());
    if plan.request.provenance == ProvenancePolicy::Strip {
        plan.lossy_decisions
            .push("Memory source provenance is explicitly stripped".into());
    }
    if plan.request.freshness != FreshnessRewriteMode::PreserveEffectiveCurrent {
        plan.lossy_decisions.push(
            "effective Freshness is replaced by the selected reset/recomputation policy".into(),
        );
    }
    plan.profile_retained_memory_rows = plan
        .selected_profiles
        .iter()
        .map(|id| (id.clone(), plan.retained_memories))
        .collect();
}
fn finalize_plan_digest(plan: &mut RewritePlan) -> Result<(), RewriteError> {
    plan.policy_digest.clear();
    let bytes = serde_json::to_vec(plan).map_err(|e| RewriteError(e.to_string()))?;
    plan.policy_digest = format!(
        "{:x}",
        Sha256::digest([b"reliquary-history-rewrite-plan-v1\0".as_slice(), &bytes].concat())
    );
    Ok(())
}
