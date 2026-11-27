use super::*;
use crate::instance_ego::EgoPhyRead;
use reliquary_application::{AgenticMemory, AgenticObservation, inspection::inspection_cut};
use reliquary_owner_session::model::{CompatibilityProfileId, MemoryBody, MemoryBodyId, MemoryId};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationId, ObservationInspection, ObservationMemoryLineage,
    ObservationProfile, ObservationRevision, ObservationSource, ObservationStanding,
    ObservationStatus, ObservationTimeScope, ObservationValidity,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const PAGE_SIZE: usize = 128;

/// Exact verified wording and its proof for one Observation revision.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationClaim {
    pub citation: ObservationCitation,
    pub proposition: String,
    pub profile: ObservationProfile,
    pub status: ObservationStatus,
    pub time_scope: ObservationTimeScope,
    pub validity: Option<ObservationValidity>,
    pub support: Vec<ObservationCitation>,
    pub lineage: Vec<ObservationMemoryLineage>,
}

impl From<&ObservationRevision> for ObservationClaim {
    fn from(revision: &ObservationRevision) -> Self {
        let draft = &revision.draft;
        Self {
            citation: revision.full_citation(),
            proposition: draft.proposition.clone(),
            profile: draft.profile.clone(),
            status: draft.status,
            time_scope: draft.time_scope,
            validity: draft.validity,
            support: draft.citations.clone(),
            lineage: revision.lineage.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct InspectedObservation {
    pub claim: ObservationClaim,
    pub predecessor: Option<u64>,
    pub standing: ObservationStanding,
    pub reviewed_evidence: Vec<ObservationCitation>,
    pub triggering_evidence: Vec<ObservationCitation>,
}

impl From<ObservationInspection<'_>> for InspectedObservation {
    fn from(item: ObservationInspection<'_>) -> Self {
        Self {
            claim: item.revision.into(),
            predecessor: item.revision.predecessor,
            standing: item.standing,
            reviewed_evidence: item.revision.draft.reviewed.clone(),
            triggering_evidence: item.triggering_evidence,
        }
    }
}

fn project_inspection(
    read: ObservationRead<'_>,
    item: ObservationInspection<'_>,
) -> InspectedObservation {
    let eligible = read.source_eligible(item.revision.id, item.revision.revision);
    let mut result = InspectedObservation::from(item);
    if !eligible {
        result.claim.proposition.clear();
        if let ObservationProfile::Relational(participants) = &mut result.claim.profile {
            for participant in participants {
                participant.role = None;
            }
        }
    }
    result
}

/// A continuation bound to the owner, history, and dependency state of a page.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ObservationCursor<K> {
    cut: ObservationReadCut,
    after: K,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ObservationPage<K> {
    pub items: Vec<InspectedObservation>,
    pub next: Option<ObservationCursor<K>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct ObservationReadCut {
    owner_id: [u8; 16],
    generation: [u8; 32],
    memory: u64,
    perception: u64,
    #[serde(default)]
    source_removal: u64,
    pub(crate) publication: u64,
}

impl ObservationReadCut {
    pub(crate) fn capture(rel: &RelSession) -> Self {
        let cut = inspection_cut(&rel.management_read());
        Self {
            owner_id: cut.owner_id,
            generation: cut.generation,
            memory: cut.memory,
            perception: cut.perception,
            source_removal: rel.archive_source_removal_version(),
            publication: rel.perception_review_version(),
        }
    }

    pub(crate) fn contains(self, cite: &ObservationCitation) -> bool {
        cite.owner_id == self.owner_id
            && cite.history_generation == self.generation
            && matches!(cite.source, ObservationSource::Observation(_))
            && cite.revision > 0
            && cite.start_byte == 0
            && cite.end_byte > 0
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ObservationRead<'a> {
    Rel(&'a RelSession),
    Phy(&'a PhySession),
    PhyWriter(&'a PhyWriter),
}

macro_rules! read_observations {
    ($read:expr, $method:ident($($arg:expr),* $(,)?)) => {
        match $read {
            ObservationRead::Rel(owner) => owner.$method($($arg),*),
            ObservationRead::Phy(owner) => owner.$method($($arg),*),
            ObservationRead::PhyWriter(owner) => owner.$method($($arg),*),
        }
    };
}

impl<'a> ObservationRead<'a> {
    fn cut(self) -> Result<ObservationReadCut, GatewayError> {
        let (read, publication) = match self {
            Self::Rel(rel) => return Ok(ObservationReadCut::capture(rel)),
            Self::Phy(phy) => (phy.management_read(), phy.perception_review_version()),
            Self::PhyWriter(phy) => (phy.management_read()?, phy.perception_review_version()),
        };
        let cut = inspection_cut(&read);
        Ok(ObservationReadCut {
            owner_id: cut.owner_id,
            generation: cut.generation,
            memory: cut.memory,
            perception: cut.perception,
            source_removal: 0,
            publication,
        })
    }

    fn source_eligible(self, id: ObservationId, revision: u64) -> bool {
        read_observations!(self, observation_revision_source_eligible(id, revision))
    }

    pub(crate) fn citation_text(
        self,
        cite: &ObservationCitation,
    ) -> Result<String, OwnerSessionError> {
        read_observations!(self, observation_citation_text(cite))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RetrievalObservation {
    pub claim: ObservationClaim,
    pub standing: ObservationStanding,
    pub lexical_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub combined_score: Option<f64>,
    pub freshness: i16,
}

impl Instance {
    pub(crate) fn with_observation_read<T>(
        &self,
        owner: OwnerId,
        phy: bool,
        operation: impl FnOnce(ObservationRead<'_>) -> T,
    ) -> Result<T, GatewayError> {
        if phy {
            self.with_ego_phy(owner, |read| {
                operation(match read {
                    EgoPhyRead::Session(phy) => ObservationRead::Phy(phy),
                    EgoPhyRead::Writer(phy) => ObservationRead::PhyWriter(phy),
                })
            })
        } else {
            self.with_reliquary(owner, |rel| operation(ObservationRead::Rel(rel)))
        }
    }

    pub fn inspect_observation(
        &self,
        owner: OwnerId,
        id: ObservationId,
        revision: Option<u64>,
    ) -> Result<Option<InspectedObservation>, GatewayError> {
        let result =
            self.with_observation_read(owner, self.phy_grants.contains(&owner), |read| {
                let item = match revision {
                    Some(revision) => read_observations!(read, observation_revision(id, revision)),
                    None => read_observations!(read, observation(id)),
                };
                item.map(|item| project_inspection(read, item))
            })?;
        self.ensure_credential_active()?;
        Ok(result)
    }

    pub fn list_observations(
        &self,
        owner: OwnerId,
        cursor: Option<&ObservationCursor<ObservationId>>,
        limit: usize,
    ) -> Result<ObservationPage<ObservationId>, GatewayError> {
        let page = self.with_observation_read(
            owner,
            self.phy_grants.contains(&owner),
            |read| -> Result<_, GatewayError> {
                let cut = page_cut(read, cursor, limit)?;
                let rows = read_observations!(
                    read,
                    observations_after(cursor.map(|cursor| cursor.after), limit)
                );
                let next = rows.last().and_then(|last| {
                    let after = last.revision.id;
                    (!read_observations!(read, observations_after(Some(after), 1)).is_empty())
                        .then_some(ObservationCursor { cut, after })
                });
                Ok(ObservationPage {
                    items: rows
                        .into_iter()
                        .map(|row| project_inspection(read, row))
                        .collect(),
                    next,
                })
            },
        )??;
        self.ensure_credential_active()?;
        Ok(page)
    }

    pub fn observation_history(
        &self,
        owner: OwnerId,
        id: ObservationId,
        cursor: Option<&ObservationCursor<(ObservationId, u64)>>,
        limit: usize,
    ) -> Result<ObservationPage<(ObservationId, u64)>, GatewayError> {
        let page =
            self.with_observation_read(owner, self.phy_grants.contains(&owner), |read| {
                let cut = page_cut(read, cursor, limit)?;
                if cursor.is_some_and(|cursor| cursor.after.0 != id) {
                    return Err(GatewayError::OwnerSession(
                        OwnerSessionError::ManagementStale,
                    ));
                }
                let rows = read_observations!(
                    read,
                    observation_revisions_after(
                        id,
                        cursor.map_or(0, |cursor| cursor.after.1),
                        limit,
                    )
                );
                let next = rows.last().and_then(|last| {
                    let revision = last.revision.revision;
                    (!read_observations!(read, observation_revisions_after(id, revision, 1))
                        .is_empty())
                    .then_some(ObservationCursor {
                        cut,
                        after: (id, revision),
                    })
                });
                Ok(ObservationPage {
                    items: rows
                        .into_iter()
                        .map(|row| project_inspection(read, row))
                        .collect(),
                    next,
                })
            })??;
        self.ensure_credential_active()?;
        Ok(page)
    }

    /// Read an exact cited or reviewed passage from an inspected Observation revision.
    pub fn inspect_observation_evidence(
        &self,
        owner: OwnerId,
        id: ObservationId,
        revision: u64,
        cite: ObservationCitation,
    ) -> Result<Option<String>, GatewayError> {
        let text = self.with_observation_read(owner, self.phy_grants.contains(&owner), |read| {
            let row = read_observations!(read, observation_revision(id, revision))?;
            let included = row.revision.draft.citations.contains(&cite)
                || row.revision.draft.reviewed.contains(&cite)
                || row.triggering_evidence.contains(&cite);
            included
                .then(|| read_observations!(read, historical_observation_citation_text(&cite)).ok())
                .flatten()
        })?;
        self.ensure_credential_active()?;
        Ok(text)
    }
}

fn page_cut<K>(
    read: ObservationRead<'_>,
    cursor: Option<&ObservationCursor<K>>,
    limit: usize,
) -> Result<ObservationReadCut, GatewayError> {
    if !(1..=PAGE_SIZE).contains(&limit) {
        return Err(GatewayError::Management(
            reliquary_application::ManagementError::InvalidPage,
        ));
    }
    let cut = read.cut()?;
    if cursor.is_some_and(|cursor| cursor.cut != cut) {
        return Err(GatewayError::OwnerSession(
            OwnerSessionError::ManagementStale,
        ));
    }
    Ok(cut)
}

pub(crate) fn observations_are_current(rel: &RelSession, hits: &[RetrievalObservation]) -> bool {
    hits.iter().all(|hit| {
        rel.observation_citation_text(&hit.claim.citation)
            .is_ok_and(|text| text == hit.claim.proposition)
    })
}

/// Check standing before scoring. Retain only a bounded ranked result across keyset pages.
#[derive(Clone, PartialEq)]
pub(crate) struct ObservationRetrievalCandidate {
    pub(crate) input: AgenticObservation,
    pub(crate) result: RetrievalObservation,
}

/// A private retrieval key. It is never returned as a Memory ID or graph seed.
pub(crate) fn observation_retrieval_id(cite: &ObservationCitation) -> MemoryId {
    let mut key = Sha256::new();
    key.update(b"reliquary/observation-retrieval/v1\0");
    key.update(cite.owner_id);
    key.update(cite.history_generation);
    match cite.source {
        ObservationSource::Memory(id) => key.update(id),
        ObservationSource::Observation(id) => key.update(id.0),
    }
    key.update(cite.revision.to_le_bytes());
    key.update(cite.body_id);
    MemoryId(key.finalize().into())
}

/// Project verified heads into the normal retrieval population. The typed result
/// remains bound to its canonical claim and citations.
pub(crate) fn observation_retrieval_candidates(
    rel: &RelSession,
    profile: Option<CompatibilityProfileId>,
    excluded: &BTreeSet<ObservationCitation>,
) -> Result<BTreeMap<MemoryId, ObservationRetrievalCandidate>, GatewayError> {
    observation_candidates(ObservationRead::Rel(rel), profile, excluded)
}

pub(crate) fn observation_candidates(
    read: ObservationRead<'_>,
    profile: Option<CompatibilityProfileId>,
    excluded: &BTreeSet<ObservationCitation>,
) -> Result<BTreeMap<MemoryId, ObservationRetrievalCandidate>, GatewayError> {
    let mut after = None;
    let mut candidates = BTreeMap::new();
    loop {
        let rows = read_observations!(read, observations_after(after, PAGE_SIZE));
        let Some(last) = rows.last() else { break };
        after = Some(last.revision.id);
        for row in &rows {
            if row.standing != ObservationStanding::Current
                || excluded.contains(&row.revision.full_citation())
            {
                continue;
            }
            let claim = ObservationClaim::from(row.revision);
            let id = observation_retrieval_id(&claim.citation);
            let freshness = match read {
                ObservationRead::Rel(rel) => {
                    rel.observation_freshness(row.revision.id).unwrap_or(0)
                }
                _ => 0,
            };
            let vector = profile
                .map(|profile| {
                    read_observations!(read, observation_claim_vector(profile, row.revision))
                })
                .transpose()?
                .flatten();
            let input = AgenticObservation {
                candidate: AgenticMemory {
                    id,
                    revision: claim.citation.revision,
                    body_id: MemoryBodyId(claim.citation.body_id),
                    body: MemoryBody {
                        title: String::new(),
                        content: claim.proposition.clone(),
                    },
                    freshness,
                    vector,
                },
                validity: claim.validity,
            };
            let result = RetrievalObservation {
                claim,
                standing: row.standing,
                lexical_score: 0.0,
                semantic_score: None,
                combined_score: None,
                freshness,
            };
            candidates.insert(id, ObservationRetrievalCandidate { input, result });
        }
        if rows.len() < PAGE_SIZE {
            break;
        }
    }
    Ok(candidates)
}
