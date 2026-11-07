use super::*;

/// Execute an immutable rewrite plan into a new, no-replace destination.
pub fn execute_rewrite(
    plan: &RewritePlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<FreshHistoryRewriteReport, ProductError> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    if destination.exists() {
        return Err(crate::owner::publication_collision(destination));
    }
    let new_owner =
        (plan.policy.identity == RewriteIdentityMode::NewOwner).then(|| *Uuid::new_v4().as_bytes());
    let admission = GraphAdmission::for_changes(plan.source_owner_id, [], []);
    app::execute_memory_rewrite(plan, source, destination, new_owner, &admission)
        .map_err(rewrite_error)
}

pub fn prune_memory_copy(
    plan: &VerifiedCopyPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    selected: &[MemoryId],
) -> Result<PrunedOwnerCopyReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.inner.owner_id(), [], []);
    app::prune_verified_memory_copy(
        &plan.inner,
        source.as_ref(),
        destination.as_ref(),
        plan.inner.owner_id(),
        selected,
        &admission,
    )
    .map_err(|error| maintenance_error(error.to_string()))
}

pub fn prune_archive_copy(
    plan: &VerifiedCopyPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    conversations: &[String],
) -> Result<ArchivePruneReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.inner.owner_id(), [], []);
    app::prune_terminal_archive_copy(
        &plan.inner,
        source.as_ref(),
        destination.as_ref(),
        conversations,
        &admission,
    )
    .map_err(maintenance_error)
}

pub fn prune_entity_descriptions_copy(
    plan: &VerifiedCopyPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    entities: &[EntityId],
) -> Result<EntityPruneReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.inner.owner_id(), [], []);
    app::prune_terminal_entity_descriptions_copy(
        &plan.inner,
        source.as_ref(),
        destination.as_ref(),
        entities,
        &admission,
    )
    .map_err(maintenance_error)
}

pub fn prune_relationship_descriptions_copy(
    plan: &VerifiedCopyPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    relationships: &[RelationshipId],
) -> Result<RelationshipPruneReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.inner.owner_id(), [], []);
    app::prune_terminal_relationship_descriptions_copy(
        &plan.inner,
        source.as_ref(),
        destination.as_ref(),
        relationships,
        &admission,
    )
    .map_err(maintenance_error)
}

pub fn plan_profile_repair(
    source: impl AsRef<Path>,
    retained: &[CompatibilityProfileId],
) -> Result<ProfileRepairPlan, ProductError> {
    let owner = read_owner_id(source.as_ref())?;
    let admission = GraphAdmission::for_changes(owner, [], []);
    app::plan_profile_repair(source.as_ref(), &admission, retained)
        .map_err(|error| maintenance_error(error.to_string()))
}

pub fn execute_profile_repair(
    plan: &ProfileRepairPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<ProfileRepairReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.owner_id(), [], []);
    app::execute_profile_repair(plan, source.as_ref(), destination.as_ref(), &admission)
        .map_err(|error| maintenance_error(error.to_string()))
}

pub fn prune_retired_profiles_copy(
    plan: &VerifiedCopyPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    profiles: &[CompatibilityProfileId],
) -> Result<RetiredProfilePruneReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.inner.owner_id(), [], []);
    app::prune_retired_profile_vectors_copy(
        &plan.inner,
        source.as_ref(),
        destination.as_ref(),
        profiles,
        &admission,
    )
    .map_err(maintenance_error)
}

pub fn reclaim_storage_copy(
    plan: &VerifiedCopyPlan,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<StorageReclamationReport, ProductError> {
    let admission = GraphAdmission::for_changes(plan.inner.owner_id(), [], []);
    app::reclaim_storage_copy(
        &plan.inner,
        source.as_ref(),
        destination.as_ref(),
        &admission,
    )
    .map_err(maintenance_error)
}

/// Reconcile two native, same-owner REL histories into a distinct destination.
pub fn reconcile_rel(
    left: impl AsRef<Path>,
    right: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<ReconciliationReport, ProductError> {
    let left = left.as_ref();
    let right = right.as_ref();
    let destination = destination.as_ref();
    if destination.exists() {
        return Err(crate::owner::publication_collision(destination));
    }
    let left_owner = Rel::open(left)?;
    let right_owner = Rel::open(right)?;
    if left_owner.owner_id() != right_owner.owner_id() {
        return Err(ProductError::new(
            ProductErrorKind::Admission,
            "REL reconciliation requires the same owner identity",
        ));
    }
    let admission = GraphAdmission::for_changes(left_owner.owner_id().as_bytes(), [], []);
    let result = app::reconcile_rel_owner(left, right, destination, &admission, [], [])
        .map_err(maintenance_error)?;
    let relation = match result.comparison.relation {
        reliquary_owner_session::RelHistoryRelation::Identical => ReconciliationRelation::Identical,
        reliquary_owner_session::RelHistoryRelation::LeftExtendsRight => {
            ReconciliationRelation::LeftExtendsRight
        }
        reliquary_owner_session::RelHistoryRelation::RightExtendsLeft => {
            ReconciliationRelation::RightExtendsLeft
        }
        reliquary_owner_session::RelHistoryRelation::Diverged => ReconciliationRelation::Diverged,
    };
    let (mode, replayed_archive_records, replayed_memory_revisions, destination_dream_admissions) =
        match result.evidence {
            app::RelReconciliationEvidence::ExactCopy(_) => {
                (ReconciliationMode::ExactCopy, 0, 0, 0)
            }
            app::RelReconciliationEvidence::SemanticReplay(report) => (
                ReconciliationMode::SemanticReplay,
                report.replayed_archive_records,
                report.replayed_memory_revisions,
                report.destination_dream_admissions.len(),
            ),
        };
    Ok(ReconciliationReport {
        owner_id: crate::OwnerId::from_bytes(result.comparison.owner_id),
        relation,
        mode,
        common_records: result.comparison.common_record_count,
        left_records: result.comparison.left_record_count,
        right_records: result.comparison.right_record_count,
        replayed_archive_records,
        replayed_memory_revisions,
        destination_dream_admissions,
    })
}
