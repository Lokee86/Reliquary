use super::*;
use crate::instance_owner::RelMount;
pub use reliquary_application::ego_context::EgoQueryVector;
use reliquary_application::ego_context::{
    EgoQuery, MAX_EGO_POINT_CANDIDATES, OwnerRank, PackageInput, PhyProfile, Pin, PinKey,
    PinPriority, PointKey, RelPoint, SupportScore, pack,
};
use reliquary_owner_session::{EgoPhyText, EgoPoint, PerceptionSourceCut};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, Serialize)]
pub struct EgoContextBinding {
    pub owner: [u8; 16],
    pub history_generation: [u8; 32],
    pub memory_version: u64,
    pub summary_revision: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct EgoContext {
    pub identity: Option<String>,
    pub standing_context: String,
    pub token_count: usize,
    pub bindings: Vec<EgoContextBinding>,
}

struct ScoredPoint {
    key: PointKey,
    title: String,
    value: EgoPoint,
    quality: f64,
    scores: Vec<SupportScore>,
}

#[derive(Eq, PartialEq)]
struct PhyHead {
    memory_version: u64,
    identity: Option<(EgoIdentityId, u64)>,
    selection: Option<EgoIdentitySelection>,
    pins: Vec<(EgoAnchorId, u64)>,
    profile: Option<u64>,
    summaries: Vec<(OwnerId, Option<u64>)>,
}

struct PhyCapture {
    head: PhyHead,
    identity: Option<EgoIdentity>,
    pins: Vec<EgoAnchor>,
    profile: Option<EgoPhyText>,
    points: Vec<ScoredPoint>,
}

impl Instance {
    /// Read and recheck the mounted owners when preparing the outgoing request.
    pub fn ego_context(
        &self,
        rel: Option<OwnerId>,
        phy: OwnerId,
        query: &str,
        vector: Option<&EgoQueryVector>,
        budget: usize,
        count: impl Fn(&str) -> usize,
    ) -> Result<EgoContext, GatewayError> {
        let ranks = self.ego_rel_ranks(rel)?;
        let phy_mount = self.ego_phy_mount(phy)?;
        let query = EgoQuery::new(query, vector);
        let PhyCapture {
            head,
            identity,
            pins,
            profile,
            points,
        } = self.with_ego_phy(phy, |read| read.context_capture(&ranks, &query))?;
        let mut points_by_rel = BTreeMap::<OwnerId, Vec<ScoredPoint>>::new();
        for point in points {
            points_by_rel
                .entry(OwnerId(point.key.owner))
                .or_default()
                .push(point);
        }
        let mut pins_by_owner = BTreeMap::from([(phy, pins)]);
        let mut points = Vec::new();
        let mut checks = Vec::with_capacity(ranks.len());
        for &owner in ranks.keys() {
            let mount = RelMount::capture(self, owner)?;
            let scoped = points_by_rel.remove(&owner).unwrap_or_default();
            let (cut, pins, points_here) = self.with_reliquary(owner, |read| {
                let pins = read.ego_anchors().cloned().collect::<Vec<_>>();
                let points = scoped
                    .into_iter()
                    .filter_map(|mut point| {
                        point.scores = point
                            .value
                            .citations
                            .iter()
                            .map(|citation| {
                                let score = read.ego_citation_score(citation).ok()??;
                                Some(SupportScore {
                                    memory: citation.memory_id,
                                    score,
                                })
                            })
                            .collect::<Option<Vec<_>>>()?;
                        Some(point)
                    })
                    .collect::<Vec<_>>();
                (read.perception_source_cut(), pins, points)
            })?;
            checks.push((owner, mount, cut, pin_versions(&pins)));
            pins_by_owner.insert(owner, pins);
            points.extend(points_here);
        }
        let ends = profile
            .as_ref()
            .map(|profile| {
                profile
                    .paragraphs
                    .iter()
                    .map(|paragraph| paragraph.end_byte)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let pins = pins_by_owner
            .iter()
            .flat_map(|(&owner, pins)| {
                let owner_rank = if owner == phy {
                    OwnerRank::ActivePhy
                } else {
                    ranks[&owner]
                };
                pins.iter().map(move |pin| Pin {
                    key: PinKey {
                        owner: owner.0,
                        id: pin.id.0,
                    },
                    owner_rank,
                    priority: match pin.priority {
                        EgoAnchorPriority::High => PinPriority::High,
                        EgoAnchorPriority::Normal => PinPriority::Normal,
                        EgoAnchorPriority::Low => PinPriority::Low,
                    },
                    text: &pin.text,
                })
            })
            .collect();
        let input = PackageInput {
            identity: identity.as_ref().map(|identity| identity.text.as_str()),
            pins,
            phy: profile.as_ref().map(|profile| PhyProfile {
                text: &profile.text,
                paragraph_ends: &ends,
            }),
            points: points
                .iter()
                .map(|point| RelPoint {
                    key: point.key,
                    owner_rank: ranks[&OwnerId(point.key.owner)],
                    topic: &point.title,
                    statement: &point.value.statement,
                    match_score: point.quality,
                    supports: &point.scores,
                })
                .collect(),
        };
        let result = pack(input, budget, count);

        if self.ego_rel_ranks(rel)? != ranks
            || !same_phy_mount(&phy_mount, &self.ego_phy_mount(phy)?)
            || self.with_ego_phy(phy, |read| read.context_head(&ranks))? != head
        {
            return Err(GatewayError::StaleRelContext);
        }
        for (owner, mount, cut, pins) in &checks {
            if RelMount::capture(self, *owner)? != *mount
                || !self.with_reliquary(*owner, |read| {
                    read.perception_source_cut() == *cut
                        && read
                            .ego_anchors()
                            .map(|pin| (pin.id, pin.revision))
                            .eq(pins.iter().copied())
                })?
            {
                return Err(GatewayError::StaleRelContext);
            }
        }
        self.ensure_credential_active()?;
        let mut bindings = checks
            .into_iter()
            .map(|(owner, _, cut, _)| {
                let summary_revision = head
                    .summaries
                    .iter()
                    .find(|(source, _)| *source == owner)
                    .and_then(|(_, revision)| *revision);
                context_binding(owner, &cut, summary_revision)
            })
            .collect::<Vec<_>>();
        if let Some(binding) = self.with_ego_phy(phy, |read| read.profile_binding(phy))? {
            if binding.summary_revision != head.profile {
                return Err(GatewayError::StaleRelContext);
            }
            bindings.push(binding);
        }
        Ok(EgoContext {
            identity: result.identity,
            standing_context: result.standing_context,
            token_count: result.token_count,
            bindings,
        })
    }

    fn ego_rel_ranks(
        &self,
        rel: Option<OwnerId>,
    ) -> Result<BTreeMap<OwnerId, OwnerRank>, GatewayError> {
        let Some(rel) = rel else {
            return Ok(BTreeMap::new());
        };
        self.with_rel_dependency_order(rel, |authorized| {
            let authorized: BTreeSet<_> = authorized.into_iter().collect();
            let mut ranks = BTreeMap::new();
            let mut scan = VecDeque::from([(rel, 0_u16)]);
            while let Some((owner, depth)) = scan.pop_front() {
                if !authorized.contains(&owner) || ranks.contains_key(&owner) {
                    continue;
                }
                let rank = if depth == 0 {
                    OwnerRank::ActiveRel
                } else {
                    OwnerRank::Ancestor(depth)
                };
                ranks.insert(owner, rank);
                let ids = self.with_reliquary(owner, RelSession::rel_dependency_ids)??;
                scan.extend(
                    ids.into_iter()
                        .map(|id| (OwnerId(id), depth.saturating_add(1))),
                );
            }
            Ok(ranks)
        })
    }
}

fn same_phy_mount(left: &MountedOwner, right: &MountedOwner) -> bool {
    match (left, right) {
        (MountedOwner::Phy(a), MountedOwner::Phy(b)) => Arc::ptr_eq(a, b),
        (MountedOwner::PhyWriter(a), MountedOwner::PhyWriter(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

fn pin_versions(pins: &[EgoAnchor]) -> Vec<(EgoAnchorId, u64)> {
    pins.iter().map(|pin| (pin.id, pin.revision)).collect()
}

fn context_binding(
    owner: OwnerId,
    cut: &PerceptionSourceCut,
    summary_revision: Option<u64>,
) -> EgoContextBinding {
    EgoContextBinding {
        owner: owner.0,
        history_generation: cut.history_generation,
        memory_version: cut.memory,
        summary_revision,
    }
}

macro_rules! read_phy {
    ($value:expr, |$owner:ident| $body:expr) => {
        match $value {
            EgoPhyRead::Session($owner) => $body,
            EgoPhyRead::Writer($owner) => $body,
        }
    };
}

impl EgoPhyRead<'_> {
    fn context_head(&self, ranks: &BTreeMap<OwnerId, OwnerRank>) -> PhyHead {
        read_phy!(self, |owner| PhyHead {
            memory_version: owner.memory_version(),
            identity: owner
                .ego_active_identity()
                .map(|record| (record.id, record.revision)),
            selection: owner.ego_identity_selection(),
            pins: owner
                .ego_anchors()
                .map(|pin| (pin.id, pin.revision))
                .collect(),
            profile: owner.ego_phy_summary().map(|record| record.revision),
            summaries: ranks
                .keys()
                .map(|id| (
                    *id,
                    owner.ego_rel_summary(id.0).map(|record| record.revision)
                ))
                .collect(),
        })
    }

    fn profile_binding(&self, phy: OwnerId) -> Option<EgoContextBinding> {
        read_phy!(self, |owner| owner.ego_phy_summary().map(|summary| {
            EgoContextBinding {
                owner: phy.0,
                history_generation: summary.completion.cut.history_generation,
                memory_version: summary.completion.cut.memory_version,
                summary_revision: Some(summary.revision),
            }
        }))
    }

    fn context_capture(
        &self,
        ranks: &BTreeMap<OwnerId, OwnerRank>,
        query: &EgoQuery<'_>,
    ) -> PhyCapture {
        let head = self.context_head(ranks);
        read_phy!(self, |owner| {
            let identity = owner.ego_active_identity().cloned();
            let pins = owner.ego_anchors().cloned().collect();
            let profile = owner
                .ego_phy_summary()
                .and_then(|summary| summary.text.as_ref())
                .filter(|text| {
                    text.paragraphs
                        .iter()
                        .flat_map(|p| &p.citations)
                        .all(|citation| owner.ego_citation_current(citation))
                })
                .cloned();
            let mut points = Vec::new();
            for &id in ranks.keys() {
                let Some(summary) = owner.ego_rel_summary(id.0) else {
                    continue;
                };
                for (point_id, point) in &summary.points {
                    let title = summary
                        .topics
                        .get(&point.value.topic_id)
                        .filter(|topic| topic.tombstone.is_none());
                    let Some(title) = title.filter(|_| point.tombstone.is_none()) else {
                        continue;
                    };
                    let vector = point.value.embedding.as_ref().map(|embedding| {
                        (
                            embedding.profile_id,
                            embedding.components().collect::<Vec<_>>(),
                        )
                    });
                    let phrase = format!("{} {}", title.value.title, point.value.statement);
                    let quality = query.score(
                        &phrase,
                        vector
                            .as_ref()
                            .map(|(profile, values)| (*profile, values.as_slice())),
                    );
                    if quality > 0.0 && quality.is_finite() {
                        points.push(ScoredPoint {
                            key: PointKey {
                                owner: id.0,
                                topic: point.value.topic_id.0,
                                id: point_id.0,
                            },
                            title: title.value.title.clone(),
                            value: point.value.clone(),
                            quality,
                            scores: Vec::new(),
                        });
                    }
                }
            }
            points.sort_unstable_by(|a, b| b.quality.total_cmp(&a.quality).then(a.key.cmp(&b.key)));
            points.truncate(MAX_EGO_POINT_CANDIDATES);
            PhyCapture {
                head,
                identity,
                pins,
                profile,
                points,
            }
        })
    }
}
