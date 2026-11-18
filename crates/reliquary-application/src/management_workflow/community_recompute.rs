use super::ManagementError;
use reliquary_owner_session::{
    CommunityRecomputeCut, CommunityRecomputeInput, OwnerSessionError, RelWriter,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Coverage and modularity for the calculated partition.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommunityRecomputeReport {
    pub eligible_count: usize,
    pub assigned_count: usize,
    pub community_count: usize,
    /// Number of communities for each member count.
    pub community_sizes: BTreeMap<usize, usize>,
    pub modularity: f64,
    /// Digest of the sorted groups and their sorted Memory identities.
    pub membership_sha256: String,
    pub generation: Option<u64>,
    pub changed: bool,
}

pub struct PreparedCommunityRecompute {
    cut: CommunityRecomputeCut,
    groups: Vec<Vec<[u8; 32]>>,
    report: CommunityRecomputeReport,
}

pub fn prepare_community_recompute(
    input: CommunityRecomputeInput,
) -> Result<PreparedCommunityRecompute, ManagementError> {
    let eligible_count = input.graph.eligible_count();
    let partition = input
        .graph
        .solve()
        .map_err(|error| ManagementError::Owner(error.to_string()))?;
    let mut community_sizes = BTreeMap::new();
    let mut assigned_count = 0;
    let mut hash = Sha256::new();
    hash.update(b"reliquary-community-partition-v1\0");
    for group in &partition.groups {
        *community_sizes.entry(group.len()).or_insert(0) += 1;
        assigned_count += group.len();
        hash.update((group.len() as u64).to_le_bytes());
        for member in group {
            hash.update(member);
        }
    }
    let report = CommunityRecomputeReport {
        eligible_count,
        assigned_count,
        community_count: partition.groups.len(),
        community_sizes,
        modularity: partition.modularity,
        membership_sha256: format!("{:x}", hash.finalize()),
        generation: None,
        changed: false,
    };
    Ok(PreparedCommunityRecompute {
        cut: input.cut,
        groups: partition.groups,
        report,
    })
}

impl PreparedCommunityRecompute {
    pub fn preview(self) -> CommunityRecomputeReport {
        self.report
    }

    pub fn apply(
        mut self,
        writer: &mut RelWriter,
    ) -> Result<CommunityRecomputeReport, ManagementError> {
        let (generation, changed) = writer
            .publish_recomputed_communities(self.cut, self.groups)
            .map_err(|error| match error {
                OwnerSessionError::ManagementStale => ManagementError::Stale,
                error => ManagementError::Owner(error.to_string()),
            })?;
        self.report.generation = Some(generation);
        self.report.changed = changed;
        Ok(self.report)
    }
}
