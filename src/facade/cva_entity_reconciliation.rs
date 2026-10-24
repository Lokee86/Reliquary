use crate::entity_reconciliation::{ReconciliationRoundContext, reconcile_round};
use crate::entity_reconciliation_model::EntityReconciliationReport;
use crate::{Cva, EntityReconciliationError, GeneralEndpoint, Phylactery};
use std::collections::HashSet;

macro_rules! impl_reconciliation {
    ($owner:ty $(, $archive:ident)?) => {
        impl $owner {
            pub fn reconcile_entities<E: GeneralEndpoint>(
                &mut self,
                endpoint: &E,
                now_ns: i64,
            ) -> Result<EntityReconciliationReport, EntityReconciliationError> {
                let mut report = EntityReconciliationReport {
                    rounds: 0,
                    candidate_pairs: 0,
                    deterministic_merges: 0,
                    model_merges: 0,
                    rejected_mentions_reconsidered: 0,
                    unresolved_pairs: 0,
                    final_audit: crate::EntityAuditReport::default(),
                };
                let mut seen_model_pairs = HashSet::new();
                let mut timestamp = now_ns;
                for round in 0..crate::DEFAULT_ENTITY_RECONCILIATION_ROUNDS {
                    let mut context = ReconciliationRoundContext {
                        container: &mut self.container,
                        memories: &self.memories,
                        entities: &mut self.entities,
                        graph: &mut self.graph,
                        resolutions: &mut self.entity_resolutions,
                        relationships: &mut self.relationships,
                    };
                    let result = reconcile_round(
                        &mut context,
                        endpoint,
                        &mut timestamp,
                        &mut seen_model_pairs,
                    )?;
                    report.candidate_pairs += result.candidate_pairs;
                    report.deterministic_merges += result.deterministic_merges;
                    report.model_merges += result.model_merges;
                    report.unresolved_pairs += result.unresolved_pairs;
                    let reconsidered = {
                        let mut context =
                            crate::entity_reconciliation_owner::RejectionReconsiderationContext {
                                container: &mut self.container,
                                memories: &self.memories,
                                entities: &mut self.entities,
                                graph: &mut self.graph,
                                resolutions: &mut self.entity_resolutions,
                                lexical_index: &mut self.lexical_index,
                                archive: None $(.or(Some(&self.$archive)))?,
                            };
                        crate::entity_reconciliation_owner::reconsider_late_exact_rejections(
                            &mut context,
                            endpoint,
                            &mut timestamp,
                        )?
                    };
                    report.rejected_mentions_reconsidered += reconsidered;
                    self.container
                        .sync()
                        .map_err(|error| EntityReconciliationError::Operation(error.to_string()))?;
                    report.rounds = round + 1;
                    if !result.changed && reconsidered == 0 {
                        break;
                    }
                }
                report.final_audit = self.audit_entities();
                Ok(report)
            }
        }
    };
}
impl_reconciliation!(Cva, archive);
impl_reconciliation!(Phylactery);

macro_rules! impl_reconciliation_prepare {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn prepare_entity_reconciliation(
                &self,
            ) -> Result<
                crate::entity_reconciliation_model::EntityReconciliationPrepared,
                crate::EntityReconciliationError,
            > {
                let context = crate::entity_reconciliation_prepare::ReconciliationPrepareContext {
                    entities: &self.entities,
                    graph: &self.graph,
                    memories: &self.memories,
                    resolutions: &self.entity_resolutions,
                    container: &self.container,
                };
                crate::entity_reconciliation_prepare::prepare_entity_reconciliation(&context)
            }
        }
    };
}
impl_reconciliation_prepare!(Cva);
impl_reconciliation_prepare!(Phylactery);

use crate::entity_reconciliation_commit::{self, ReconciliationCommitContext};
use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPrepared,
};
macro_rules! impl_reconciliation_commit {
    ($owner:ty) => {
        impl $owner {
            pub(crate) fn commit_entity_reconciliation(
                &mut self,
                prepared: EntityReconciliationPrepared,
                evaluation: EntityReconciliationEvaluation,
                now_ns: i64,
            ) -> Result<
                Option<crate::entity_reconciliation_model::EntityReconciliationCommit>,
                crate::EntityReconciliationError,
            > {
                let mut context = ReconciliationCommitContext {
                    container: &mut self.container,
                    memories: &self.memories,
                    entities: &mut self.entities,
                    graph: &mut self.graph,
                    resolutions: &mut self.entity_resolutions,
                    relationships: &mut self.relationships,
                };
                entity_reconciliation_commit::commit_entity_reconciliation(
                    &mut context,
                    prepared,
                    evaluation,
                    now_ns,
                )
            }
        }
    };
}
impl_reconciliation_commit!(Cva);
impl_reconciliation_commit!(Phylactery);
