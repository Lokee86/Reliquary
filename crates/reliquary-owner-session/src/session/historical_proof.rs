use super::*;
use reliquary_freshness_store::FreshnessEventCut;

mod fingerprint;
mod grouped;
#[cfg(test)]
mod grouped_tests;
mod projection;
#[cfg(test)]
mod tests;

use grouped::validate_grouped_memory_cut;
use projection::{common_count, common_history, validate_rel_cut};

const MAX_PROOF_RECORDS: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HistoricalOwnerCut {
    pub memory_version: u64,
    pub perception_version: u64,
    pub graph_version: u64,
    pub ego_version: u64,
    pub vector_version: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelHistoricalCut {
    pub owner: HistoricalOwnerCut,
    pub archive_version: u64,
    pub freshness: FreshnessEventCut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalOwnerProof {
    pub owner_id: [u8; 16],
    pub file_kind: FileKind,
    pub history_generation: [u8; 32],
    pub digest: [u8; 32],
    pub retained_records: usize,
    pub unavailable_payloads: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoricalProofError {
    WrongOwner,
    WrongOwnerKind,
    FutureCut(&'static str),
    InconsistentCut(&'static str),
    LimitExceeded,
    InvalidState(String),
}

impl fmt::Display for HistoricalProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongOwner => f.write_str("historical query belongs to another owner"),
            Self::WrongOwnerKind => f.write_str("historical query uses the wrong owner kind"),
            Self::FutureCut(domain) => write!(f, "historical {domain} cut is unavailable"),
            Self::InconsistentCut(reason) => write!(f, "inconsistent historical cut: {reason}"),
            Self::LimitExceeded => f.write_str("historical proof record limit exceeded"),
            Self::InvalidState(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for HistoricalProofError {}

pub(super) struct CommonHistory {
    pub(super) memories: Vec<(MemoryId, MemoryHistory)>,
    pub(super) perception: reliquary_perception_store::PerceptionHistorySnapshot,
    pub(super) graph_history: Vec<Relation>,
    pub(super) graph: Vec<Relation>,
    pub(super) community_history: Vec<CommunitySnapshot>,
    pub(super) community_renames: Vec<reliquary_graph::CommunityRenameReceipt>,
    pub(super) ego: reliquary_ego_store::EgoHistorySnapshot,
    pub(super) generations: Vec<VectorGeneration>,
    pub(super) unavailable_payloads: usize,
}

impl RelSession {
    /// Capture the fully accepted owner heads as one explicit historical cut.
    pub fn current_historical_cut(&self) -> RelHistoricalCut {
        let rel_turn = self.archive.rel_turn_count();
        RelHistoricalCut {
            owner: HistoricalOwnerCut {
                memory_version: self.common.memories.memory_version(),
                perception_version: self.common.perception.version(),
                graph_version: self.common.graph.graph_version(),
                ego_version: self.common.ego.version(),
                vector_version: self.generations.vector_version(),
            },
            archive_version: self.archive.archive_version(),
            freshness: self.freshness.cut_through_turn(rel_turn),
        }
    }

    pub fn historical_proof(
        &self,
        expected_owner: [u8; 16],
        cut: &RelHistoricalCut,
    ) -> Result<HistoricalOwnerProof, HistoricalProofError> {
        if expected_owner != self.owner_id {
            return Err(HistoricalProofError::WrongOwner);
        }
        validate_grouped_memory_cut(&self.grouped_completions, cut.owner.memory_version)?;
        let common = common_history(&self.common, Some(&self.generations), cut.owner)?;
        let archive = self
            .archive
            .history_at(cut.archive_version)
            .map_err(|_| HistoricalProofError::FutureCut("Archive"))?;
        if cut.freshness.rel_turn > archive.activity_cut {
            return Err(HistoricalProofError::InconsistentCut(
                "Freshness activity exceeds Archive authority",
            ));
        }
        if !self.freshness.is_authoritative_cut(&cut.freshness) {
            return Err(HistoricalProofError::InconsistentCut(
                "Freshness event boundary is not authoritative",
            ));
        }
        let freshness = self
            .freshness
            .records_at(&cut.freshness)
            .map_err(HistoricalProofError::InvalidState)?;
        let freshness_evidence = self.freshness.replay_entries_at(&cut.freshness);
        validate_rel_cut(&common, &archive, &freshness)?;
        let retained = common_count(&common)
            .checked_add(archive.turns.len())
            .and_then(|count| count.checked_add(archive.branch_revisions.len()))
            .and_then(|count| count.checked_add(archive.episodes.len()))
            .and_then(|count| count.checked_add(archive.import_receipts.len()))
            .and_then(|count| count.checked_add(archive.fragment_definitions.len()))
            .and_then(|count| count.checked_add(archive.conversation_tombstones.len()))
            .and_then(|count| count.checked_add(freshness_evidence.len()))
            .ok_or(HistoricalProofError::LimitExceeded)?;
        if retained > MAX_PROOF_RECORDS {
            return Err(HistoricalProofError::LimitExceeded);
        }
        Ok(HistoricalOwnerProof {
            owner_id: self.owner_id,
            file_kind: FileKind::Reliquary,
            history_generation: self.history_generation,
            digest: fingerprint::rel(
                self.owner_id,
                &common,
                &archive,
                &freshness,
                &freshness_evidence,
                cut,
            ),
            retained_records: retained,
            unavailable_payloads: common.unavailable_payloads
                + archive
                    .turns
                    .iter()
                    .filter(|turn| turn.content.is_none())
                    .count(),
        })
    }
}

impl PhySession {
    /// Capture the fully accepted private-owner heads as one explicit cut.
    pub fn current_historical_cut(&self) -> HistoricalOwnerCut {
        HistoricalOwnerCut {
            memory_version: self.common.memories.memory_version(),
            perception_version: self.common.perception.version(),
            graph_version: self.common.graph.graph_version(),
            ego_version: self.common.ego.version(),
            vector_version: 0,
        }
    }

    pub fn historical_proof(
        &self,
        expected_owner: [u8; 16],
        cut: HistoricalOwnerCut,
    ) -> Result<HistoricalOwnerProof, HistoricalProofError> {
        if expected_owner != self.owner_id {
            return Err(HistoricalProofError::WrongOwner);
        }
        let common = common_history(&self.common, None, cut)?;
        let retained = common_count(&common);
        if retained > MAX_PROOF_RECORDS {
            return Err(HistoricalProofError::LimitExceeded);
        }
        Ok(HistoricalOwnerProof {
            owner_id: self.owner_id,
            file_kind: FileKind::Phylactery,
            history_generation: self.history_generation,
            digest: fingerprint::phy(self.owner_id, &common, cut),
            retained_records: retained,
            unavailable_payloads: common.unavailable_payloads,
        })
    }
}
