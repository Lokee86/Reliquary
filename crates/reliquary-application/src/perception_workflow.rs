//! Application composition for owner-local Observation discovery and routing.
mod contribution;
mod discovery;
mod phy_discovery;

pub use contribution::{
    ContributionEmbeddingCache, ContributionFailure, ContributionWork, capture_contribution,
    capture_duplicate_contribution, evaluate_contribution,
};
pub use discovery::retain_current_ego_evidence;
pub use discovery::{
    AuditDiscovery, AuditDiscoveryResult, audit_lookups, capture_discovery, discover,
};
pub use phy_discovery::{
    PhyAuditDiscovery, capture_phy_discovery, discover_phy, phy_audit_citation, phy_audit_lookups,
};
use reliquary_memory_audit::{
    Eligibility, Grade, OwnerCut, Passage, SourceCitation, SourceId, TextField,
};
use reliquary_owner_session::model::MemoryTextField;
use reliquary_owner_session::observation::{ObservationCitation, ObservationId, ObservationSource};
use reliquary_owner_session::{
    MemoryId, PerceptionMemoryEvidence, PerceptionSourceCut, PerceptionSourceGrade,
};
use reliquary_switchboard::{DecisionEndpoint, EmbeddingEndpoint, GeneralEndpoint};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub use reliquary_ego::EgoCandidate;
pub use reliquary_memory_audit::{Feature, ListResult, LookupError, LookupRequest};
pub use reliquary_perception::ObservationProposal;

/// Immutable, nonsecret binding supplied by the configured runtime.
#[derive(Clone)]
pub struct PerceptionRoutes {
    pub general: Arc<dyn GeneralEndpoint>,
    pub contribution: Arc<dyn DecisionEndpoint>,
    pub verification: Arc<dyn DecisionEndpoint>,
    pub embedding: Arc<dyn EmbeddingEndpoint>,
    pub identity: [u8; 32],
    pub embedding_identity: [u8; 32],
}

impl PerceptionRoutes {
    pub fn embedding_profile(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"reliquary-observation-receptor-embedding-v1\0");
        hash.update(self.embedding_identity);
        hash.update(self.embedding.dimensions().to_le_bytes());
        hash.update(self.embedding.model().unwrap_or("").as_bytes());
        hash.finalize().into()
    }
}

pub fn audit_owner_cut(cut: &PerceptionSourceCut) -> OwnerCut {
    OwnerCut {
        owner_id: cut.owner_id,
        history_generation: cut.history_generation,
        frontier: cut.memory,
    }
}

fn owner_citation(source: SourceCitation) -> ObservationCitation {
    ObservationCitation {
        owner_id: source.owner_id,
        history_generation: source.history_generation,
        source: match source.source {
            SourceId::Memory(id) => ObservationSource::Memory(id),
            SourceId::Observation(id) => ObservationSource::Observation(ObservationId(id)),
        },
        revision: source.revision,
        body_id: source.body_id,
        field: match source.field {
            TextField::Title => MemoryTextField::Title,
            TextField::Content => MemoryTextField::Content,
        },
        start_byte: source.start_byte,
        end_byte: source.end_byte,
    }
}

fn audit_citation(source: ObservationCitation) -> SourceCitation {
    SourceCitation {
        owner_id: source.owner_id,
        history_generation: source.history_generation,
        source: match source.source {
            ObservationSource::Memory(id) => SourceId::Memory(id),
            ObservationSource::Observation(id) => SourceId::Observation(id.0),
        },
        revision: source.revision,
        body_id: source.body_id,
        field: match source.field {
            MemoryTextField::Title => TextField::Title,
            MemoryTextField::Content => TextField::Content,
        },
        start_byte: source.start_byte,
        end_byte: source.end_byte,
    }
}

fn memory_passage(
    cut: &PerceptionSourceCut,
    evidence: &PerceptionMemoryEvidence,
    field: MemoryTextField,
    start: usize,
    end: usize,
    sampled: bool,
) -> Option<Passage> {
    let text = match field {
        MemoryTextField::Title => &evidence.title,
        MemoryTextField::Content => &evidence.content,
    }
    .get(start..end)?;
    if text.trim().is_empty() {
        return None;
    }
    let source = evidence.metadata.source;
    let grade = match evidence.metadata.grade {
        PerceptionSourceGrade::Fresh => Grade::Fresh,
        PerceptionSourceGrade::Stale => Grade::Stale,
        PerceptionSourceGrade::Dormant => Grade::Dormant,
    };
    Some(Passage {
        citation: audit_citation(ObservationCitation {
            owner_id: cut.owner_id,
            history_generation: cut.history_generation,
            source: ObservationSource::Memory(source.id.0),
            revision: source.revision,
            body_id: source.body_id.0,
            field,
            start_byte: start as u32,
            end_byte: end as u32,
        }),
        eligible: if !sampled && grade == Grade::Fresh {
            Eligibility::BOTH
        } else {
            Eligibility::PERCEPTION
        },
        grade,
        text: text.to_owned(),
    })
}
