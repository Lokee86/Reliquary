use crate::native::{Question, dispatch, insert_question};
use crate::{DecisionDiagnostics, HitVerdict, NativeDecisionError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const VERIFICATION_POLICY_VERSION: u16 = 1;
const MAX_CITATIONS: usize = 8;
const MAX_HITS: usize = 3;
const MAX_CITATION_BYTES: usize = 24 * 1024;
const MAX_BACKCHECK_BYTES: usize = 8 * 1024;
const MAX_RESPONSE_BYTES: usize = 16 * 1024;
const TEMPLATE: &str = include_str!("questions/verification.json");

/// The exact quoted text and adjacent context from the authorized source cut.
/// The opaque source ID is shared when two passages reach the same ultimate lineage.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidencePassage {
    pub id: String,
    pub source_id: String,
    pub text: String,
    pub before: String,
    pub after: String,
}

impl EvidencePassage {
    fn bytes(&self) -> usize {
        self.text.len() + self.before.len() + self.after.len()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct VerificationState {
    pub claim: String,
    pub citations: Vec<EvidencePassage>,
    pub backcheck_hits: Vec<EvidencePassage>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationVerdict {
    StandsAsWritten,
    NeedsRevision,
    Unresolved,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VerificationDecision {
    /// The disposition after applying the material back-check veto.
    pub verdict: VerificationVerdict,
    pub overall: VerificationVerdict,
    pub hits: BTreeMap<String, HitVerdict>,
    pub diagnostics: BTreeMap<String, DecisionDiagnostics>,
}

/// Verify one frozen draft using the same contract for initial and successor claims.
pub fn verify(
    endpoint: &dyn reliquary_switchboard::DecisionEndpoint,
    state: &VerificationState,
) -> Result<VerificationDecision, NativeDecisionError> {
    let mut ids = BTreeSet::new();
    if state.claim.trim().is_empty()
        || state.citations.is_empty()
        || state.citations.len() > MAX_CITATIONS
        || state.backcheck_hits.len() > MAX_HITS
        || state
            .citations
            .iter()
            .map(EvidencePassage::bytes)
            .sum::<usize>()
            > MAX_CITATION_BYTES
        || state
            .backcheck_hits
            .iter()
            .map(EvidencePassage::bytes)
            .sum::<usize>()
            > MAX_BACKCHECK_BYTES
        || state
            .citations
            .iter()
            .chain(&state.backcheck_hits)
            .any(|row| {
                row.id.trim().is_empty()
                    || row.source_id.trim().is_empty()
                    || row.text.trim().is_empty()
                    || !ids.insert(&row.id)
            })
    {
        return Err(NativeDecisionError::InvalidRequest(
            "invalid verification evidence or limits",
        ));
    }
    let template = serde_json::from_str::<BTreeMap<String, Question>>(TEMPLATE)
        .expect("checked verification template");
    let mut questions = BTreeMap::from([("verify".into(), template["verify"].clone())]);
    for hit in &state.backcheck_hits {
        insert_question(
            &mut questions,
            format!("hit-{}", hit.id),
            template["hit-{hit}"].for_hit(&hit.id),
        )?;
    }
    let answers = dispatch(endpoint, state, &questions, Some(MAX_RESPONSE_BYTES))?;
    let overall = answers.choice("verify")?;
    let hits = state
        .backcheck_hits
        .iter()
        .map(|hit| {
            Ok((
                hit.id.clone(),
                answers.choice::<HitVerdict>(&format!("hit-{}", hit.id))?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let verdict = if overall == VerificationVerdict::StandsAsWritten
        && hits.values().any(|choice| choice.is_material())
    {
        VerificationVerdict::Unresolved
    } else {
        overall
    };
    Ok(VerificationDecision {
        verdict,
        overall,
        hits,
        diagnostics: answers.diagnostics,
    })
}
