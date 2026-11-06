//! Read-only planning for policy-driven REL/PHY fresh-history reconstruction.
//!
//! A plan freezes the exact source bytes and selected semantic identities. It
//! is deliberately not an authorization credential and performs no writes.

use reliquary_owner_session::model::{MemoryId, MemoryRecord, NodeRef, Relation};
use reliquary_owner_session::{
    FreshHistoryArchive, FreshHistoryFreshness, FreshHistoryIdentity, FreshHistoryRequest,
    FreshHistoryRewriteReport, GraphAdmission, PhySession, RelSession, rewrite_memory_history_copy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt, fs, path::Path};

const MAX_EXPLICIT_MEMORY_IDS: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RewriteIdentityMode {
    NewOwner,
    PreserveOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OwnerRewriteKind {
    Rel,
    Phy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ArchiveRewriteMode {
    Omit,
    RetainRequiredProvenance,
    RetainAll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FreshnessRewriteMode {
    Reset,
    PreserveEffectiveCurrent,
    RetainSelectedHistory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DerivedStateRewriteMode {
    Omit,
    RetainVerified,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryRewriteFilter {
    pub include: BTreeSet<[u8; 32]>,
    pub exclude: BTreeSet<[u8; 32]>,
    pub lifecycle_states: BTreeSet<String>,
    pub include_tombstoned: bool,
    pub include_archived: bool,
    pub include_superseded: bool,
    pub drop_known_duplicates: bool,
    pub created_from_ns: Option<i64>,
    pub updated_from_ns: Option<i64>,
    pub source_time_from_ns: Option<i64>,
    pub source_time_through_ns: Option<i64>,
    pub minimum_effective_freshness: Option<i16>,
    pub retain_all_revisions: bool,
    pub retain_revisions_since_memory_version: Option<u64>,
}

impl Default for MemoryRewriteFilter {
    fn default() -> Self {
        Self {
            include: BTreeSet::new(),
            exclude: BTreeSet::new(),
            lifecycle_states: BTreeSet::new(),
            include_tombstoned: false,
            include_archived: true,
            include_superseded: true,
            drop_known_duplicates: false,
            created_from_ns: None,
            updated_from_ns: None,
            source_time_from_ns: None,
            source_time_through_ns: None,
            minimum_effective_freshness: None,
            retain_all_revisions: false,
            retain_revisions_since_memory_version: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewritePolicy {
    pub identity: RewriteIdentityMode,
    pub memories: MemoryRewriteFilter,
    /// Empty is an explicit request for no embedding profiles.
    pub profile_ids: BTreeSet<[u8; 32]>,
    pub archive: ArchiveRewriteMode,
    pub freshness: FreshnessRewriteMode,
    pub derived_state: DerivedStateRewriteMode,
}

impl Default for RewritePolicy {
    fn default() -> Self {
        Self {
            identity: RewriteIdentityMode::NewOwner,
            memories: MemoryRewriteFilter::default(),
            profile_ids: BTreeSet::new(),
            archive: ArchiveRewriteMode::RetainRequiredProvenance,
            freshness: FreshnessRewriteMode::PreserveEffectiveCurrent,
            derived_state: DerivedStateRewriteMode::RetainVerified,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewriteSelection {
    pub retained_memory_ids: Vec<[u8; 32]>,
    pub dropped_memory_ids: Vec<[u8; 32]>,
    pub closure_expansions: Vec<[u8; 32]>,
    pub retained_revisions: u64,
    pub retained_tombstones: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewritePlan {
    pub kind: OwnerRewriteKind,
    pub source_owner_id: [u8; 16],
    pub source_bytes: u64,
    pub source_digest: [u8; 32],
    pub source_memory_version: u64,
    pub policy: RewritePolicy,
    pub selection: RewriteSelection,
    pub warnings: Vec<String>,
    pub estimated_minimum_output_bytes: u64,
    pub plan_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RewriteError {
    InvalidPolicy(&'static str),
    UnknownMemory([u8; 32]),
    Unsupported(String),
    OwnerRead(String),
    SourceRead(String),
    SourceChanged,
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPolicy(reason) => write!(f, "invalid rewrite policy: {reason}"),
            Self::UnknownMemory(id) => write!(f, "rewrite policy names unknown Memory {}", hex(id)),
            Self::Unsupported(reason) => write!(f, "unsupported rewrite policy: {reason}"),
            Self::OwnerRead(reason) => write!(f, "rewrite owner read failed: {reason}"),
            Self::SourceRead(reason) => write!(f, "rewrite source read failed: {reason}"),
            Self::SourceChanged => f.write_str("rewrite source changed after planning"),
        }
    }
}

impl std::error::Error for RewriteError {}

trait RewriteOwnerView {
    fn owner_id(&self) -> [u8; 16];
    fn memory_version(&self) -> u64;
    fn memory_ids_page(
        &self,
        after: Option<MemoryId>,
        limit: usize,
    ) -> Result<(u64, Vec<MemoryId>), String>;
    fn memory_history(
        &self,
        id: MemoryId,
    ) -> Result<Option<reliquary_owner_session::model::MemoryHistory>, String>;
    fn graph_relations(&self) -> Vec<Relation>;
    fn semantic_memory_closure(
        &self,
        retained: &BTreeSet<[u8; 32]>,
    ) -> Result<Vec<MemoryId>, String>;
    fn has_profile(&self, id: [u8; 32]) -> bool;
    fn has_memory_vector(&self, profile: [u8; 32], body: [u8; 32]) -> Result<bool, String>;
    fn effective_freshness(&self, id: MemoryId) -> Result<Option<i16>, String>;
}

macro_rules! owner_view {
    ($owner:ty, |$this:ident, $memory_id:ident| $freshness:block) => {
        impl RewriteOwnerView for $owner {
            fn owner_id(&self) -> [u8; 16] {
                self.owner_id()
            }
            fn memory_version(&self) -> u64 {
                self.memory_version()
            }
            fn memory_ids_page(
                &self,
                after: Option<MemoryId>,
                limit: usize,
            ) -> Result<(u64, Vec<MemoryId>), String> {
                self.memory_ids_page(after, limit)
                    .map_err(|error| error.to_string())
            }
            fn memory_history(
                &self,
                id: MemoryId,
            ) -> Result<Option<reliquary_owner_session::model::MemoryHistory>, String> {
                self.memory_history(id).map_err(|error| error.to_string())
            }
            fn graph_relations(&self) -> Vec<Relation> {
                self.graph_relations()
            }
            fn semantic_memory_closure(
                &self,
                retained: &BTreeSet<[u8; 32]>,
            ) -> Result<Vec<MemoryId>, String> {
                self.fresh_history_memory_closure(retained.iter().copied().map(MemoryId))
                    .map_err(|error| error.to_string())
            }
            fn has_profile(&self, id: [u8; 32]) -> bool {
                self.active_compatibility_profile(
                    reliquary_owner_session::model::CompatibilityProfileId(id),
                )
                .is_some()
            }
            fn has_memory_vector(&self, profile: [u8; 32], body: [u8; 32]) -> Result<bool, String> {
                self.memory_vector_row(
                    reliquary_owner_session::model::CompatibilityProfileId(profile),
                    reliquary_owner_session::model::MemoryBodyId(body),
                )
                .map(|row| row.is_some())
                .map_err(|error| error.to_string())
            }
            fn effective_freshness(&self, id: MemoryId) -> Result<Option<i16>, String> {
                let $this = self;
                let $memory_id = id;
                $freshness
            }
        }
    };
}
owner_view!(RelSession, |owner, id| {
    let turn = owner.rel_turn_count();
    owner
        .freshness_record(reliquary_owner_session::FreshnessMemoryId(id.0))
        .map_err(|error| error.to_string())?
        .map(|record| record.score_at_with_policy(turn, &owner.freshness_policy()))
        .transpose()
        .map_err(|error| error.to_string())
});
owner_view!(PhySession, |_owner, _id| { Ok(None) });

pub fn plan_rel_rewrite(
    source: &Path,
    owner: &RelSession,
    policy: RewritePolicy,
) -> Result<RewritePlan, RewriteError> {
    plan_rewrite(source, owner, OwnerRewriteKind::Rel, policy)
}

pub fn plan_phy_rewrite(
    source: &Path,
    owner: &PhySession,
    policy: RewritePolicy,
) -> Result<RewritePlan, RewriteError> {
    if policy.memories.minimum_effective_freshness.is_some() {
        return Err(RewriteError::Unsupported(
            "PHY has no REL Freshness authority for threshold filtering".into(),
        ));
    }
    if policy.archive != ArchiveRewriteMode::Omit {
        return Err(RewriteError::Unsupported(
            "PHY cannot retain REL Archive state".into(),
        ));
    }
    if policy.freshness != FreshnessRewriteMode::Reset {
        return Err(RewriteError::Unsupported(
            "PHY has no REL Freshness authority".into(),
        ));
    }
    plan_rewrite(source, owner, OwnerRewriteKind::Phy, policy)
}

pub fn verify_rewrite_source(plan: &RewritePlan, source: &Path) -> Result<(), RewriteError> {
    let (bytes, digest) = source_proof(source)?;
    if bytes != plan.source_bytes || digest != plan.source_digest {
        return Err(RewriteError::SourceChanged);
    }
    Ok(())
}

/// Execute a frozen semantic rewrite plan into a separately verified owner.
pub fn execute_memory_rewrite(
    plan: &RewritePlan,
    source: &Path,
    destination: &Path,
    new_owner_id: Option<[u8; 16]>,
    graph_admission: &GraphAdmission,
) -> Result<FreshHistoryRewriteReport, RewriteError> {
    verify_rewrite_source(plan, source)?;
    let identity = match plan.policy.identity {
        RewriteIdentityMode::NewOwner => FreshHistoryIdentity::NewOwner(new_owner_id.ok_or(
            RewriteError::InvalidPolicy("NewOwner requires a destination owner identity"),
        )?),
        RewriteIdentityMode::PreserveOwner => {
            if new_owner_id.is_some() {
                return Err(RewriteError::InvalidPolicy(
                    "PreserveOwner cannot replace the owner identity",
                ));
            }
            FreshHistoryIdentity::PreserveOwner
        }
    };
    let request = FreshHistoryRequest {
        source_owner_id: plan.source_owner_id,
        source_digest: plan.source_digest,
        plan_digest: plan.plan_digest,
        identity,
        retained_memory_ids: plan
            .selection
            .retained_memory_ids
            .iter()
            .copied()
            .map(MemoryId)
            .collect(),
        retain_all_revisions: plan.policy.memories.retain_all_revisions,
        retain_revisions_since_memory_version: plan
            .policy
            .memories
            .retain_revisions_since_memory_version,
        retain_derived_state: plan.policy.derived_state == DerivedStateRewriteMode::RetainVerified,
        archive: match plan.policy.archive {
            ArchiveRewriteMode::Omit => FreshHistoryArchive::Omit,
            ArchiveRewriteMode::RetainRequiredProvenance => FreshHistoryArchive::RequiredProvenance,
            ArchiveRewriteMode::RetainAll => FreshHistoryArchive::All,
        },
        freshness: match plan.policy.freshness {
            FreshnessRewriteMode::Reset => FreshHistoryFreshness::Reset,
            FreshnessRewriteMode::PreserveEffectiveCurrent => {
                FreshHistoryFreshness::PreserveCurrent
            }
            FreshnessRewriteMode::RetainSelectedHistory => FreshHistoryFreshness::RetainHistory,
        },
        profile_ids: plan
            .policy
            .profile_ids
            .iter()
            .copied()
            .map(reliquary_owner_session::model::CompatibilityProfileId)
            .collect(),
    };
    rewrite_memory_history_copy(source, destination, &request, graph_admission).map_err(|reason| {
        if reason.contains("unsupported")
            || reason.contains("cannot be safely replayed")
            || reason.contains("omits present")
            || reason.contains("lacks retained")
        {
            RewriteError::Unsupported(reason)
        } else {
            RewriteError::OwnerRead(reason)
        }
    })
}

fn plan_rewrite(
    source: &Path,
    owner: &impl RewriteOwnerView,
    kind: OwnerRewriteKind,
    policy: RewritePolicy,
) -> Result<RewritePlan, RewriteError> {
    validate_policy(&policy)?;
    let (source_bytes, source_digest) = source_proof(source)?;
    if policy.profile_ids.iter().any(|id| !owner.has_profile(*id)) {
        return Err(RewriteError::Unsupported(
            "selected embedding profile is unavailable or retired".into(),
        ));
    }
    let source_memory_version = owner.memory_version();
    let mut ids = Vec::new();
    let mut after = None;
    loop {
        let (cut, page) = owner
            .memory_ids_page(after, 128)
            .map_err(RewriteError::OwnerRead)?;
        if cut != source_memory_version {
            return Err(RewriteError::SourceChanged);
        }
        if page.is_empty() {
            break;
        }
        after = page.last().copied();
        ids.extend(page);
    }
    let known: BTreeSet<_> = ids.iter().map(|id| id.0).collect();
    for id in policy
        .memories
        .include
        .iter()
        .chain(&policy.memories.exclude)
    {
        if !known.contains(id) {
            return Err(RewriteError::UnknownMemory(*id));
        }
    }

    let mut retained = BTreeSet::new();
    let mut dropped = BTreeSet::new();
    for id in ids {
        let history = owner
            .memory_history(id)
            .map_err(RewriteError::OwnerRead)?
            .ok_or(RewriteError::SourceChanged)?;
        let latest = history
            .revisions
            .last()
            .ok_or_else(|| RewriteError::OwnerRead("Memory history has no revision".into()))?;
        let freshness = if policy.memories.minimum_effective_freshness.is_some() {
            owner
                .effective_freshness(id)
                .map_err(RewriteError::OwnerRead)?
        } else {
            None
        };
        if selected(
            &policy.memories,
            &latest.record,
            history.tombstone.is_some(),
            freshness,
        ) {
            retained.insert(id.0);
        } else {
            dropped.insert(id.0);
        }
    }

    let mut closure_expansions = BTreeSet::new();
    let graph_relations = owner.graph_relations();
    loop {
        let mut changed = false;
        for &id in retained.clone().iter() {
            let history = owner
                .memory_history(MemoryId(id))
                .map_err(RewriteError::OwnerRead)?
                .ok_or(RewriteError::SourceChanged)?;
            let record = &history
                .revisions
                .last()
                .ok_or_else(|| RewriteError::OwnerRead("Memory history has no revision".into()))?
                .record;
            for dependency in [record.parent_id, record.superseded_by]
                .into_iter()
                .flatten()
            {
                if !known.contains(&dependency.0) {
                    return Err(RewriteError::UnknownMemory(dependency.0));
                }
                if retained.insert(dependency.0) {
                    dropped.remove(&dependency.0);
                    closure_expansions.insert(dependency.0);
                    changed = true;
                }
            }
        }
        for relation in &graph_relations {
            let endpoints = [relation.source, relation.target];
            if !endpoints
                .iter()
                .any(|node| matches!(node, NodeRef::Memory(id) if retained.contains(id)))
            {
                continue;
            }
            for node in endpoints {
                if let NodeRef::Memory(id) = node {
                    if !known.contains(&id) {
                        return Err(RewriteError::UnknownMemory(id));
                    }
                    if retained.insert(id) {
                        dropped.remove(&id);
                        closure_expansions.insert(id);
                        changed = true;
                    }
                }
            }
        }
        if policy.derived_state == DerivedStateRewriteMode::RetainVerified {
            for id in owner
                .semantic_memory_closure(&retained)
                .map_err(RewriteError::OwnerRead)?
            {
                if retained.insert(id.0) {
                    dropped.remove(&id.0);
                    closure_expansions.insert(id.0);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut retained_revisions = 0u64;
    let mut retained_tombstones = 0u64;
    for &id in &retained {
        let history = owner
            .memory_history(MemoryId(id))
            .map_err(RewriteError::OwnerRead)?
            .ok_or(RewriteError::SourceChanged)?;
        let revisions = selected_revisions(&history, &policy.memories)?;
        retained_revisions += revisions.len() as u64;
        retained_tombstones += u64::from(history.tombstone.is_some());
        for profile in &policy.profile_ids {
            for revision in &revisions {
                if !owner
                    .has_memory_vector(*profile, revision.record.body_id.0)
                    .map_err(RewriteError::OwnerRead)?
                {
                    return Err(RewriteError::Unsupported(
                        "selected embedding profile lacks retained Memory coverage".into(),
                    ));
                }
            }
        }
    }

    let mut warnings = vec![
        "operational Dream/import/Insomnia evidence is an explicit unsupported-state stop".into(),
    ];
    if policy.profile_ids.is_empty() {
        warnings.push("no embedding profiles or vectors will be retained".into());
    }
    if policy.archive == ArchiveRewriteMode::RetainRequiredProvenance {
        warnings.push("REL Archive selection is expanded only for retained provenance".into());
    }
    let estimated_minimum_output_bytes = 64u64
        .saturating_add(retained_revisions.saturating_mul(256))
        .saturating_add((policy.profile_ids.len() as u64).saturating_mul(128));
    let selection = RewriteSelection {
        retained_memory_ids: retained.into_iter().collect(),
        dropped_memory_ids: dropped.into_iter().collect(),
        closure_expansions: closure_expansions.into_iter().collect(),
        retained_revisions,
        retained_tombstones,
    };
    let mut plan = RewritePlan {
        kind,
        source_owner_id: owner.owner_id(),
        source_bytes,
        source_digest,
        source_memory_version,
        policy,
        selection,
        warnings,
        estimated_minimum_output_bytes,
        plan_digest: [0; 32],
    };
    plan.plan_digest = digest_plan(&plan)?;
    Ok(plan)
}

fn validate_policy(policy: &RewritePolicy) -> Result<(), RewriteError> {
    if policy.memories.include.len() > MAX_EXPLICIT_MEMORY_IDS
        || policy.memories.exclude.len() > MAX_EXPLICIT_MEMORY_IDS
    {
        return Err(RewriteError::InvalidPolicy(
            "explicit Memory selection exceeds bound",
        ));
    }
    if !policy
        .memories
        .include
        .is_disjoint(&policy.memories.exclude)
    {
        return Err(RewriteError::InvalidPolicy(
            "a Memory cannot be both included and excluded",
        ));
    }
    if policy
        .memories
        .source_time_from_ns
        .zip(policy.memories.source_time_through_ns)
        .is_some_and(|(from, through)| from > through)
    {
        return Err(RewriteError::InvalidPolicy("source-time range is inverted"));
    }
    if policy.memories.retain_all_revisions
        && policy
            .memories
            .retain_revisions_since_memory_version
            .is_some()
    {
        return Err(RewriteError::InvalidPolicy(
            "revision history modes are contradictory",
        ));
    }
    if policy
        .memories
        .retain_revisions_since_memory_version
        .is_some_and(|cut| cut == 0)
    {
        return Err(RewriteError::InvalidPolicy(
            "revision memory-version cut must be positive",
        ));
    }
    if policy
        .memories
        .lifecycle_states
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err(RewriteError::InvalidPolicy(
            "lifecycle filters cannot be empty",
        ));
    }
    Ok(())
}

fn selected(
    filter: &MemoryRewriteFilter,
    record: &MemoryRecord,
    tombstoned: bool,
    effective_freshness: Option<i16>,
) -> bool {
    let id = record.id.0;
    if filter.exclude.contains(&id) {
        return false;
    }
    if !filter.include.is_empty() && !filter.include.contains(&id) {
        return false;
    }
    if tombstoned && !filter.include_tombstoned {
        return false;
    }
    if record.archived && !filter.include_archived {
        return false;
    }
    if record.superseded_by.is_some()
        && (!filter.include_superseded || filter.drop_known_duplicates)
    {
        return false;
    }
    if !filter.lifecycle_states.is_empty()
        && !filter.lifecycle_states.contains(&record.lifecycle_state)
    {
        return false;
    }
    if filter
        .created_from_ns
        .is_some_and(|from| record.created_at_ns < from)
        || filter
            .updated_from_ns
            .is_some_and(|from| record.updated_at_ns < from)
    {
        return false;
    }
    if filter
        .source_time_from_ns
        .is_some_and(|from| record.source_time_ns.is_none_or(|time| time < from))
    {
        return false;
    }
    if filter
        .source_time_through_ns
        .is_some_and(|through| record.source_time_ns.is_none_or(|time| time > through))
    {
        return false;
    }
    if filter
        .minimum_effective_freshness
        .is_some_and(|minimum| effective_freshness.is_none_or(|score| score < minimum))
    {
        return false;
    }
    true
}

fn selected_revisions<'a>(
    history: &'a reliquary_owner_session::model::MemoryHistory,
    filter: &MemoryRewriteFilter,
) -> Result<Vec<&'a reliquary_owner_session::model::MemoryRevision>, RewriteError> {
    let revisions = if filter.retain_all_revisions {
        history.revisions.iter().collect::<Vec<_>>()
    } else if let Some(cut) = filter.retain_revisions_since_memory_version {
        history
            .revisions
            .iter()
            .filter(|revision| revision.record.memory_version >= cut)
            .collect::<Vec<_>>()
    } else {
        history.revisions.last().into_iter().collect::<Vec<_>>()
    };
    if revisions.is_empty() {
        return Err(RewriteError::Unsupported(
            "retained Memory has no revision at the requested cut".into(),
        ));
    }
    Ok(revisions)
}

fn source_proof(path: &Path) -> Result<(u64, [u8; 32]), RewriteError> {
    let bytes = fs::read(path).map_err(|error| RewriteError::SourceRead(error.to_string()))?;
    Ok((bytes.len() as u64, Sha256::digest(&bytes).into()))
}

fn digest_plan(plan: &RewritePlan) -> Result<[u8; 32], RewriteError> {
    let mut canonical = plan.clone();
    canonical.plan_digest = [0; 32];
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| RewriteError::OwnerRead(error.to_string()))?;
    Ok(Sha256::digest(bytes).into())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "rewrite_workflow/tests.rs"]
mod tests;
