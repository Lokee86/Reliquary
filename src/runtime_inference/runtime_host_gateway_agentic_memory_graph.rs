//! Complete authorized one-hop Memory evidence; no recursive graph browsing.
use super::query::{auth_error, read_error};
use super::*;
use crate::{AgenticMemoryError, AgenticMemoryRelationEvidence, GraphDirection, MemoryRef};

impl GatewayRuntime {
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
