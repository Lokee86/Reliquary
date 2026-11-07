use crate::{
    CandidateKind,
    cut::{fields, observe_cut},
    evidence_encoding::hex,
    retained_commitment::commitment,
};
use reliquary_archive::{Branch, RetainedArchiveValue};
use reliquary_owner_session::ArchiveOwnerHistoryObservation;
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};

mod values;
use values::{Content, Project, episode_value, project_correlation, turn_value};

pub fn observe_archive_owner_history(kind: CandidateKind, path: &Path) -> Value {
    let report = json!({
        "schema_id": "reliquary-d02-archive-owner-history-v1",
        "status": "INCOMPLETE",
        "observation_status": "INCOMPLETE",
        "owner": null,
        "candidate_sha256_before": null,
        "candidate_sha256_after": null,
        "candidate_unchanged": null,
        "archive_head": null,
        "rows": null,
        "reason": "",
    });
    observe_cut(kind, path, report, |observer| {
        let history = observer.archive_owner_history()?;
        let head = history.history.archive_head;
        Ok(fields(json!({
            "observation_status": "OBSERVED",
            "archive_head": head.to_string(),
            "rows": retained_rows(history)?,
            "reason": "retained_archive_and_project_operational_only",
        })))
    })
}

fn retained_rows(observation: ArchiveOwnerHistoryObservation) -> Result<Vec<Value>, String> {
    let snapshot = observation.history.snapshot;
    if !snapshot.import_receipts.is_empty()
        || !snapshot.conversation_tombstones.is_empty()
        || !snapshot.fragment_definitions.is_empty()
    {
        return Err("archive_retained_population_outside_pinned_carriers".into());
    }
    let mut content = Content::new();
    let mut projects = Project::new();
    let mut values = BTreeMap::new();
    let mut add = |version, value| {
        if version == 0 || values.insert(version, value).is_some() {
            return Err("archive_publication_value_missing_origin_or_duplicate");
        }
        Ok(())
    };
    for (turn, origin) in snapshot.turns.iter().zip(&observation.history.turn_origins) {
        add(
            origin.archive_version,
            turn_value(turn, &mut content, &mut projects)?,
        )?;
    }
    for branch in &snapshot.branch_revisions {
        add(branch.archive_version, branch_value(&branch.branch))?;
    }
    for episode in &snapshot.episodes {
        let version = observation
            .episode_origins
            .get(&episode.id)
            .ok_or("archive_episode_origin_missing")?;
        add(*version, episode_value(episode))?;
    }
    for retained in &snapshot.retained_publications {
        let value = match &retained.value {
            RetainedArchiveValue::Fragment { fragment } => json!({"fragment": {
                "id": hex(&fragment.id.0), "conversation": fragment.conversation_id,
                "start": fragment.start_node_id, "end": fragment.end_node_id,
            }}),
            RetainedArchiveValue::ConversationMetadata {
                conversation_id,
                title,
                active,
                policy,
                policy_revision,
            } => json!({"conversation": {
                "id": conversation_id, "title": title, "active": active,
                "policy": policy, "policy_revision": policy_revision,
            }}),
        };
        add(retained.archive_version, value)?;
    }
    if u64::try_from(snapshot.publication_bindings.len()).ok() != Some(snapshot.archive_version) {
        return Err("archive_retained_publication_bindings_incomplete".into());
    }
    let mut rows = Vec::new();
    let mut used = 0;
    for (index, binding) in snapshot.publication_bindings.iter().enumerate() {
        if binding.archive_version != index as u64 + 1 {
            return Err("archive_retained_publication_bindings_out_of_order".into());
        }
        let value = values
            .get(&binding.first_applying_archive_version)
            .ok_or("archive_retained_publication_value_missing")?;
        used += usize::from(binding.archive_version == binding.first_applying_archive_version);
        rows.push(commitment("archive", "publication", &binding.archive_version.to_be_bytes(), json!({
            "archive_version": binding.archive_version, "global_version": binding.global_version,
            "transaction_time_ns": binding.transaction_time_ns, "record": value,
        }), "retained")?);
    }
    if used != values.len() {
        return Err("archive_retained_publication_value_unbound".into());
    }
    for (identity, value) in content {
        rows.push(commitment(
            "archive", "content", &identity, value, "retained",
        )?);
    }
    for (id, value) in projects {
        rows.push(commitment(
            "project_operational",
            "project_file_binding",
            &id,
            value,
            "retained",
        )?);
    }
    for correlation in &observation.project_correlations {
        rows.push(commitment(
            "project_operational",
            "project_revision_correlation",
            &correlation.sequence.to_be_bytes(),
            project_correlation(correlation),
            "retained",
        )?);
    }
    Ok(rows)
}

fn branch_value(branch: &Branch) -> Value {
    json!({"branch": {
        "id": branch.id, "conversation": branch.conversation_id,
        "leaf": branch.leaf_node_id, "canonical": branch.canonical,
    }})
}
