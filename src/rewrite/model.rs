use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityPolicy {
    NewOwner,
    PreserveOwner,
}
impl Default for IdentityPolicy {
    fn default() -> Self {
        Self::NewOwner
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryBase {
    AllCurrent,
    ActiveCurrent,
}
impl Default for MemoryBase {
    fn default() -> Self {
        Self::AllCurrent
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RevisionPolicy {
    CurrentOnly,
    AllRetained,
    SinceGlobalVersion(u64),
}
impl Default for RevisionPolicy {
    fn default() -> Self {
        Self::CurrentOnly
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MemorySelection {
    pub base: MemoryBase,
    pub include_ids: Vec<String>,
    pub exclude_ids: Vec<String>,
    pub include_archived: bool,
    pub include_superseded: bool,
    pub include_tombstoned: bool,
    pub drop_known_duplicates: bool,
    pub minimum_effective_freshness: Option<i16>,
    pub created_after_ns: Option<i64>,
    pub updated_after_ns: Option<i64>,
    pub source_after_ns: Option<i64>,
    pub revisions: RevisionPolicy,
}
impl Default for MemorySelection {
    fn default() -> Self {
        Self {
            base: MemoryBase::AllCurrent,
            include_ids: vec![],
            exclude_ids: vec![],
            include_archived: true,
            include_superseded: true,
            include_tombstoned: false,
            drop_known_duplicates: false,
            minimum_effective_freshness: None,
            created_after_ns: None,
            updated_after_ns: None,
            source_after_ns: None,
            revisions: RevisionPolicy::CurrentOnly,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum FreshnessRewriteMode {
    PreserveEffectiveCurrent,
    Reset,
    RecomputeRetainedHistory,
}
impl Default for FreshnessRewriteMode {
    fn default() -> Self {
        Self::PreserveEffectiveCurrent
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ArchiveSelection {
    AllRetained,
    CurrentFromActivity(u64),
    Conversations(Vec<String>),
    ProvenanceClosureOnly,
}
impl Default for ArchiveSelection {
    fn default() -> Self {
        Self::ProvenanceClosureOnly
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenancePolicy {
    PreserveClosure,
    Strip,
    Reject,
}
impl Default for ProvenancePolicy {
    fn default() -> Self {
        Self::PreserveClosure
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RewriteRequest {
    pub identity: IdentityPolicy,
    pub memories: MemorySelection,
    /// Compatibility profile IDs encoded as 64 hexadecimal characters.
    pub profiles: Vec<String>,
    pub freshness: FreshnessRewriteMode,
    pub archive: ArchiveSelection,
    pub provenance: ProvenancePolicy,
    /// Copy REL's current synthesized Ego projection when true. PHY's authoritative
    /// user-owned Ego state is always preserved by its semantic-state emitter.
    pub include_rel_ego_synthesis: bool,
}
impl Default for RewriteRequest {
    fn default() -> Self {
        Self {
            identity: IdentityPolicy::NewOwner,
            memories: MemorySelection::default(),
            profiles: vec![],
            freshness: FreshnessRewriteMode::PreserveEffectiveCurrent,
            archive: ArchiveSelection::default(),
            provenance: ProvenancePolicy::PreserveClosure,
            include_rel_ego_synthesis: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips_as_a_policy_file_with_defaults() {
        let request = RewriteRequest::default();
        let json = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<RewriteRequest>(&json).unwrap(),
            request
        );
        let short = serde_json::from_str::<RewriteRequest>("{}").unwrap();
        assert_eq!(short, request);
    }

    #[test]
    fn policy_file_rejects_unknown_fields() {
        assert!(serde_json::from_str::<RewriteRequest>(r#"{"unexpected":true}"#).is_err());
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryRewriteDecision {
    pub id: String,
    pub retain: bool,
    pub reason: String,
    pub tombstoned: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoryRevisionSelection {
    pub memory_id: String,
    /// Source revision numbers selected by the policy. Emission compacts these
    /// into a new per-Memory revision sequence while preserving the stable ID.
    pub source_revisions: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewritePlan {
    pub source_kind: String,
    pub source_owner_uuid: Option<String>,
    pub source_generation: String,
    pub source_global_version: u64,
    pub source_memory_version: u64,
    pub source_head_digest: String,
    pub request: RewriteRequest,
    pub memory_decisions: Vec<MemoryRewriteDecision>,
    pub selected_revisions: Vec<MemoryRevisionSelection>,
    pub selected_profiles: Vec<String>,
    pub profile_missing_memory_rows: Vec<(String, usize)>,
    pub retained_memories: usize,
    pub dropped_memories: usize,
    pub archive_conversations: Vec<String>,
    pub retained_archive_fragments: usize,
    pub profile_retained_memory_rows: Vec<(String, usize)>,
    pub closure_expansions: Vec<String>,
    pub lossy_decisions: Vec<String>,
    pub policy_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RewriteReport {
    pub plan: RewritePlan,
    pub output_path: String,
    pub destination_owner_uuid: String,
    pub destination_generation: String,
}
