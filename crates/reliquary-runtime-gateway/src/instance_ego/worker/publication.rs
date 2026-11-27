use super::{discovery::PhyDiscovery, *};
use reliquary_application::{
    ego_pending::ego_as_of,
    ego_phy::{capture_phy_summary, prepare_pending_phy_publication, revalidate_phy_summary},
    ego_rel_workflow::{
        RelEgoRoutes, RelEgoWork, capture_rel_ego_evidence, prepare_rel_ego_publication,
    },
};
use reliquary_owner_session::{EgoItemChange, EgoRelChange, PhyAuditCut};

impl EgoWorker {
    pub(super) fn publish_rel(
        &self,
        owner: OwnerId,
        run: EgoRun,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        let (complete, prior) = self
            .instance
            .with_ego_phy_writer(self.phy, |phy| {
                Ok((
                    phy.ego_completion(run.id()).is_some(),
                    phy.ego_rel_summary(owner.0).cloned(),
                ))
            })
            .map_err(as_error)?;
        if complete {
            pending.clear_completed(run).map_err(as_error)?;
            return Ok(true);
        }
        if run.kind == EgoRunKind::RelIndependentRecurring
            && self.shared_perception_available(owner)
        {
            pending.discard_stale(run).map_err(as_error)?;
            return Ok(true);
        }
        if !self.accepted_run_current(owner, run, pending)? {
            return Ok(true);
        }
        if prior
            .as_ref()
            .and_then(|summary| summary.completion.as_ref())
            .is_some_and(|previous| {
                previous.cut.history_generation == run.source.history_generation
                    && previous.cut.memory_version >= run.memory_version
                    && previous.cut.audit_frontier >= run.source.frontier
            })
        {
            pending.discard_stale(run).map_err(as_error)?;
            return Ok(true);
        }
        let embedding = self
            .routes
            .embedding
            .as_deref()
            .ok_or("Ego REL embedding route unavailable")?;
        let batches = collect_batches(pending, run)?;
        let prepared = self
            .instance
            .with_reliquary(owner, |rel| {
                capture_rel_ego_evidence(rel, run, &batches, prior.as_ref(), 16 * 1024 * 1024)
            })
            .map_err(as_error)?
            .map_err(as_error)?;
        let started_ns = pending.started_ns(run).map_err(as_error)?;
        let routes = RelEgoRoutes {
            general: self.routes.general.as_ref(),
            embedding,
            identity: self.routes.identity,
            embedding_profile: self.routes.embedding_profile,
            max_request_bytes: limits().max_input_bytes,
        };
        let publication = prepare_rel_ego_publication(
            &routes,
            RelEgoWork {
                run,
                batches: &batches,
                prior: prior.as_ref(),
                evidence: &prepared,
                as_of: &ego_as_of(started_ns),
                verified_at_ns: started_ns,
            },
            pending,
        )
        .map_err(as_error)?;

        self.still_authorized(owner).map_err(as_error)?;
        if run.kind == EgoRunKind::RelIndependentRecurring
            && self.shared_perception_available(owner)
        {
            pending.discard_stale(run).map_err(as_error)?;
            return Ok(true);
        }
        let _topology = self
            .instance
            .rel_topology
            .read()
            .map_err(|_| "Ego owner topology unavailable")?;
        self.instance
            .with_reliquary(owner, |rel| {
                let source = rel.perception_source_cut();
                if source.history_generation != run.source.history_generation
                    || source.memory < run.memory_version
                    || source.settlements < run.source.frontier
                {
                    return Err(GatewayError::StaleRelContext);
                }
                for change in &publication.changes {
                    let (citations, retained): (&[_], &[_]) = match change {
                        EgoRelChange::Point {
                            change: EgoItemChange::Put(point),
                            ..
                        } => (&point.citations, &[]),
                        EgoRelChange::Topic {
                            id,
                            change: EgoItemChange::Put(topic),
                            ..
                        } => {
                            let covered = prior
                                .as_ref()
                                .and_then(|summary| summary.topics.get(id))
                                .map(|item| item.value.references.as_slice())
                                .unwrap_or_default();
                            (&topic.references, covered)
                        }
                        _ => continue,
                    };
                    for citation in citations
                        .iter()
                        .filter(|citation| !retained.contains(citation))
                    {
                        if rel
                            .ego_citation_score(citation)?
                            .is_none_or(|score| score < 1)
                        {
                            return Err(GatewayError::StaleRelContext);
                        }
                    }
                }
                self.still_authorized(owner)?;
                if run.kind == EgoRunKind::RelIndependentRecurring
                    && self.shared_perception_available(owner)
                {
                    return Err(GatewayError::StaleRelContext);
                }
                self.instance
                    .with_ego_phy_writer_under_topology(self.phy, |phy| {
                        if !self.running.load(Ordering::Acquire) {
                            return Err(OwnerSessionError::ManagementStale);
                        }
                        phy.publish_ego_rel_summary(publication)
                    })?;
                Ok(())
            })
            .map_err(as_error)?
            .map_err(as_error)?;
        pending.clear_completed(run).map_err(as_error)?;
        Ok(true)
    }

    pub(super) fn publish_phy(
        &self,
        run: EgoRun,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        let complete = self
            .instance
            .with_ego_phy_writer(self.phy, |phy| Ok(phy.ego_completion(run.id()).is_some()))
            .map_err(as_error)?;
        if complete {
            pending.clear_completed(run).map_err(as_error)?;
            return Ok(true);
        }
        if !self.accepted_run_current(self.phy, run, pending)? {
            return Ok(true);
        }
        let walk: PhyDiscovery = pending
            .checkpoint(run)
            .map_err(as_error)?
            .ok_or("Ego PHY run cut missing")?;
        let batches = collect_batches(pending, run)?;
        let done = completion(run, &batches);
        let candidates = batches
            .into_iter()
            .flat_map(|batch| batch.candidates)
            .collect::<Vec<_>>();
        let started_ns = pending.started_ns(run).map_err(as_error)?;
        let capture = self
            .instance
            .with_ego_phy_writer(self.phy, |phy| {
                Ok(capture_phy_summary(
                    phy,
                    walk.cut,
                    done,
                    &candidates,
                    &walk.changes,
                    started_ns,
                ))
            })
            .map_err(as_error)?;
        let work = match capture {
            Ok(work) => work,
            Err(error) => {
                if self.recapture_changed_phy(run, walk.cut, pending)? {
                    return Ok(true);
                }
                return Err(as_error(error));
            }
        };
        let Some(publication) = prepare_pending_phy_publication(
            &work,
            run,
            pending,
            self.routes.general.as_ref(),
            self.routes.count_tokens.as_ref(),
            limits().max_input_bytes,
        )
        .map_err(as_error)?
        else {
            return self.recapture_changed_phy(run, walk.cut, pending);
        };
        if publication.replacement.as_ref().is_some_and(|text| {
            (self.routes.count_tokens)(&text.text) != u64::from(text.token_count)
        }) {
            return Err("Ego PHY summary token count changed before publication".into());
        }
        self.still_authorized(self.phy).map_err(as_error)?;
        let committed = self.instance.with_ego_phy_writer(self.phy, |phy| {
            if !self.running.load(Ordering::Acquire) {
                return Err(OwnerSessionError::ManagementStale);
            }
            revalidate_phy_summary(phy, &work).map_err(|_| OwnerSessionError::ManagementStale)?;
            phy.publish_ego_phy_summary(publication)
        });
        if let Err(error) = committed {
            if matches!(
                error,
                GatewayError::OwnerSession(OwnerSessionError::ManagementStale)
            ) && self.recapture_changed_phy(run, walk.cut, pending)?
            {
                return Ok(true);
            }
            return Err(as_error(error));
        }
        pending.clear_completed(run).map_err(as_error)?;
        Ok(true)
    }

    fn recapture_changed_phy(
        &self,
        run: EgoRun,
        captured: PhyAuditCut,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        let current = self
            .instance
            .with_ego_phy_writer(self.phy, |phy| Ok(phy.phy_audit_cut()))
            .map_err(as_error)?;
        if current.memory != captured.memory
            || current.settlements != captured.settlements
            || current.history_generation != captured.history_generation
        {
            pending.discard_stale(run).map_err(as_error)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn accepted_run_current(
        &self,
        owner: OwnerId,
        run: EgoRun,
        pending: &mut EgoPendingWork,
    ) -> Result<bool, String> {
        let generation = if owner == self.phy {
            self.instance
                .with_ego_phy_writer(self.phy, |phy| Ok(phy.phy_audit_cut().history_generation))
                .map_err(as_error)?
        } else {
            self.instance
                .with_reliquary(owner, |rel| rel.perception_source_cut().history_generation)
                .map_err(as_error)?
        };
        if generation != run.source.history_generation {
            pending.discard_stale(run).map_err(as_error)?;
            return Ok(false);
        }
        if run.contract != CONTRACT_VERSION {
            pending.discard_stale(run).map_err(as_error)?;
            if owner != self.phy {
                self.advance_rel_bootstrap(owner, pending)?;
            }
            return Ok(false);
        }
        if run.route != self.routes.identity {
            let batches = collect_batches(pending, run)?;
            let rebound = EgoRun {
                route: self.routes.identity,
                contract: CONTRACT_VERSION,
                ..run
            };
            let mut checkpoint = pending
                .checkpoint::<serde_json::Value>(run)
                .map_err(as_error)?;
            if let Some(value) = checkpoint.take() {
                pending.save_checkpoint(rebound, &value).map_err(as_error)?;
            }
            for batch in batches {
                let key = EgoBatchKey {
                    capture: batch.id,
                    ordinal: 0,
                };
                pending
                    .accept_batch(rebound, key, &batch.candidates)
                    .map_err(as_error)?;
            }
            pending.seal(rebound).map_err(as_error)?;
            pending.discard_stale(run).map_err(as_error)?;
            return Ok(false);
        }
        Ok(true)
    }
}
