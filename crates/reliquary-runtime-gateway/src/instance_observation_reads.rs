use super::*;
use reliquary_application::{ObservationLexicalQuery, TemporalFilter, inspection::inspection_cut};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationId, ObservationInspection, ObservationMemoryLineage,
    ObservationProfile, ObservationRevision, ObservationSource, ObservationStanding,
    ObservationStatus, ObservationTimeScope, ObservationValidity,
};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RetrievalObservation {
    pub claim: ObservationClaim,
    pub standing: ObservationStanding,
    pub lexical_score: f64,
    pub freshness: i16,
}

impl Instance {
    pub fn inspect_observation(
        &self,
        owner: OwnerId,
        id: ObservationId,
        revision: Option<u64>,
    ) -> Result<Option<InspectedObservation>, GatewayError> {
        let result = self.with_reliquary(owner, |rel| match revision {
            Some(revision) => rel.observation_revision(id, revision).map(Into::into),
            None => rel.observation(id).map(Into::into),
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
        let page = self.with_reliquary(owner, |rel| -> Result<_, GatewayError> {
            let cut = page_cut(rel, cursor, limit)?;
            let rows = rel.observations_after(cursor.map(|cursor| cursor.after), limit);
            let next = rows.last().and_then(|last| {
                let after = last.revision.id;
                (!rel.observations_after(Some(after), 1).is_empty())
                    .then_some(ObservationCursor { cut, after })
            });
            Ok(ObservationPage {
                items: rows.into_iter().map(Into::into).collect(),
                next,
            })
        })??;
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
        let page = self.with_reliquary(owner, |rel| {
            let cut = page_cut(rel, cursor, limit)?;
            if cursor.is_some_and(|cursor| cursor.after.0 != id) {
                return Err(GatewayError::OwnerSession(
                    OwnerSessionError::ManagementStale,
                ));
            }
            let rows = rel.observation_revisions_after(
                id,
                cursor.map_or(0, |cursor| cursor.after.1),
                limit,
            );
            let next = rows.last().and_then(|last| {
                let revision = last.revision.revision;
                (!rel.observation_revisions_after(id, revision, 1).is_empty()).then_some(
                    ObservationCursor {
                        cut,
                        after: (id, revision),
                    },
                )
            });
            Ok(ObservationPage {
                items: rows.into_iter().map(Into::into).collect(),
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
        let text = self.with_reliquary(owner, |rel| {
            let row = rel.observation_revision(id, revision)?;
            let included = row.revision.draft.citations.contains(&cite)
                || row.revision.draft.reviewed.contains(&cite)
                || row.triggering_evidence.contains(&cite);
            included
                .then(|| rel.historical_observation_citation_text(&cite).ok())
                .flatten()
        })?;
        self.ensure_credential_active()?;
        Ok(text)
    }
}

fn page_cut<K>(
    rel: &RelSession,
    cursor: Option<&ObservationCursor<K>>,
    limit: usize,
) -> Result<ObservationReadCut, GatewayError> {
    if !(1..=PAGE_SIZE).contains(&limit) {
        return Err(GatewayError::Management(
            reliquary_application::ManagementError::InvalidPage,
        ));
    }
    let cut = ObservationReadCut::capture(rel);
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
pub(crate) fn rank_current_observations(
    rel: &RelSession,
    query: &str,
    filter: Option<TemporalFilter>,
    excluded: &BTreeSet<ObservationCitation>,
    limit: usize,
) -> (Vec<RetrievalObservation>, bool) {
    let query = ObservationLexicalQuery::new(query, filter);
    let mut after = None;
    let mut hits = Vec::new();
    let mut truncated = false;
    loop {
        let rows = rel.observations_after(after, PAGE_SIZE);
        let Some(last) = rows.last() else { break };
        after = Some(last.revision.id);
        let more = rows.len() == PAGE_SIZE;
        for row in rows {
            if row.standing != ObservationStanding::Current
                || excluded.contains(&row.revision.full_citation())
            {
                continue;
            }
            let Some(lexical_score) = query.score(&row.revision.draft) else {
                continue;
            };
            hits.push(RetrievalObservation {
                claim: row.revision.into(),
                standing: row.standing,
                lexical_score,
                freshness: rel.observation_freshness(row.revision.id).unwrap_or(0),
            });
        }
        hits.sort_by(|left, right| {
            right
                .lexical_score
                .total_cmp(&left.lexical_score)
                .then_with(|| right.freshness.cmp(&left.freshness))
                .then_with(|| left.claim.citation.cmp(&right.claim.citation))
        });
        truncated |= hits.len() > limit;
        hits.truncate(limit);
        if !more {
            break;
        }
    }
    (hits, truncated)
}
