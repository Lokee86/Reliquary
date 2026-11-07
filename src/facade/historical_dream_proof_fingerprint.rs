//! Dream has no publication clock: zero denotes a current clock-neutral snapshot.
//! Embedded activity/Graph proof cuts remain exact and are never borrowed clocks.
use crate::Cva;
use crate::historical_fingerprint::{HistoricalFingerprint, OwnerProjection};

pub(crate) fn dream_proof_state(cva: &Cva) -> Result<HistoricalFingerprint, String> {
    let owner = cva
        .owner_uuid()
        .ok_or("Dream proof oracle needs owner UUID")?;
    let mut projection = OwnerProjection::new(owner, "dream_proof", 0, None)?;
    projection.insert(
        "resume_state",
        b"current",
        cva.dream_freshness.historical_oracle_value(),
    )?;
    // Only retained accepted graph proofs certify completed outcomes. A settled
    // marker without that evidence remains distinct from a verified empty pass.
    for (source, pass) in cva
        .historical_completed_initial_dream_passes()
        .map_err(|e| e.to_string())?
    {
        projection.insert(
            "completed",
            source.0,
            crate::freshness_dream_journal::historical_pass_value(&pass),
        )?;
    }
    projection.fingerprint()
}

#[cfg(test)]
#[path = "historical_dream_proof_fingerprint_tests.rs"]
mod tests;
