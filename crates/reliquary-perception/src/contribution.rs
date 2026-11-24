use crate::native::{Question, dispatch, insert_question};
use crate::{DecisionDiagnostics, HitVerdict, NativeDecisionError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CONTRIBUTION_POLICY_VERSION: u16 = 1;
const MAX_HITS: usize = 5;
const MAX_PASSAGE_BYTES: usize = 24 * 1024;
const TEMPLATE: &str = include_str!("questions/contribution.json");

/// The stored claim and exact passage text keyed by request-local passage ID.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContributionState {
    pub existing_observation: String,
    pub evidence_hits: BTreeMap<String, String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TemporalChoice {
    ChangeToOngoingClaim,
    EarlierPeriodEvidence,
    FirstEffectiveAfter,
    LaterChangeOnly,
    UnclearOrOther,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeChoice {
    ExplicitlySeparateScope,
    OverlappingScope,
    UnclearScope,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContributionHitDecision {
    pub verdict: HitVerdict,
    pub time: TemporalChoice,
    pub scope: ScopeChoice,
}

impl ContributionHitDecision {
    pub fn triggers_review(self) -> bool {
        self.verdict.is_material()
            && self.scope != ScopeChoice::ExplicitlySeparateScope
            && !(self.verdict == HitVerdict::MaterialQualification
                && self.time == TemporalChoice::LaterChangeOnly)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContributionDecision {
    pub hits: BTreeMap<String, ContributionHitDecision>,
    pub diagnostics: BTreeMap<String, DecisionDiagnostics>,
}

impl ContributionDecision {
    pub fn triggers_review(&self) -> bool {
        self.hits.values().any(|hit| hit.triggers_review())
    }
}

/// Dispatch one native request for the admitted batch and validate all three answers per hit.
pub fn contribute(
    endpoint: &dyn reliquary_switchboard::DecisionEndpoint,
    state: &ContributionState,
) -> Result<ContributionDecision, NativeDecisionError> {
    if state.existing_observation.trim().is_empty()
        || state.evidence_hits.is_empty()
        || state.evidence_hits.len() > MAX_HITS
        || state
            .evidence_hits
            .iter()
            .any(|(id, text)| id.trim().is_empty() || text.trim().is_empty())
        || state.evidence_hits.values().map(String::len).sum::<usize>() > MAX_PASSAGE_BYTES
    {
        return Err(NativeDecisionError::InvalidRequest(
            "invalid contribution evidence or limits",
        ));
    }
    let template = serde_json::from_str::<BTreeMap<String, Question>>(TEMPLATE)
        .expect("checked contribution template");
    let mut questions = BTreeMap::new();
    for id in state.evidence_hits.keys() {
        for (key, question) in &template {
            insert_question(
                &mut questions,
                key.replace("{hit}", id),
                question.for_hit(id),
            )?;
        }
    }
    let answers = dispatch(endpoint, state, &questions, None)?;
    let hits = state
        .evidence_hits
        .keys()
        .map(|id| {
            let hit = ContributionHitDecision {
                verdict: answers.choice(id)?,
                time: answers.choice(&format!("{id}-time"))?,
                scope: answers.choice(&format!("{id}-scope"))?,
            };
            Ok((id.clone(), hit))
        })
        .collect::<Result<_, _>>()?;
    Ok(ContributionDecision {
        hits,
        diagnostics: answers.diagnostics,
    })
}
