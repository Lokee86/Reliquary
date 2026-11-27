use super::*;
use reliquary_application::ego_context::{SupportScore, eligible_freshness};
use reliquary_owner_session::{
    EgoCitation, EgoItemChange, EgoPhySummary, EgoPointId, EgoPointStatus, EgoRelChange,
    EgoRelPublication, EgoRelSummary, EgoTimeScope, EgoTombstone, EgoTopicId,
};

#[derive(Clone, Serialize)]
pub struct EgoRelInspection {
    pub revision: u64,
    pub topics: Vec<EgoTopicInspection>,
    pub points: Vec<EgoPointInspection>,
}

#[derive(Clone, Serialize)]
pub struct EgoTopicInspection {
    pub id: EgoTopicId,
    pub revision: u64,
    pub title: Option<String>,
    pub tombstone: Option<EgoTombstone>,
}

#[derive(Clone, Serialize)]
pub struct EgoPointInspection {
    pub id: EgoPointId,
    pub topic: EgoTopicId,
    pub revision: u64,
    pub statement: Option<String>,
    pub status: EgoPointStatus,
    pub time_scope: EgoTimeScope,
    pub citations: Vec<EgoCitation>,
    pub current: bool,
    pub eligible_for_context: bool,
    pub tombstone: Option<EgoTombstone>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum EgoSummaryItemId {
    Topic(EgoTopicId),
    Point(EgoPointId),
}

#[derive(Clone, Deserialize, Serialize)]
pub enum EgoSummaryEdit {
    Withdraw {
        item: EgoSummaryItemId,
        revision: u64,
        suppress: bool,
    },
    LiftSuppression {
        item: EgoSummaryItemId,
        revision: u64,
    },
    ReplacePoint {
        id: EgoPointId,
        revision: u64,
        statement: String,
        status: EgoPointStatus,
        time_scope: EgoTimeScope,
        citations: Vec<EgoCitation>,
    },
}

impl Instance {
    pub fn inspect_ego_phy_summary(
        &self,
        phy: OwnerId,
        revision: Option<u64>,
    ) -> Result<Option<EgoPhySummary>, GatewayError> {
        self.with_ego_phy(phy, |read| match read {
            EgoPhyRead::Session(owner) => redact_phy(
                match revision {
                    Some(revision) => owner.ego_phy_summary_revision(revision),
                    None => owner.ego_phy_summary(),
                },
                |citation| match revision {
                    Some(_) => owner.historical_ego_citation_text(citation).is_ok(),
                    None => owner.ego_citation_current(citation),
                },
            ),
            EgoPhyRead::Writer(owner) => redact_phy(
                match revision {
                    Some(revision) => owner.ego_phy_summary_revision(revision),
                    None => owner.ego_phy_summary(),
                },
                |citation| match revision {
                    Some(_) => owner.historical_ego_citation_text(citation).is_ok(),
                    None => owner.ego_citation_current(citation),
                },
            ),
        })
    }

    pub fn inspect_ego_rel_summary(
        &self,
        phy: OwnerId,
        source: OwnerId,
        revision: Option<u64>,
    ) -> Result<Option<EgoRelInspection>, GatewayError> {
        self.authorize_rel_access(source)?;
        let summary = self.with_ego_phy(phy, |read| read.rel_summary(source, revision))??;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let inspection =
            self.with_reliquary(source, |owner| {
                let active_topics = summary
                    .topics
                    .iter()
                    .filter(|(_, item)| item.tombstone.is_none())
                    .map(|(&id, _)| id)
                    .collect::<BTreeSet<_>>();
                let topics = summary
                    .topics
                    .into_iter()
                    .map(|(id, item)| {
                        let readable =
                            item.value.references.iter().any(|citation| {
                                owner.historical_ego_citation_text(citation).is_ok()
                            });
                        EgoTopicInspection {
                            id,
                            revision: item.revision,
                            title: readable.then_some(item.value.title),
                            tombstone: item.tombstone,
                        }
                    })
                    .collect();
                let points = summary
                    .points
                    .into_iter()
                    .map(|(id, item)| {
                        let standing = item.tombstone.is_none()
                            && active_topics.contains(&item.value.topic_id)
                            && item
                                .value
                                .citations
                                .iter()
                                .all(|citation| owner.ego_citation_current(citation));
                        let scores = standing
                            .then(|| {
                                item.value
                                    .citations
                                    .iter()
                                    .map(|citation| {
                                        Some(SupportScore {
                                            memory: citation.memory_id,
                                            score: owner.ego_citation_score(citation).ok()??,
                                        })
                                    })
                                    .collect::<Option<Vec<_>>>()
                            })
                            .flatten();
                        let readable =
                            item.value.citations.iter().all(|citation| {
                                owner.historical_ego_citation_text(citation).is_ok()
                            });
                        EgoPointInspection {
                            id,
                            topic: item.value.topic_id,
                            revision: item.revision,
                            statement: readable.then_some(item.value.statement),
                            status: item.value.status,
                            time_scope: item.value.time_scope,
                            citations: item.value.citations,
                            current: standing && revision.is_none(),
                            eligible_for_context: revision.is_none()
                                && scores.as_deref().is_some_and(eligible_freshness),
                            tombstone: item.tombstone,
                        }
                    })
                    .collect();
                EgoRelInspection {
                    revision: summary.revision,
                    topics,
                    points,
                }
            })?;
        self.authorize_ego_phy(phy)?;
        self.authorize_rel_access(source)?;
        Ok(Some(inspection))
    }

    /// Apply an explicit Inspector edit after checking both owners and the
    /// user's expected summary/item revision.
    pub fn edit_ego_rel_summary(
        &self,
        phy: OwnerId,
        source: OwnerId,
        expected_summary_revision: u64,
        edit: EgoSummaryEdit,
    ) -> Result<u64, GatewayError> {
        let _topology = self
            .rel_topology
            .read()
            .map_err(|_| GatewayError::RegistryPoisoned)?;
        self.with_reliquary(source, |rel| {
            self.with_ego_phy_writer_under_topology(phy, |writer| {
                let summary = writer
                    .ego_rel_summary(source.0)
                    .filter(|summary| summary.revision == expected_summary_revision)
                    .ok_or(OwnerSessionError::ManagementStale)?;
                let change = match edit {
                    EgoSummaryEdit::Withdraw {
                        item: EgoSummaryItemId::Topic(id),
                        revision,
                        suppress,
                    } => EgoRelChange::Topic {
                        id,
                        expected_revision: revision,
                        change: EgoItemChange::Tombstone { suppress },
                    },
                    EgoSummaryEdit::Withdraw {
                        item: EgoSummaryItemId::Point(id),
                        revision,
                        suppress,
                    } => EgoRelChange::Point {
                        id,
                        expected_revision: revision,
                        change: EgoItemChange::Tombstone { suppress },
                    },
                    EgoSummaryEdit::LiftSuppression {
                        item: EgoSummaryItemId::Topic(id),
                        revision,
                    } => EgoRelChange::Topic {
                        id,
                        expected_revision: revision,
                        change: EgoItemChange::LiftSuppression,
                    },
                    EgoSummaryEdit::LiftSuppression {
                        item: EgoSummaryItemId::Point(id),
                        revision,
                    } => EgoRelChange::Point {
                        id,
                        expected_revision: revision,
                        change: EgoItemChange::LiftSuppression,
                    },
                    EgoSummaryEdit::ReplacePoint {
                        id,
                        revision,
                        statement,
                        status,
                        time_scope,
                        citations,
                    } => {
                        let old = summary
                            .points
                            .get(&id)
                            .filter(|item| item.tombstone.is_none() && item.revision == revision)
                            .ok_or(OwnerSessionError::ManagementStale)?;
                        if citations.is_empty()
                            || !citations
                                .iter()
                                .all(|citation| rel.ego_citation_current(citation))
                        {
                            return Err(OwnerSessionError::ManagementEvidence);
                        }
                        let mut value = old.value.clone();
                        if value.statement != statement {
                            value.embedding = None;
                        }
                        value.statement = statement;
                        value.status = status;
                        value.time_scope = time_scope;
                        value.citations = citations;
                        EgoRelChange::Point {
                            id,
                            expected_revision: revision,
                            change: EgoItemChange::Put(value),
                        }
                    }
                };
                writer.publish_ego_rel_summary(EgoRelPublication {
                    source_owner_id: source.0,
                    expected_revision: expected_summary_revision,
                    completion: None,
                    changes: vec![change],
                })
            })
        })?
    }
}

fn redact_phy(
    summary: Option<&EgoPhySummary>,
    current: impl Fn(&EgoCitation) -> bool,
) -> Option<EgoPhySummary> {
    let mut summary = summary?.clone();
    if summary.text.as_ref().is_some_and(|text| {
        text.paragraphs
            .iter()
            .flat_map(|p| &p.citations)
            .any(|citation| !current(citation))
    }) {
        summary.text = None;
    }
    Some(summary)
}

impl EgoPhyRead<'_> {
    fn rel_summary(
        self,
        source: OwnerId,
        revision: Option<u64>,
    ) -> Result<Option<EgoRelSummary>, OwnerSessionError> {
        match (self, revision) {
            (Self::Session(owner), Some(revision)) => {
                owner.ego_rel_summary_revision(source.0, revision)
            }
            (Self::Writer(owner), Some(revision)) => {
                owner.ego_rel_summary_revision(source.0, revision)
            }
            (Self::Session(owner), None) => Ok(owner.ego_rel_summary(source.0).cloned()),
            (Self::Writer(owner), None) => Ok(owner.ego_rel_summary(source.0).cloned()),
        }
    }
}
