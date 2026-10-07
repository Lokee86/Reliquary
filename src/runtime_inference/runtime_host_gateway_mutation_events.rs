//! P4 post-sync classification at the gateway and concrete REL worker boundary.
//! No standalone facade hooks, semantic ownership or general event log.
use super::events::{RelEventClass, RelInvalidation, RelSequencerGate};
use super::observation::DurableWatermarks;
use super::{
    GatewayRuntime, OwnerAccessToken, OwnerGrant, OwnerKind, ReliquaryRuntimeHostError, operation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RelMutationFamily {
    Semantic,
    Metadata,
    ConversationMetadata,
    ConversationBranch,
    Project,
    Processing,
    Background,
    Retrieval,
}

#[derive(PartialEq)]
struct Cut {
    watermarks: DurableWatermarks,
    metadata: crate::RelMetadata,
    processing: crate::InsomniaStats,
    communities: crate::CommunityStats,
    community_names: Vec<crate::CommunitySemanticName>,
    profiles: usize,
}
fn cut(runtime: &crate::InteractionRuntime) -> Cut {
    let cva = runtime.cva();
    Cut {
        watermarks: super::snapshots::reader::source_watermarks(cva),
        metadata: cva.rel_metadata(),
        processing: cva.insomnia_stats(),
        communities: cva.community_stats(),
        community_names: cva.community_semantic_names(),
        profiles: cva.compatibility_profiles().len(),
    }
}

/// The bool represents a changed auxiliary record not covered by source clocks
/// (for example a community name or Dream cooldown). IDs contain no bodies.
pub(super) fn commit<T>(
    gate: &mut RelSequencerGate<'_>,
    runtime: &mut crate::InteractionRuntime,
    family: RelMutationFamily,
    mutate: impl FnOnce(
        &mut crate::InteractionRuntime,
    ) -> Result<(T, bool, Vec<RelInvalidation>), ReliquaryRuntimeHostError>,
) -> Result<T, ReliquaryRuntimeHostError> {
    let before = cut(runtime);
    let (value, auxiliary_changed, mut ids) = match mutate(runtime) {
        Ok(result) => result,
        Err(error) => {
            // A multi-step owner mutation may already have changed authority.
            // Never publish success or pretend continuity after an unknown cut.
            if cut(runtime) != before {
                gate.invalidate();
            }
            return Err(error);
        }
    };
    if let Err(error) = runtime.cva.sync().map_err(operation) {
        gate.invalidate();
        return Err(error);
    }
    let after = cut(runtime);
    if before == after && !auxiliary_changed {
        return Ok(value);
    }
    // The P3 journal has a separate gate; preserve its last observed durable cut.
    let mut watermarks = after.watermarks.clone();
    watermarks.conversation_journal_sequence = gate.watermarks().conversation_journal_sequence;
    let a = &watermarks;
    let b = &before.watermarks;
    let semantic = a.memory_version != b.memory_version
        || a.entity_version != b.entity_version
        || a.graph_version != b.graph_version
        || a.ego_mutation_version != b.ego_mutation_version
        || after.communities != before.communities
        || after.community_names != before.community_names
        || (family == RelMutationFamily::Semantic && auxiliary_changed);
    let retrieval = after.profiles != before.profiles
        || semantic
        || (family == RelMutationFamily::Retrieval && auxiliary_changed)
        || a.vector_version != b.vector_version
        || after.metadata.dependencies != before.metadata.dependencies;
    let project = a.project_correlation_sequence != b.project_correlation_sequence
        || (family == RelMutationFamily::Project && auxiliary_changed);
    if project {
        if let Some(sequence) = a.project_correlation_sequence {
            let id = RelInvalidation::ProjectCorrelation(sequence);
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    let processing = after.processing != before.processing
        || (matches!(
            family,
            RelMutationFamily::Processing | RelMutationFamily::Background
        ) && auxiliary_changed);
    let classes = [
        (
            a.archive_version != b.archive_version
                || (matches!(
                    family,
                    RelMutationFamily::ConversationMetadata | RelMutationFamily::ConversationBranch
                ) && auxiliary_changed),
            RelEventClass::ArchiveChanged,
        ),
        (semantic, RelEventClass::SemanticOwnerChanged),
        (retrieval, RelEventClass::RetrievalInvalidated),
        (processing, RelEventClass::ProcessingStatusChanged),
        (project, RelEventClass::ProjectStateChanged),
        (
            after.metadata != before.metadata,
            RelEventClass::OwnerLifecycleChanged,
        ),
    ];
    // Distribution failure cannot negate the already synchronized owner result.
    for (changed, class) in classes {
        if changed {
            let _ = gate.publish(class, a.clone(), ids.clone());
        }
    }
    Ok(value)
}

impl GatewayRuntime {
    pub(super) fn publish_host_capability_change(&self) {
        let owners = match self.rel_executions.read() {
            Ok(rels) => rels
                .values()
                .map(|rel| {
                    (
                        std::sync::Arc::clone(&rel.fence),
                        std::sync::Arc::clone(&rel.sequencer),
                    )
                })
                .collect::<Vec<_>>(),
            Err(_) => return,
        };
        for (fence, sequencer) in owners {
            let Ok(_lease) = fence.acquire_current() else {
                continue;
            };
            let Ok(mut gate) = sequencer.current_gate() else {
                continue;
            };
            let watermarks = gate.watermarks().clone();
            let ids = super::RuntimeCapability::ALL
                .into_iter()
                .map(RelInvalidation::HostCapability)
                .collect();
            let _ = gate.publish(RelEventClass::HostWorkChanged, watermarks, ids);
        }
    }

    pub(super) fn with_rel_mutation<T>(
        &self,
        token: &OwnerAccessToken,
        family: RelMutationFamily,
        mutate: impl FnOnce(
            &mut crate::InteractionRuntime,
        )
            -> Result<(T, bool, Vec<RelInvalidation>), ReliquaryRuntimeHostError>,
    ) -> Result<T, ReliquaryRuntimeHostError> {
        let required = match family {
            RelMutationFamily::Project => OwnerGrant::ManageProject,
            RelMutationFamily::ConversationBranch => OwnerGrant::SubmitConversation,
            RelMutationFamily::Semantic if token.grant == OwnerGrant::SubmitConversation => {
                OwnerGrant::SubmitConversation
            }
            _ => OwnerGrant::ManageRel,
        };
        if token.kind != OwnerKind::Rel || token.grant != required {
            return Err(operation(
                "REL mutation requires the exact applicable owner grant",
            ));
        }
        self.with_rel_distribution_admission(token, |sequencer| {
            let mut gate = sequencer.gate(token.mount_epoch)?;
            self.with_rel(token, |runtime| commit(&mut gate, runtime, family, mutate))
        })
    }
}
#[cfg(test)]
#[path = "runtime_host_gateway_mutation_events_tests.rs"]
mod tests;
