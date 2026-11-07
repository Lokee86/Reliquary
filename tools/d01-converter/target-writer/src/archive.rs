use crate::{Failure, transfer::Transfer};
use reliquary_owner_session::{RelSession, RelWriter};
use serde_json::{Value, json};
use std::path::Path;

pub fn materialize(bytes: &[u8], staged_file: &Path) -> Result<Value, Failure> {
    let transfer: Transfer =
        serde_json::from_slice(bytes).map_err(|_| (4, "invalid synthetic Archive transfer"))?;
    let owner = transfer.owner().map_err(|reason| (4, reason))?;
    let scope = transfer.scope();
    let generation = transfer.generation().map_err(|reason| (4, reason))?;
    let mut writer = RelWriter::create_current_path(staged_file, scope, owner)
        .map_err(|_| (7, "could not initialize a current owner"))?;
    let cuts = transfer.replay(&mut writer).map_err(|reason| (4, reason))?;
    if cuts[0].1.history_generation != generation {
        return Err((4, "unsupported synthetic owner history generation"));
    }
    drop(writer);

    let reader = RelSession::open_path_current(staged_file)
        .map_err(|_| (6, "candidate failed normal current-only reopen"))?;
    if reader.owner_id() != owner || reader.scope() != scope {
        return Err((6, "candidate owner identity changed on reopen"));
    }
    for (cut, proof) in &cuts {
        if reader.historical_proof(owner, cut).as_ref() != Ok(proof)
            || proof.history_generation != generation
        {
            return Err((6, "candidate historical proof changed on reopen"));
        }
    }
    if cuts.last().map(|(cut, _)| cut) != Some(&reader.current_historical_cut()) {
        return Err((6, "candidate head changed on reopen"));
    }
    let history = cuts
        .iter()
        .map(|(cut, proof)| {
            json!({
                "archive_version": cut.archive_version.to_string(),
                "retained_records": proof.retained_records.to_string(),
                "proof_sha256": hex::encode(proof.digest)
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "schema_id": "reliquary-d01-synthetic-archive-receipt-v1",
        "scope": "synthetic_archive_only",
        "owner": hex::encode(owner),
        "history_generation": hex::encode(generation),
        "archive_history": history
    }))
}
