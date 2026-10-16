//! Complete authorized one-hop Memory evidence; no recursive graph browsing.
use super::query::{auth_error, read_error};
use super::*;
use crate::{AgenticMemoryError, AgenticMemoryRelationEvidence, GraphDirection, MemoryRef};

struct PreparedAnchor {
    report: crate::AgenticMemoryAnchorReport,
    candidates: Vec<crate::AgenticMemoryContent>,
}

impl GatewayRuntime {
    /// Independently evaluate one hop per anchor, then commit the gathered union.
    pub fn traverse_graph(
        &self,
        instance: &InstanceRuntime,
        controller: &mut crate::AgenticMemoryController,
        anchors: &[MemoryRef],
        delivery_id: &str,
    ) -> Result<crate::PreparedAgenticMemoryResult, AgenticMemoryError> {
        if delivery_id.trim().is_empty() {
            return Err(AgenticMemoryError::InvalidDeliveryIdentity);
        }
        let mut staged = controller.stage_traversal()?;
        let envelope = self.capture_agentic_read(instance).map_err(auth_error)?;
        let anchors = anchors.iter().cloned().collect::<BTreeSet<_>>();
        let prepared = anchors
            .iter()
            .map(|anchor| self.prepare_agentic_anchor(&envelope, controller, anchor))
            .collect::<Result<Vec<_>, _>>()?;
        let decided = self.decide_agentic_anchors(staged.query(), prepared)?;
        let mut evaluated = BTreeSet::new();
        let mut approved = BTreeSet::new();
        let mut bodies = BTreeMap::new();
        let mut reports = Vec::new();
        for anchor in decided {
            approved.extend(anchor.report.approved.iter().cloned());
            for candidate in anchor.candidates {
                evaluated.insert(candidate.memory.clone());
                if let Some(previous) = bodies.insert(candidate.memory.clone(), candidate.clone()) {
                    if previous != candidate {
                        return Err(read_error(
                            "shared graph neighbor changed during preparation",
                        ));
                    }
                }
            }
            reports.push(anchor.report);
        }
        staged.record_evaluated(evaluated)?;
        let content = bodies
            .into_iter()
            .filter(|(memory, _)| approved.contains(memory))
            .map(|(_, body)| body)
            .collect::<Vec<_>>();
        let outcome = if content.is_empty() {
            crate::AgenticMemoryOutcome::NoMatches
        } else {
            crate::AgenticMemoryOutcome::Memories(content)
        };
        let result = super::query::prepare_result(
            delivery_id,
            crate::AgenticMemoryOperation::Traversal,
            outcome,
            reports,
            &envelope,
        );
        let result = self
            .with_agentic_read_delivery(instance, &envelope, || Ok(result))
            .map_err(auth_error)?;
        staged.commit(controller)?;
        Ok(result)
    }

    fn decide_agentic_anchors(
        &self,
        query: &str,
        prepared: Vec<PreparedAnchor>,
    ) -> Result<Vec<PreparedAnchor>, AgenticMemoryError> {
        if prepared.iter().all(|anchor| anchor.candidates.is_empty()) {
            return Ok(prepared);
        }
        let endpoint = self.agentic_decision_route().map_err(read_error)?;
        let results = std::thread::scope(|scope| {
            let workers = prepared
                .into_iter()
                .map(|mut anchor| {
                    let endpoint = &endpoint;
                    scope.spawn(move || {
                        anchor.report.approved =
                            crate::agentic_memory_retrieval_decision::select_memories(
                                endpoint.as_ref(),
                                crate::AgenticMemoryOperation::Traversal,
                                query,
                                &anchor.candidates,
                                Some(&anchor.report.anchor),
                                &anchor.report.relations,
                            )?;
                        Ok(anchor)
                    })
                })
                .collect::<Vec<_>>();
            // Join every worker before interpreting failures, preserving input order.
            workers
                .into_iter()
                .map(|worker| {
                    worker.join().unwrap_or_else(|_| {
                        Err(AgenticMemoryError::Decision(
                            "graph anchor worker panicked".into(),
                        ))
                    })
                })
                .collect::<Vec<Result<PreparedAnchor, AgenticMemoryError>>>()
        });
        results.into_iter().collect()
    }

    fn prepare_agentic_anchor(
        &self,
        envelope: &AgenticReadEnvelope,
        controller: &crate::AgenticMemoryController,
        anchor: &MemoryRef,
    ) -> Result<PreparedAnchor, AgenticMemoryError> {
        use crate::agentic_memory_retrieval::{grade_agentic_memories, order_agentic_memories};
        let relations = self.agentic_anchor_evidence(envelope, anchor)?;
        let neighbors = relations
            .iter()
            .map(|edge| edge.neighbor.clone())
            .collect::<BTreeSet<_>>();
        let declined_neighbors = neighbors
            .iter()
            .filter(|id| {
                controller.searched_ids().contains(*id) && !controller.selected_ids().contains(*id)
            })
            .count();
        let unseen = neighbors
            .into_iter()
            .filter(|id| {
                !controller.searched_ids().contains(id) && !controller.selected_ids().contains(id)
            })
            .collect::<Vec<_>>();
        let unseen_neighbors = unseen.len();
        let owner = envelope
            .owners
            .iter()
            .find(|owner| owner.token.owner_id == anchor.owner_id)
            .ok_or_else(|| auth_error("graph anchor is outside the authorized REL closure"))?;
        let ids = unseen.iter().map(|id| id.memory_id).collect::<Vec<_>>();
        let mut ranked = self
            .with_rel(&owner.token, |runtime| {
                grade_agentic_memories(&runtime.cva, &ids, owner.cut).map_err(operation)
            })
            .map_err(read_error)?;
        order_agentic_memories(&mut ranked);
        ranked.truncate(10);
        let candidates = ranked
            .iter()
            .map(|candidate| self.hydrate_agentic_memory(envelope, &candidate.memory))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PreparedAnchor {
            report: crate::AgenticMemoryAnchorReport {
                anchor: anchor.clone(),
                unseen_neighbors,
                declined_neighbors,
                relations,
                approved: BTreeSet::new(),
            },
            candidates,
        })
    }

    fn agentic_anchor_evidence(
        &self,
        envelope: &AgenticReadEnvelope,
        anchor: &MemoryRef,
    ) -> Result<Vec<AgenticMemoryRelationEvidence>, AgenticMemoryError> {
        // Hydration validates both the owner-qualified anchor and its active body.
        self.hydrate_agentic_memory(envelope, anchor)?;
        let owner = envelope
            .owners
            .iter()
            .find(|owner| owner.token.owner_id == anchor.owner_id)
            .ok_or_else(|| auth_error("graph anchor is outside the authorized REL closure"))?;
        self.with_rel(&owner.token, |runtime| {
            let mut evidence = BTreeMap::new();
            for relation in runtime.cva.graph_relations() {
                for (source, target, direction, direction_key) in [
                    (
                        relation.source,
                        relation.target,
                        GraphDirection::Outgoing,
                        0u8,
                    ),
                    (
                        relation.target,
                        relation.source,
                        GraphDirection::Incoming,
                        1u8,
                    ),
                ] {
                    if source != anchor.memory_id || target == anchor.memory_id {
                        continue;
                    }
                    let body = runtime.cva.memory(target).map_err(operation)?;
                    if body.archived {
                        continue;
                    }
                    let neighbor = MemoryRef {
                        owner_id: anchor.owner_id.clone(),
                        memory_id: target,
                    };
                    evidence.insert(
                        (neighbor.clone(), relation.kind, direction_key),
                        AgenticMemoryRelationEvidence {
                            neighbor,
                            kind: relation.kind,
                            direction,
                        },
                    );
                }
            }
            Ok(evidence.into_values().collect())
        })
        .map_err(read_error)
    }
}

#[cfg(test)]
#[path = "runtime_host_gateway_agentic_memory_graph_tests.rs"]
mod tests;
