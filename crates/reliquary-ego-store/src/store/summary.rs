use super::*;
use crate::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) enum ExtendedRecord {
    IdentitySelection(EgoIdentitySelection),
    Rel(EgoRelPublication),
    Phy(EgoPhyPublication),
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct SummaryProjection {
    selection: Option<EgoIdentitySelection>,
    rel: BTreeMap<[u8; 16], EgoRelSummary>,
    phy: BTreeMap<u64, EgoPhySummary>,
    completions: BTreeMap<[u8; 32], (u64, EgoCompletion)>,
    batches: BTreeMap<[u8; 32], [u8; 32]>,
}

enum Accepted {
    Selection(EgoIdentitySelection),
    Rel([u8; 16], EgoRelSummary, Option<EgoCompletion>),
    Phy(EgoPhySummary),
}

impl EgoStore {
    pub fn identity_selection(&self) -> Option<EgoIdentitySelection> {
        self.summaries.selection
    }

    pub fn active_identity(&self) -> Option<&EgoIdentity> {
        self.identity(self.identity_selection()?.id?)
    }

    pub fn select_identity(
        &mut self,
        container: &mut Container,
        expected_selection_revision: u64,
        target: Option<(EgoIdentityId, u64)>,
    ) -> Result<EgoIdentitySelection, EgoError> {
        self.require_private_owner(container, "Identity selection")?;
        let previous = self.identity_selection();
        let actual = previous.map_or(0, |selection| selection.revision);
        check_revision(expected_selection_revision, actual)?;
        if let Some((id, expected_revision)) = target {
            let current = self.identity(id).ok_or(EgoError::NotFound("Identity"))?;
            check_revision(expected_revision, current.revision)?;
        }
        let id = target.map(|(id, _)| id);
        if let Some(selection) = previous.filter(|selection| selection.id == id) {
            return Ok(selection);
        }
        let selection = EgoIdentitySelection {
            revision: next_revision(actual)?,
            id,
        };
        self.append_extended(container, ExtendedRecord::IdentitySelection(selection))?;
        Ok(selection)
    }

    pub fn rel_summary(&self, owner_id: [u8; 16]) -> Option<&EgoRelSummary> {
        self.summaries.rel.get(&owner_id)
    }

    pub fn rel_summary_revision(
        &self,
        owner_id: [u8; 16],
        revision: u64,
    ) -> Result<Option<EgoRelSummary>, EgoError> {
        let Some(current) = self.rel_summary(owner_id) else {
            return Ok(None);
        };
        if revision == 0 || revision > current.revision {
            return Ok(None);
        }
        if revision == current.revision {
            return Ok(Some(current.clone()));
        }
        let mut summary = EgoRelSummary::default();
        for record in &self.records {
            if let EgoRecord::Extended {
                record: ExtendedRecord::Rel(publication),
                ..
            } = record
                && publication.source_owner_id == owner_id
            {
                summary.apply(publication)?;
                if summary.revision == revision {
                    return Ok(Some(summary));
                }
            }
        }
        Err(EgoError::InvalidRecord(
            "retained REL summary revision is unavailable",
        ))
    }

    pub fn has_extended_records(&self) -> bool {
        self.summaries.selection.is_some()
            || !self.summaries.phy.is_empty()
            || !self.summaries.rel.is_empty()
    }

    pub fn phy_publication_in_frame(payload: &[u8]) -> Result<Option<EgoPhyPublication>, EgoError> {
        match EgoRecord::decode(payload)? {
            Some(EgoRecord::Extended {
                record: ExtendedRecord::Phy(publication),
                ..
            }) => Ok(Some(publication)),
            _ => Ok(None),
        }
    }

    pub fn phy_publications(&self) -> impl Iterator<Item = &EgoPhyPublication> {
        self.records.iter().filter_map(|record| match record {
            EgoRecord::Extended {
                record: ExtendedRecord::Phy(publication),
                ..
            } => Some(publication),
            _ => None,
        })
    }

    pub fn phy_summary(&self) -> Option<&EgoPhySummary> {
        self.summaries
            .phy
            .last_key_value()
            .map(|(_, summary)| summary)
    }

    pub fn phy_summary_revision(&self, revision: u64) -> Option<&EgoPhySummary> {
        self.summaries.phy.get(&revision)
    }

    pub fn completion(&self, id: [u8; 32]) -> Option<&EgoCompletion> {
        self.summaries
            .completions
            .get(&id)
            .map(|(_, completion)| completion)
    }

    pub fn completion_for_batch(&self, batch_id: [u8; 32]) -> Option<&EgoCompletion> {
        self.completion(*self.summaries.batches.get(&batch_id)?)
    }

    pub fn publish_rel_summary(
        &mut self,
        container: &mut Container,
        publication: EgoRelPublication,
    ) -> Result<u64, EgoError> {
        self.require_private_owner(container, "REL summary")?;
        let id = publication
            .completion
            .as_ref()
            .map(|completion| completion.id);
        self.append_summary(container, ExtendedRecord::Rel(publication), id)
    }

    pub fn publish_phy_summary(
        &mut self,
        container: &mut Container,
        publication: EgoPhyPublication,
    ) -> Result<u64, EgoError> {
        self.require_private_owner(container, "PHY summary")?;
        let id = publication.completion.id;
        self.append_summary(container, ExtendedRecord::Phy(publication), Some(id))
    }

    fn append_summary(
        &mut self,
        container: &mut Container,
        record: ExtendedRecord,
        id: Option<[u8; 32]>,
    ) -> Result<u64, EgoError> {
        if let Some((version, _)) = id.and_then(|id| self.summaries.completions.get(&id)) {
            let index = usize::try_from(version - 1).map_err(|_| EgoError::VersionExhausted)?;
            if let Some(EgoRecord::Extended {
                record: previous, ..
            }) = self.records.get(index)
                && *previous == record
            {
                return record.summary_revision();
            }
            return Err(EgoError::InvalidRecord("completed Ego work ID was reused"));
        }
        let revision = record.summary_revision()?;
        self.append_extended(container, record)?;
        Ok(revision)
    }

    fn append_extended(
        &mut self,
        container: &mut Container,
        record: ExtendedRecord,
    ) -> Result<(), EgoError> {
        let version = self.next_version()?;
        let accepted = self.validate_extended(&record)?;
        let wrapped = EgoRecord::Extended { version, record };
        container.append(&wrapped.encode()?)?;
        container.sync()?;
        self.summaries.apply(version, accepted);
        self.version = version;
        self.records.push(wrapped);
        Ok(())
    }

    pub(super) fn apply_extended(
        &mut self,
        version: u64,
        record: &ExtendedRecord,
    ) -> Result<(), EgoError> {
        let accepted = self.validate_extended(record)?;
        self.summaries.apply(version, accepted);
        Ok(())
    }

    fn validate_extended(&self, record: &ExtendedRecord) -> Result<Accepted, EgoError> {
        match record {
            ExtendedRecord::IdentitySelection(selection) => {
                validate_revision(
                    self.identity_selection().map(|previous| previous.revision),
                    selection.revision,
                )?;
                if selection.id.is_some_and(|id| self.identity(id).is_none()) {
                    return Err(EgoError::NotFound("Identity"));
                }
                Ok(Accepted::Selection(*selection))
            }
            ExtendedRecord::Rel(publication) => {
                if self.owner_id == Some(publication.source_owner_id) {
                    return Err(EgoError::InvalidRecord(
                        "REL summary source is the destination PHY",
                    ));
                }
                let mut summary = self
                    .rel_summary(publication.source_owner_id)
                    .cloned()
                    .unwrap_or_default();
                if let Some(completion) = &publication.completion {
                    self.summaries.validate_completion(
                        completion,
                        publication.source_owner_id,
                        summary.completion.as_ref(),
                    )?;
                } else if publication.changes.is_empty() {
                    return Err(EgoError::InvalidRecord(
                        "REL summary publication has no changes or completion",
                    ));
                }
                summary.apply(publication)?;
                Ok(Accepted::Rel(
                    publication.source_owner_id,
                    summary,
                    publication.completion.clone(),
                ))
            }
            ExtendedRecord::Phy(publication) => {
                let owner_id = self.owner_id.ok_or(EgoError::OwnerIdentityMismatch)?;
                let current = self.phy_summary();
                check_revision(
                    publication.expected_revision,
                    current.map_or(0, |summary| summary.revision),
                )?;
                self.summaries.validate_completion(
                    &publication.completion,
                    owner_id,
                    current.map(|summary| &summary.completion),
                )?;
                let text = match &publication.replacement {
                    Some(replacement) => {
                        replacement.validate(publication.completion.cut)?;
                        (!replacement.text.is_empty()).then(|| replacement.clone())
                    }
                    None => current.and_then(|summary| summary.text.clone()),
                };
                Ok(Accepted::Phy(EgoPhySummary {
                    revision: record.summary_revision()?,
                    completion: publication.completion.clone(),
                    text,
                }))
            }
        }
    }
}

impl ExtendedRecord {
    fn summary_revision(&self) -> Result<u64, EgoError> {
        match self {
            Self::Rel(publication) => next_revision(publication.expected_revision),
            Self::Phy(publication) => next_revision(publication.expected_revision),
            Self::IdentitySelection(_) => {
                Err(EgoError::InvalidRecord("expected summary publication"))
            }
        }
    }
}

impl SummaryProjection {
    fn validate_completion(
        &self,
        completion: &EgoCompletion,
        owner_id: [u8; 16],
        previous: Option<&EgoCompletion>,
    ) -> Result<(), EgoError> {
        let cut = completion.cut;
        if cut.owner_id != owner_id {
            return Err(EgoError::InvalidRecord("completion source owner mismatch"));
        }
        if self.completions.contains_key(&completion.id) {
            return Err(EgoError::InvalidRecord("completed Ego work ID was reused"));
        }
        let mut unique = BTreeSet::new();
        if completion
            .batch_ids
            .iter()
            .any(|id| !unique.insert(id) || self.batches.contains_key(id))
        {
            return Err(EgoError::InvalidRecord("completed Ego batch ID was reused"));
        }
        if let Some(previous) = previous.map(|completion| completion.cut)
            && previous.history_generation == cut.history_generation
            && (cut.memory_version < previous.memory_version
                || cut.audit_frontier < previous.audit_frontier)
        {
            return Err(EgoError::InvalidRecord("completion cut moved backwards"));
        }
        Ok(())
    }

    fn apply(&mut self, version: u64, accepted: Accepted) {
        let completion = match accepted {
            Accepted::Selection(selection) => {
                self.selection = Some(selection);
                None
            }
            Accepted::Rel(owner_id, summary, completion) => {
                self.rel.insert(owner_id, summary);
                completion
            }
            Accepted::Phy(summary) => {
                let completion = summary.completion.clone();
                self.phy.insert(summary.revision, summary);
                Some(completion)
            }
        };
        if let Some(completion) = completion {
            self.batches.extend(
                completion
                    .batch_ids
                    .iter()
                    .map(|batch_id| (*batch_id, completion.id)),
            );
            self.completions
                .insert(completion.id, (version, completion));
        }
    }
}

impl EgoRelSummary {
    fn apply(&mut self, publication: &EgoRelPublication) -> Result<(), EgoError> {
        check_revision(publication.expected_revision, self.revision)?;
        let mut topics = BTreeSet::new();
        let mut points = BTreeSet::new();
        let generation = publication
            .completion
            .as_ref()
            .or(self.completion.as_ref())
            .map(|completion| completion.cut.history_generation);
        for change in &publication.changes {
            match change {
                EgoRelChange::Topic {
                    id,
                    expected_revision,
                    change,
                } => {
                    if !topics.insert(*id) {
                        return Err(EgoError::InvalidRecord("duplicate topic change"));
                    }
                    if let EgoItemChange::Put(value) = change {
                        validate_text(&value.title, "Topic")?;
                        validate_embedding(value.embedding.as_ref())?;
                        if !value.references.is_empty() {
                            validate_citations(
                                &value.references,
                                publication.source_owner_id,
                                generation,
                            )?;
                        }
                    }
                    let item = change_item(self.topics.get(id), *expected_revision, change)?;
                    self.topics.insert(*id, item);
                    if matches!(change, EgoItemChange::Tombstone { .. }) {
                        for (point_id, point) in &mut self.points {
                            if point.value.topic_id == *id && point.tombstone.is_none() {
                                if !points.insert(*point_id) {
                                    return Err(EgoError::InvalidRecord("duplicate point change"));
                                }
                                point.revision = next_revision(point.revision)?;
                                point.tombstone = Some(EgoTombstone { suppressed: false });
                            }
                        }
                    }
                }
                EgoRelChange::Point {
                    id,
                    expected_revision,
                    change,
                } => {
                    if !points.insert(*id) {
                        return Err(EgoError::InvalidRecord("duplicate point change"));
                    }
                    let previous = self.points.get(id);
                    if let EgoItemChange::Put(value) = change {
                        if !self
                            .topics
                            .get(&value.topic_id)
                            .is_some_and(|topic| topic.tombstone.is_none())
                            || previous.is_some_and(|point| point.value.topic_id != value.topic_id)
                        {
                            return Err(EgoError::InvalidRecord(
                                "point requires its original live topic",
                            ));
                        }
                        validate_text(&value.statement, "Point")?;
                        validate_embedding(value.embedding.as_ref())?;
                        validate_citations(
                            &value.citations,
                            publication.source_owner_id,
                            generation,
                        )?;
                    }
                    let item = change_item(previous, *expected_revision, change)?;
                    self.points.insert(*id, item);
                }
            }
        }
        self.revision = next_revision(self.revision)?;
        if let Some(completion) = &publication.completion {
            self.completion = Some(completion.clone());
        }
        Ok(())
    }
}

fn change_item<T: Clone>(
    previous: Option<&EgoSummaryItem<T>>,
    expected_revision: u64,
    change: &EgoItemChange<T>,
) -> Result<EgoSummaryItem<T>, EgoError> {
    let actual = previous.map_or(0, |item| item.revision);
    check_revision(expected_revision, actual)?;
    let revision = next_revision(actual)?;
    match (previous, change) {
        (None, EgoItemChange::Put(value))
        | (
            Some(EgoSummaryItem {
                tombstone: None, ..
            }),
            EgoItemChange::Put(value),
        ) => Ok(EgoSummaryItem {
            revision,
            value_revision: revision,
            value: value.clone(),
            tombstone: None,
        }),
        (
            Some(
                item @ EgoSummaryItem {
                    tombstone: None, ..
                },
            ),
            EgoItemChange::Tombstone { suppress },
        ) => Ok(EgoSummaryItem {
            revision,
            tombstone: Some(EgoTombstone {
                suppressed: *suppress,
            }),
            ..item.clone()
        }),
        (
            Some(
                item @ EgoSummaryItem {
                    tombstone: Some(EgoTombstone { suppressed: true }),
                    ..
                },
            ),
            EgoItemChange::LiftSuppression,
        ) => Ok(EgoSummaryItem {
            revision,
            tombstone: Some(EgoTombstone { suppressed: false }),
            ..item.clone()
        }),
        _ => Err(EgoError::InvalidRecord("invalid summary item transition")),
    }
}

fn validate_embedding(embedding: Option<&EgoEmbedding>) -> Result<(), EgoError> {
    if let Some(embedding) = embedding
        && (embedding.components.is_empty()
            || embedding.components().any(|value| !value.is_finite()))
    {
        return Err(EgoError::InvalidRecord(
            "embedding must have finite components",
        ));
    }
    Ok(())
}

fn validate_citations(
    citations: &[EgoCitation],
    owner_id: [u8; 16],
    generation: Option<[u8; 32]>,
) -> Result<(), EgoError> {
    let mut unique = BTreeSet::new();
    let expected_generation = generation.or_else(|| {
        citations
            .first()
            .map(|citation| citation.history_generation)
    });
    if citations.is_empty()
        || citations.iter().any(|citation| {
            citation.owner_id != owner_id
                || Some(citation.history_generation) != expected_generation
                || citation.revision == 0
                || citation.start_byte >= citation.end_byte
                || !unique.insert(citation)
        })
    {
        return Err(EgoError::InvalidRecord("invalid summary citations"));
    }
    Ok(())
}

impl EgoPhyText {
    fn validate(&self, cut: EgoSourceCut) -> Result<(), EgoError> {
        if self.text.is_empty() && self.paragraphs.is_empty() && self.token_count == 0 {
            return Ok(());
        }
        validate_text(&self.text, "PHY summary")?;
        if self.token_count == 0 || self.token_count > MAX_EGO_PHY_SUMMARY_TOKENS {
            return Err(EgoError::InvalidRecord(
                "PHY summary token count exceeds its cap",
            ));
        }
        let mut previous_end = 0;
        for paragraph in &self.paragraphs {
            let start = paragraph.start_byte as usize;
            let end = paragraph.end_byte as usize;
            let gap = self.text.get(previous_end..start);
            let paragraph_text = self.text.get(start..end);
            if !gap.is_some_and(|text| text.trim().is_empty())
                || !paragraph_text.is_some_and(|text| !text.trim().is_empty())
            {
                return Err(EgoError::InvalidRecord(
                    "invalid PHY summary paragraph range",
                ));
            }
            validate_citations(
                &paragraph.citations,
                cut.owner_id,
                Some(cut.history_generation),
            )?;
            previous_end = end;
        }
        if !self.text[previous_end..].trim().is_empty() {
            return Err(EgoError::InvalidRecord(
                "PHY summary contains unverified text",
            ));
        }
        Ok(())
    }
}
