use super::*;
use reliquary_owner_session::model::MemoryId;
use reliquary_owner_session::observation::{
    ObservationChange, ObservationComparison, ObservationComparisonOutcome,
    ObservationPendingReason, ObservationSource, ObservationWithdrawalReason,
};

pub(super) struct ReviewFence {
    id: ObservationId,
    revision: u64,
    body_id: [u8; 32],
    standing: ObservationStanding,
    trigger_evidence: Vec<ObservationCitation>,
    support: Vec<(ObservationCitation, Option<ObservationCitation>)>,
    participants: Vec<(EntityId, Option<u64>)>,
}

impl ReviewFence {
    pub fn revalidate(&self, capture: &mut Capture<'_>) -> Result<()> {
        let head = capture.rel.observation(self.id).ok_or(Error::Stale)?;
        if head.revision.revision != self.revision
            || head.revision.draft.body_id() != self.body_id
            || head.standing != self.standing
            || head.triggering_evidence != self.trigger_evidence
        {
            return Err(Error::Stale);
        }
        for &(prior, expected) in &self.support {
            if resolve(capture, prior, &head.revision.draft.proposition)? != expected {
                return Err(Error::Stale);
            }
        }
        for &(entity, revision) in &self.participants {
            if capture.rel.entity(entity).map(|row| row.revision) != revision {
                return Err(Error::Stale);
            }
        }
        Ok(())
    }
}

struct ReviewInput {
    head: ObservationRevision,
    fence: ReviewFence,
    synthesis: SynthesisInput,
    prior: SynthesisDraft,
    unchanged: bool,
    required: Vec<ObservationCitation>,
    initial_judgments: Vec<RevisionEvidence>,
}

pub fn evaluate_review(
    read: &impl ScopedObservationRead,
    routes: &ObservationDraftRoutes<'_>,
    id: ObservationId,
) -> Result<Option<PreparedObservation>> {
    let Some(mut work) = read.read(|rel| capture_review(rel, routes, id))? else {
        return Ok(None);
    };
    let mut draft = if work.unchanged {
        work.prior.clone()
    } else if work.synthesis.source_slots.is_empty()
        || (work.synthesis.proposal.profile == reliquary_perception::ObservationProfile::Relational
            && work.synthesis.proposal.participants.len() < 2)
    {
        return withdraw(work, routes, ObservationWithdrawalReason::LostSupport, None).map(Some);
    } else {
        let context = RevisionContext {
            prior: work.prior.clone(),
            backcheck: work.initial_judgments.clone(),
        };
        match work.synthesis.run(routes, Some(&context))? {
            SynthesisDecision::Draft(draft) => draft,
            SynthesisDecision::Abstained => {
                return withdraw(work, routes, ObservationWithdrawalReason::LostSupport, None)
                    .map(Some);
            }
            SynthesisDecision::Duplicate { .. } => return Err(Error::PendingEvidence),
        }
    };
    let mut evidence_change = change_from(work.initial_judgments.iter().map(|hit| hit.disposition));
    let mut previous: Option<(SynthesisDraft, Vec<ObservationReceptor>)> = None;
    for attempt in 0..2 {
        let original = draft.id == work.prior.id;
        let mut canonical = if original {
            work.head.draft.clone()
        } else {
            read.read(|rel| canonical_claim(&mut Capture::new(rel), &draft.claim))?
        };
        if !original {
            let reuse = previous
                .as_ref()
                .map(|(draft, rows)| (draft, rows.as_slice(), routes.route_identity))
                .or(Some((
                    &work.prior,
                    &work.head.draft.receptors,
                    work.head.policy.general_route,
                )));
            canonical.receptors = receptors(routes, &draft, reuse)?;
        }
        let frozen = read.read(|rel| {
            work.synthesis.check_source_cut(rel)?;
            let mut capture = Capture::new(rel);
            work.fence.revalidate(&mut capture)?;
            freeze(&mut capture, canonical, &work.required, routes)
        })?;
        let decision = verify(routes.verification, &frozen.state).map_err(Error::Native)?;
        let detected = change_from(decision.hits.values().copied());
        match decision.verdict {
            VerificationVerdict::StandsAsWritten => {
                return finish(work, routes, draft.id, frozen, evidence_change).map(Some);
            }
            VerificationVerdict::NeedsRevision if attempt == 0 => {
                evidence_change = strongest(evidence_change, detected);
                work.required.extend(material_refs(&frozen, &decision));
                work.required.sort_unstable();
                work.required.dedup();
                if !original {
                    previous = Some((draft.clone(), frozen.draft.receptors.clone()));
                }
                let revision =
                    revision_input(read, &mut work.synthesis, draft, &frozen, &decision)?;
                draft = match work.synthesis.run(routes, Some(&revision))? {
                    SynthesisDecision::Draft(draft) => draft,
                    SynthesisDecision::Abstained => {
                        return withdraw(
                            work,
                            routes,
                            ObservationWithdrawalReason::UnresolvedEvidence,
                            Some(frozen),
                        )
                        .map(Some);
                    }
                    SynthesisDecision::Duplicate { .. } => return Err(Error::PendingEvidence),
                };
            }
            _ => {
                return withdraw(
                    work,
                    routes,
                    ObservationWithdrawalReason::UnresolvedEvidence,
                    Some(frozen),
                )
                .map(Some);
            }
        }
    }
    unreachable!("a bounded review yields a disposition")
}

fn capture_review(
    rel: &RelSession,
    routes: &ObservationDraftRoutes<'_>,
    id: ObservationId,
) -> Result<Option<ReviewInput>> {
    let Some(head) = rel.observation(id) else {
        return Ok(None);
    };
    let ObservationStanding::Pending(reason) = head.standing else {
        return Ok(None);
    };
    if reason == ObservationPendingReason::UpstreamReview && head.triggering_evidence.is_empty() {
        return Ok(None);
    }
    let mut capture = Capture::new(rel);
    let cut = OwnerCut {
        owner_id: capture.cut.owner_id,
        history_generation: capture.cut.history_generation,
        frontier: capture.cut.memory,
    };
    let mut support = BTreeMap::new();
    let head_draft = &head.revision.draft;
    for &cite in head_draft.citations.iter().chain(&head.triggering_evidence) {
        if let std::collections::btree_map::Entry::Vacant(entry) = support.entry(cite) {
            entry.insert(resolve(&mut capture, cite, &head_draft.proposition)?);
        }
    }
    let mut current: Vec<_> = head_draft
        .citations
        .iter()
        .filter_map(|cite| support[cite])
        .collect();
    current.sort_unstable();
    current.dedup();
    let mut required: Vec<_> = head
        .triggering_evidence
        .iter()
        .filter_map(|cite| support[cite])
        .collect();
    required.sort_unstable();
    required.dedup();
    let mut prior = semantic_claim(rel, head.revision);
    let prior_id = digest(json!([
        "reliquary-observation-review-head-v1",
        existing_ref(head.revision)
    ]));
    let prior_draft = SynthesisDraft {
        id: prior_id,
        claim: prior.clone(),
    };
    let anchors = capture.anchors(&current)?;
    let exact_participants = head_draft.profile.participants().iter().all(|p| {
        anchors.get(&p.entity_id).is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry.entity_revision == p.revision
                    && entry.mention == p.mention.key
                    && entry.mention_revision == p.mention.revision
                    && entry.retained_ordinal == p.mention.retained_ordinal
            })
        })
    });
    let unchanged = exact_participants
        && head_draft
            .citations
            .iter()
            .all(|cite| support[cite] == Some(*cite));
    let mut all_current = current.clone();
    all_current.extend(required.iter().copied());
    all_current.sort_unstable();
    all_current.dedup();
    let mut alternatives = Vec::new();
    if !unchanged && all_current.len() < MAX_DRAFT_SLICES {
        let mut original = Vec::new();
        for &cite in &all_current {
            original.push(capture.citation(cite)?.0);
        }
        alternatives = backcheck::optional_hits(
            &mut capture,
            &head_draft.proposition,
            &head_draft.receptors,
            routes.search_profile(&head_draft.receptors),
            &original,
            Vec::new(),
        )?;
        all_current.extend(alternatives.iter().map(|row| row.citation));
    }
    if !unchanged && !prior.participants.is_empty() {
        let anchors = capture.anchors(&all_current)?;
        prior.participants.retain_mut(|p| {
            let Some(evidence) = anchors
                .get(&EntityId(p.participant.entity_id))
                .and_then(|entries| entries.first())
            else {
                return false;
            };
            p.participant = participant(cut.owner_id, evidence);
            true
        });
    }
    prior.citations = current.iter().copied().map(audit_citation).collect();
    let mut synthesis = SynthesisInput {
        source_removal: capture.cut.source_removal,
        binding: StageBinding {
            cut,
            proposal_id: prior_id,
            route_identity: routes.route_identity,
        },
        participants: prior
            .participants
            .iter()
            .map(|p| p.participant.clone())
            .collect(),
        proposal: prior,
        source_slots: Vec::new(),
        existing: Vec::new(),
    };
    // The unchanged head can be verified first even when a later revision would
    // need to narrow its admitted synthesis sources. Required hits still enter Verify.
    for &cite in &current {
        if !unchanged {
            let (row, grade) = capture.citation(cite)?;
            synthesis.add(row.audit(grade))?;
        } else {
            let (row, grade) = capture.citation(cite)?;
            synthesis.source_slots.push(row.audit(grade));
        }
    }
    if !unchanged {
        for &cite in &required {
            let (row, grade) = capture.citation(cite)?;
            synthesis.add(row.audit(grade))?;
        }
        for alternate in alternatives {
            let (row, grade) = capture.citation(alternate.citation)?;
            if let Err(Error::EvidenceLimit) = synthesis.add(row.audit(grade)) {
                break;
            }
        }
    }
    let initial_judgments = head
        .triggering_evidence
        .iter()
        .filter_map(|original| {
            let cite = support[original]?;
            let outcome = rel.observation_comparison(&ObservationComparison {
                id,
                target_revision: head.revision.revision,
                citation: *original,
                outcome: ObservationComparisonOutcome::NoMaterialEffect,
            });
            Some(RevisionEvidence {
                citation: audit_citation(cite),
                disposition: match outcome {
                    Some(ObservationComparisonOutcome::MaterialChallenge) => {
                        HitVerdict::MaterialChallenge
                    }
                    Some(ObservationComparisonOutcome::MaterialQualification) => {
                        HitVerdict::MaterialQualification
                    }
                    Some(ObservationComparisonOutcome::NoMaterialEffect) => {
                        HitVerdict::NoMaterialEffect
                    }
                    _ => HitVerdict::WeakOrUncertain,
                },
            })
        })
        .collect();
    let fence = ReviewFence {
        id,
        revision: head.revision.revision,
        body_id: head_draft.body_id(),
        standing: head.standing,
        trigger_evidence: head.triggering_evidence,
        support: support.into_iter().collect(),
        participants: head_draft
            .profile
            .participants()
            .iter()
            .map(|p| {
                (
                    p.entity_id,
                    rel.entity(p.entity_id).map(|entity| entity.revision),
                )
            })
            .collect(),
    };
    Ok(Some(ReviewInput {
        head: head.revision.clone(),
        fence,
        synthesis,
        prior: prior_draft,
        unchanged,
        required,
        initial_judgments,
    }))
}

fn resolve(
    capture: &mut Capture<'_>,
    cite: ObservationCitation,
    query: &str,
) -> Result<Option<ObservationCitation>> {
    match cite.source {
        ObservationSource::Observation(id) => {
            let row = capture.rel.observation(id).ok_or(Error::PendingEvidence)?;
            match row.standing {
                ObservationStanding::Current if row.revision.revision == cite.revision => {
                    capture.citation(cite)?;
                    Ok(Some(cite))
                }
                ObservationStanding::Current => Ok(Some(row.revision.full_citation())),
                ObservationStanding::Withdrawn(_) => Ok(None),
                _ => Err(Error::PendingEvidence),
            }
        }
        ObservationSource::Memory(id) => {
            let mut current = MemoryId(id);
            let owner = capture.rel.management_read();
            for _ in 0..4 {
                let Ok(record) = owner.memory_record(current, None) else {
                    return if owner.memory_terminal(current, capture.cut.memory).is_some() {
                        Ok(None)
                    } else {
                        Err(Error::PendingEvidence)
                    };
                };
                if let Some(successor) = record.superseded_by {
                    current = successor;
                    continue;
                }
                if record.archived {
                    return Ok(None);
                }
                if current.0 == id
                    && record.revision == cite.revision
                    && record.body_id.0 == cite.body_id
                {
                    capture.citation(cite)?;
                    return Ok(Some(cite));
                }
                return Ok(Some(
                    backcheck::window_citation(capture, current, query)?
                        .ok_or(Error::PendingEvidence)?,
                ));
            }
            Err(Error::EvidenceLimit)
        }
    }
}

pub(super) fn semantic_claim(rel: &RelSession, row: &ObservationRevision) -> ProposedClaim {
    let draft = &row.draft;
    let profile = match draft.profile {
        ObservationProfile::Ordinary => reliquary_perception::ObservationProfile::Ordinary,
        ObservationProfile::Relational(_) => reliquary_perception::ObservationProfile::Relational,
    };
    let participants = draft
        .profile
        .participants()
        .iter()
        .map(|p| ClaimParticipant {
            participant: Participant {
                owner_id: row.owner_id,
                entity_id: p.entity_id.0,
                revision: p.revision,
                label: rel
                    .entity(p.entity_id)
                    .map(|row| row.canonical_name.clone())
                    .unwrap_or_default(),
            },
            role: p.role.clone(),
        })
        .collect();
    ProposedClaim {
        proposition: draft.proposition.clone(),
        profile,
        participants,
        status: match draft.status {
            ObservationStatus::Observed => Status::Observed,
            ObservationStatus::Planned => Status::Planned,
            ObservationStatus::Requested => Status::Requested,
            ObservationStatus::Possible => Status::Possible,
            ObservationStatus::Expected => Status::Expected,
        },
        time_scope: match draft.time_scope {
            ObservationTimeScope::Past => TimeScope::Past,
            ObservationTimeScope::Current => TimeScope::Current,
            ObservationTimeScope::Future => TimeScope::Future,
            ObservationTimeScope::Unspecified => TimeScope::Unspecified,
        },
        citations: draft
            .citations
            .iter()
            .copied()
            .map(audit_citation)
            .collect(),
    }
}

fn finish(
    work: ReviewInput,
    routes: &ObservationDraftRoutes<'_>,
    draft_id: [u8; 32],
    mut frozen: FrozenDraft,
    detected: ObservationChange,
) -> Result<PreparedObservation> {
    let id = work.head.id;
    let expected_revision = work.head.revision;
    let invalidated = invalidated(&work.fence);
    frozen.draft.reviewed = reviewed(&[&frozen.draft.reviewed, &invalidated])?;
    let operation = if work.head.draft.same_verified_claim(&frozen.draft) {
        ObservationOperation::Reaffirm {
            id,
            expected_revision,
            reviewed: frozen.draft.reviewed,
        }
    } else {
        let change = if work.head.draft.proposition == frozen.draft.proposition
            && work.head.draft.time_scope == frozen.draft.time_scope
            && work.head.draft.validity == frozen.draft.validity
        {
            ObservationChange::Equivalent
        } else {
            detected
        };
        ObservationOperation::Successor {
            id,
            expected_revision,
            draft: frozen.draft,
            change,
            credited_activity: None,
        }
    };
    let mut current = frozen.current;
    current.extend(
        work.synthesis
            .source_slots
            .iter()
            .map(|source| observation_citation(source.citation)),
    );
    Ok(prepared(
        routes,
        work.synthesis.binding.cut,
        work.synthesis.source_removal,
        operation,
        draft_id,
        current,
        Some(work.fence),
    ))
}

fn withdraw(
    work: ReviewInput,
    routes: &ObservationDraftRoutes<'_>,
    reason: ObservationWithdrawalReason,
    frozen: Option<FrozenDraft>,
) -> Result<PreparedObservation> {
    let invalidated = invalidated(&work.fence);
    let evidence = frozen
        .as_ref()
        .map_or(&[][..], |draft| draft.draft.reviewed.as_slice());
    let reviewed = reviewed(&[evidence, &invalidated, &work.fence.trigger_evidence])?;
    let operation = ObservationOperation::Withdraw {
        id: work.head.id,
        expected_revision: work.head.revision,
        reason,
        reviewed,
    };
    let mut current = frozen.map_or_else(Vec::new, |draft| draft.current);
    current.extend(
        work.synthesis
            .source_slots
            .iter()
            .map(|source| observation_citation(source.citation)),
    );
    Ok(prepared(
        routes,
        work.synthesis.binding.cut,
        work.synthesis.source_removal,
        operation,
        work.prior.id,
        current,
        Some(work.fence),
    ))
}

fn invalidated(fence: &ReviewFence) -> Vec<ObservationCitation> {
    fence
        .support
        .iter()
        .filter_map(|(prior, current)| (*current != Some(*prior)).then_some(*prior))
        .collect()
}

fn change_from(hits: impl Iterator<Item = HitVerdict>) -> ObservationChange {
    hits.fold(ObservationChange::Equivalent, |change, hit| {
        strongest(
            change,
            match hit {
                HitVerdict::MaterialChallenge => ObservationChange::Correction,
                HitVerdict::MaterialQualification => ObservationChange::Qualification,
                _ => ObservationChange::Equivalent,
            },
        )
    })
}

fn strongest(a: ObservationChange, b: ObservationChange) -> ObservationChange {
    match (a, b) {
        (ObservationChange::Correction, _) | (_, ObservationChange::Correction) => {
            ObservationChange::Correction
        }
        (ObservationChange::Qualification, _) | (_, ObservationChange::Qualification) => {
            ObservationChange::Qualification
        }
        _ => ObservationChange::Equivalent,
    }
}
