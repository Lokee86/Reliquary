use super::{Error, Result};
use reliquary_memory_audit::{
    Eligibility, Grade, Participant, Passage, SourceCitation, SourceId, TextField,
};
use reliquary_owner_session::model::{EntityId, MemoryId, MemoryTextField};
use reliquary_owner_session::observation::{
    ObservationCitation, ObservationId, ObservationMention, ObservationParticipant,
    ObservationProfile, ObservationSource, ObservationStanding,
};
use reliquary_owner_session::{
    PerceptionMemoryEvidence, PerceptionParticipantEvidence, PerceptionSourceCut,
    PerceptionSourceGrade, RelSession,
};
use reliquary_perception::ClaimParticipant;
use std::collections::{BTreeMap, BTreeSet};

const CONTEXT_BYTES: usize = 192;
const MAX_PROOF_ROWS: usize = 8;
const MAX_PROOF_NODES: usize = 64;

#[derive(Clone)]
pub(super) struct Excerpt {
    pub citation: ObservationCitation,
    pub text: String,
    pub before: String,
    pub after: String,
    pub lineage: [u8; 32],
}

impl Excerpt {
    pub fn bytes(&self) -> usize {
        self.text.len() + self.before.len() + self.after.len()
    }

    pub fn audit(&self, grade: Grade) -> Passage {
        Passage {
            citation: audit_citation(self.citation),
            grade,
            eligible: Eligibility::PERCEPTION,
            text: self.text.clone(),
        }
    }

    pub fn trim_context(&mut self, budget: usize) {
        let before_budget = budget.saturating_sub(self.after.len()).max(budget / 2);
        let mut start = self.before.len().saturating_sub(before_budget);
        while !self.before.is_char_boundary(start) {
            start += 1;
        }
        self.before.drain(..start);
        let mut end = self.after.len().min(budget - self.before.len());
        while !self.after.is_char_boundary(end) {
            end -= 1;
        }
        self.after.truncate(end);
    }
}

pub(super) fn fit_context(rows: &mut [Excerpt], budget: usize) -> Result<()> {
    let quoted = rows.iter().map(|row| row.text.len()).sum::<usize>();
    let mut remaining = budget.checked_sub(quoted).ok_or(Error::EvidenceLimit)?;
    let count = rows.len();
    for (index, row) in rows.iter_mut().enumerate() {
        let allowance = remaining / (count - index);
        if row.before.len() + row.after.len() > allowance {
            row.trim_context(allowance);
        }
        remaining -= row.before.len() + row.after.len();
    }
    Ok(())
}

pub(super) struct Capture<'a> {
    pub rel: &'a RelSession,
    pub cut: PerceptionSourceCut,
    memories: BTreeMap<MemoryId, Option<PerceptionMemoryEvidence>>,
}

impl<'a> Capture<'a> {
    pub fn new(rel: &'a RelSession) -> Self {
        Self {
            rel,
            cut: rel.perception_source_cut(),
            memories: BTreeMap::new(),
        }
    }

    pub fn memory(&mut self, id: MemoryId) -> Result<Option<&PerceptionMemoryEvidence>> {
        if let std::collections::btree_map::Entry::Vacant(entry) = self.memories.entry(id) {
            let result = self
                .rel
                .perception_memory_evidence(&self.cut, &[id])
                .map_err(Error::Owner)?
                .pop();
            entry.insert(result);
        }
        Ok(self.memories[&id].as_ref())
    }

    pub fn citation(&mut self, cite: ObservationCitation) -> Result<(Excerpt, Grade)> {
        if cite.owner_id != self.cut.owner_id
            || cite.history_generation != self.cut.history_generation
        {
            return Err(Error::Stale);
        }
        match cite.source {
            ObservationSource::Memory(id) => {
                let row = self.memory(MemoryId(id))?.ok_or(Error::PendingEvidence)?;
                let record = row.metadata.source;
                if record.revision != cite.revision || record.body_id.0 != cite.body_id {
                    return Err(Error::Stale);
                }
                let grade = match row.metadata.grade {
                    PerceptionSourceGrade::Fresh => Grade::Fresh,
                    PerceptionSourceGrade::Stale => Grade::Stale,
                    PerceptionSourceGrade::Dormant => Grade::Dormant,
                };
                let excerpt = excerpt(cite, text_field(row, cite.field), row.source_lineage)?;
                self.rel
                    .observation_citation_text(&cite)
                    .map_err(Error::Owner)?;
                Ok((excerpt, grade))
            }
            ObservationSource::Observation(id) => {
                let row = self.rel.observation(id).ok_or(Error::Stale)?;
                if row.standing != ObservationStanding::Current {
                    return Err(Error::PendingEvidence);
                }
                let full = row.revision.full_citation();
                if full.revision != cite.revision
                    || full.body_id != cite.body_id
                    || full.field != cite.field
                {
                    return Err(Error::Stale);
                }
                Ok((
                    excerpt(cite, &row.revision.draft.proposition, id.0)?,
                    Grade::Fresh,
                ))
            }
        }
    }

    pub fn proof(&mut self, cite: ObservationCitation) -> Result<Vec<Excerpt>> {
        let mut rows = Vec::new();
        let mut seen = BTreeSet::new();
        self.flatten(cite, &mut seen, &mut rows)?;
        Ok(rows)
    }

    pub fn check_relational_sources(&self, citations: &[ObservationCitation]) -> Result<()> {
        for cite in citations {
            let ObservationSource::Observation(id) = cite.source else {
                continue;
            };
            let source = self.rel.observation(id).ok_or(Error::Stale)?;
            if source.standing != ObservationStanding::Current
                || source.revision.revision != cite.revision
                || source.revision.draft.body_id() != cite.body_id
            {
                return Err(Error::Stale);
            }
            if source.revision.draft.profile != ObservationProfile::Ordinary {
                return Err(Error::InvalidDraft);
            }
        }
        Ok(())
    }

    fn flatten(
        &mut self,
        cite: ObservationCitation,
        seen: &mut BTreeSet<ObservationCitation>,
        rows: &mut Vec<Excerpt>,
    ) -> Result<()> {
        if !seen.insert(cite) {
            return Ok(());
        }
        if seen.len() > MAX_PROOF_NODES {
            return Err(Error::EvidenceLimit);
        }
        match cite.source {
            ObservationSource::Memory(_) => {
                if rows.len() == MAX_PROOF_ROWS {
                    return Err(Error::EvidenceLimit);
                }
                rows.push(self.citation(cite)?.0);
            }
            ObservationSource::Observation(id) => {
                self.citation(cite)?;
                let current = self.rel.observation(id).ok_or(Error::Stale)?;
                for &support in &current.revision.draft.citations {
                    self.flatten(support, seen, rows)?;
                }
            }
        }
        Ok(())
    }

    pub fn anchors(
        &self,
        citations: &[ObservationCitation],
    ) -> Result<BTreeMap<EntityId, Vec<PerceptionParticipantEvidence>>> {
        let mut anchors: BTreeMap<_, Vec<_>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for cite in citations {
            let ObservationSource::Memory(id) = cite.source else {
                continue;
            };
            if !seen.insert(id) {
                continue;
            }
            let source = reliquary_owner_session::PerceptionMemorySource {
                id: MemoryId(id),
                revision: cite.revision,
                body_id: reliquary_owner_session::model::MemoryBodyId(cite.body_id),
            };
            for anchor in self
                .rel
                .perception_participant_evidence(&self.cut, source)
                .map_err(Error::Owner)?
            {
                let current = self.rel.entity(anchor.entity_id).is_some_and(|entity| {
                    entity.revision == anchor.entity_revision
                        && !entity.kind.eq_ignore_ascii_case("generic")
                });
                if current {
                    anchors.entry(anchor.entity_id).or_default().push(anchor);
                }
            }
        }
        Ok(anchors)
    }
}

pub(super) fn participant(owner: [u8; 16], anchor: &PerceptionParticipantEvidence) -> Participant {
    Participant {
        owner_id: owner,
        entity_id: anchor.entity_id.0,
        revision: anchor.entity_revision,
        label: anchor.canonical_name.clone(),
    }
}

pub(super) fn bind_participants(
    participants: &[ClaimParticipant],
    anchors: &BTreeMap<EntityId, Vec<PerceptionParticipantEvidence>>,
) -> Result<Vec<ObservationParticipant>> {
    participants
        .iter()
        .map(|p| {
            let id = EntityId(p.participant.entity_id);
            let evidence = anchors
                .get(&id)
                .and_then(|entries| {
                    entries.iter().find(|entry| {
                        entry.entity_revision == p.participant.revision
                            && entry.canonical_name == p.participant.label
                    })
                })
                .ok_or(Error::PendingEvidence)?;
            Ok(ObservationParticipant {
                entity_id: id,
                revision: evidence.entity_revision,
                role: p.role.clone(),
                mention: ObservationMention {
                    key: evidence.mention,
                    revision: evidence.mention_revision,
                    retained_ordinal: evidence.retained_ordinal,
                },
            })
        })
        .collect()
}

pub(super) fn observation_citation(cite: SourceCitation) -> ObservationCitation {
    ObservationCitation {
        owner_id: cite.owner_id,
        history_generation: cite.history_generation,
        source: match cite.source {
            SourceId::Memory(id) => ObservationSource::Memory(id),
            SourceId::Observation(id) => ObservationSource::Observation(ObservationId(id)),
        },
        revision: cite.revision,
        body_id: cite.body_id,
        field: match cite.field {
            TextField::Title => MemoryTextField::Title,
            TextField::Content => MemoryTextField::Content,
        },
        start_byte: cite.start_byte,
        end_byte: cite.end_byte,
    }
}

pub(super) fn audit_citation(cite: ObservationCitation) -> SourceCitation {
    SourceCitation {
        owner_id: cite.owner_id,
        history_generation: cite.history_generation,
        source: match cite.source {
            ObservationSource::Memory(id) => SourceId::Memory(id),
            ObservationSource::Observation(id) => SourceId::Observation(id.0),
        },
        revision: cite.revision,
        body_id: cite.body_id,
        field: match cite.field {
            MemoryTextField::Title => TextField::Title,
            MemoryTextField::Content => TextField::Content,
        },
        start_byte: cite.start_byte,
        end_byte: cite.end_byte,
    }
}

pub(super) fn text_field(row: &PerceptionMemoryEvidence, field: MemoryTextField) -> &str {
    match field {
        MemoryTextField::Title => &row.title,
        MemoryTextField::Content => &row.content,
    }
}

pub(super) fn excerpt(cite: ObservationCitation, body: &str, lineage: [u8; 32]) -> Result<Excerpt> {
    let start = cite.start_byte as usize;
    let end = cite.end_byte as usize;
    let text = body
        .get(start..end)
        .filter(|text| !text.trim().is_empty())
        .ok_or(Error::Stale)?;
    let mut before = start.saturating_sub(CONTEXT_BYTES);
    while !body.is_char_boundary(before) {
        before += 1;
    }
    let mut after = body.len().min(end.saturating_add(CONTEXT_BYTES));
    while !body.is_char_boundary(after) {
        after -= 1;
    }
    Ok(Excerpt {
        citation: cite,
        text: text.into(),
        before: body[before..start].into(),
        after: body[end..after].into(),
        lineage,
    })
}
