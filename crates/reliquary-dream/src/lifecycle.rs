use reliquary_memory_core::MemoryId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamLifecycleMemory {
    pub id: MemoryId,
    pub lifecycle_state: &'static str,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DreamLifecycleIntent {
    pub memory_id: MemoryId,
    pub lifecycle_state: &'static str,
    pub archived: bool,
    pub superseded_by: Option<MemoryId>,
}

/// Plans lifecycle intents from already-authorized owner facts. The caller
/// remains responsible for loading and revalidating every Memory and writing.
pub fn plan_lifecycle(
    source: DreamLifecycleMemory,
    superseded_targets: &[(DreamLifecycleMemory, Option<MemoryId>)],
    has_active_duplicate_peer: bool,
    mut canonical_candidates: Vec<DreamLifecycleMemory>,
    source_has_explicit_authority: bool,
) -> Vec<DreamLifecycleIntent> {
    let mut intents = Vec::new();
    let mut source_after_archive = source;
    let mut archived_ids = Vec::with_capacity(superseded_targets.len() + 1);
    for (target, superseded_by) in superseded_targets {
        intents.push(DreamLifecycleIntent {
            memory_id: target.id,
            lifecycle_state: "archived",
            archived: true,
            superseded_by: *superseded_by,
        });
        archived_ids.push(target.id);
        if target.id == source.id {
            source_after_archive.lifecycle_state = "archived";
            source_after_archive.archived = true;
            source_after_archive.superseded_by = *superseded_by;
        }
    }

    if !source_after_archive.archived
        && source_after_archive.lifecycle_state == "extracted"
        && has_active_duplicate_peer
    {
        intents.push(DreamLifecycleIntent {
            memory_id: source.id,
            lifecycle_state: "archived",
            archived: true,
            superseded_by: source.superseded_by,
        });
        archived_ids.push(source.id);
        source_after_archive.lifecycle_state = "archived";
        source_after_archive.archived = true;
    }

    if !source_after_archive.archived && source_has_explicit_authority {
        canonical_candidates.push(source_after_archive);
    }
    canonical_candidates.sort_by_key(|memory| memory.id.0);
    canonical_candidates.dedup_by_key(|memory| memory.id);
    let mut source_promoted_to_canonical = false;
    for candidate in canonical_candidates {
        if candidate.archived
            || candidate.lifecycle_state == "canonical"
            || archived_ids.contains(&candidate.id)
        {
            continue;
        }
        intents.push(DreamLifecycleIntent {
            memory_id: candidate.id,
            lifecycle_state: "canonical",
            archived: false,
            superseded_by: candidate.superseded_by,
        });
        source_promoted_to_canonical |= candidate.id == source.id;
    }

    if !source_promoted_to_canonical
        && !source_after_archive.archived
        && source_after_archive.lifecycle_state == "extracted"
    {
        intents.push(DreamLifecycleIntent {
            memory_id: source.id,
            lifecycle_state: "knowledge",
            archived: false,
            superseded_by: source_after_archive.superseded_by,
        });
    }
    intents
}

pub fn explicit_authority_promotes(
    authority_kind: &str,
    category: &str,
    memory_type: &str,
) -> bool {
    if !matches!(authority_kind, "direct" | "correction") {
        return false;
    }
    matches!(
        category,
        "decision" | "preference" | "instruction" | "constraint" | "correction" | "commitment"
    ) || (category == "fact"
        && matches!(memory_type, "project" | "process" | "product" | "schedule"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(id: u8, state: &'static str, archived: bool) -> DreamLifecycleMemory {
        DreamLifecycleMemory {
            id: MemoryId([id; 32]),
            lifecycle_state: state,
            archived,
            superseded_by: None,
        }
    }

    #[test]
    fn plans_archive_then_canonical_then_knowledge_in_stable_order() {
        let source = fact(3, "extracted", false);
        let target = fact(2, "knowledge", false);
        let mut other = fact(4, "knowledge", false);
        other.superseded_by = Some(source.id);
        let plan = plan_lifecycle(
            source,
            &[(target, Some(source.id))],
            false,
            vec![other],
            true,
        );
        assert_eq!(plan.len(), 3);
        assert_eq!(plan[0].memory_id, target.id);
        assert_eq!(plan[0].lifecycle_state, "archived");
        assert_eq!(plan[1].memory_id, source.id);
        assert_eq!(plan[1].lifecycle_state, "canonical");
        assert_eq!(plan[2].memory_id, other.id);
        assert_eq!(plan[2].lifecycle_state, "canonical");
    }

    #[test]
    fn archived_source_is_not_promoted_and_archived_candidates_stay_archived() {
        let source = fact(1, "extracted", false);
        let other = fact(2, "knowledge", true);
        let plan = plan_lifecycle(
            source,
            &[(source, Some(other.id))],
            false,
            vec![other],
            true,
        );
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].memory_id, source.id);
        assert_eq!(plan[0].lifecycle_state, "archived");
    }

    #[test]
    fn canonical_source_does_not_also_receive_knowledge_intent() {
        let source = fact(1, "extracted", false);
        let plan = plan_lifecycle(source, &[], false, vec![], true);
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].lifecycle_state, "canonical");
    }

    #[test]
    fn explicit_authority_policy_preserves_category_and_origin_gate() {
        assert!(explicit_authority_promotes("direct", "decision", "fact"));
        assert!(explicit_authority_promotes("correction", "fact", "project"));
        assert!(!explicit_authority_promotes("inferred", "decision", "fact"));
        assert!(!explicit_authority_promotes(
            "direct",
            "fact",
            "observation"
        ));
    }
}
