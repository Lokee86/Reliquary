use crate::entity_reconciliation_model::{
    EntityReconciliationEvaluation, EntityReconciliationPairDecision, EntityReconciliationPrepared,
};
use crate::{EntityReconciliationError, EntityReconciliationRelation, GeneralEndpoint};
use reliquary_entity_resolution::classify_pair;

pub(crate) fn evaluate_reconciliation<E: GeneralEndpoint>(
    endpoint: &E,
    prepared: &EntityReconciliationPrepared,
) -> Result<EntityReconciliationEvaluation, EntityReconciliationError> {
    let mut decisions = Vec::with_capacity(prepared.pairs.len());
    for pair in &prepared.pairs {
        let relation = if pair.candidate.deterministic {
            EntityReconciliationRelation::SameIdentity
        } else {
            classify_pair(
                endpoint,
                &pair.left,
                &pair.right,
                &pair.left_support,
                &pair.right_support,
            )?
        };
        decisions.push(EntityReconciliationPairDecision {
            left: pair.left.id,
            right: pair.right.id,
            relation,
            deterministic: pair.candidate.deterministic,
        });
    }
    Ok(EntityReconciliationEvaluation { decisions })
}
