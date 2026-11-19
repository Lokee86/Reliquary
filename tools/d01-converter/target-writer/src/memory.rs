use crate::{
    Failure,
    memory_transfer::{Publication, Transfer},
};
use reliquary_container::{Container, FileKind, decode_publication};
use reliquary_memory_core::{decode_grouped_creation, history_change_in_frame};
use reliquary_owner_session::{PhySession, PhyWriter};
use serde_json::{Value, json};
use std::path::Path;

pub fn materialize(bytes: &[u8], staged_file: &Path) -> Result<Value, Failure> {
    let transfer: Transfer =
        serde_json::from_slice(bytes).map_err(|_| (4, "invalid synthetic Memory transfer"))?;
    let owner = transfer.owner().map_err(|reason| (4, reason))?;
    let generation = transfer.generation().map_err(|reason| (4, reason))?;
    let publications = transfer.publications().map_err(|reason| (4, reason))?;
    let mut writer = PhyWriter::create_path(staged_file, owner)
        .map_err(|_| (7, "could not initialize a current PHY owner"))?;
    for publication in &publications {
        writer
            .import_accepted_memory_revision(
                publication.global_version,
                publication.transaction_time_ns,
                &publication.revision,
            )
            .map_err(|_| (4, "accepted Memory revision refused by current owner"))?;
    }
    drop(writer);

    let reader = PhySession::open_path_current(staged_file)
        .map_err(|_| (6, "candidate failed normal current-only PHY reopen"))?;
    let mut cut = reader.current_historical_cut();
    if reader.owner_id() != owner
        || Some(cut.memory_version)
            != publications
                .last()
                .map(|item| item.revision.record.memory_version)
    {
        return Err((6, "candidate PHY identity or head changed on reopen"));
    }
    cut.memory_version = 0;
    let genesis = reader
        .historical_proof(owner, cut)
        .map_err(|_| (6, "candidate PHY genesis proof is invalid"))?;
    if genesis.history_generation != generation {
        return Err((4, "unsupported synthetic PHY history generation"));
    }
    let mut history = Vec::with_capacity(publications.len());
    for publication in &publications {
        let record = &publication.revision.record;
        cut.memory_version = record.memory_version;
        let reopened = reader
            .memory_history_at_cut(record.id, record.memory_version)
            .map_err(|_| (6, "candidate Memory history cut is unavailable"))?
            .ok_or((6, "candidate Memory revision is missing on reopen"))?;
        if reopened.tombstone.is_some() || reopened.revisions.last() != Some(&publication.revision)
        {
            return Err((6, "candidate Memory revision changed on reopen"));
        }
        let proof = reader
            .historical_proof(owner, cut)
            .map_err(|_| (6, "candidate Memory historical proof is invalid"))?;
        history.push(json!({
            "memory_version": record.memory_version.to_string(),
            "global_version": publication.global_version.to_string(),
            "transaction_time_ns": publication.transaction_time_ns.to_string(),
            "proof_sha256": hex::encode(proof.digest)
        }));
    }
    drop(reader);
    verify_bindings(staged_file, &publications)?;
    Ok(json!({
        "schema_id": "reliquary-d01-synthetic-phy-memory-receipt-v1",
        "scope": "synthetic_ungrouped_phy_memory_only",
        "owner": hex::encode(owner),
        "history_generation": hex::encode(generation),
        "memory_history": history
    }))
}

fn verify_bindings(path: &Path, publications: &[Publication]) -> Result<(), Failure> {
    let container = Container::open_read_only_as(path, FileKind::Phylactery)
        .map_err(|_| (6, "candidate global clock could not be reopened"))?;
    let mut expected = publications.iter();
    let frames = container
        .read_frames()
        .map_err(|_| (6, "candidate Memory publication scan failed"))?;
    for frame in frames {
        let Some(envelope) = decode_publication(&frame.payload)
            .map_err(|_| (6, "candidate Memory publication is malformed"))?
        else {
            continue;
        };
        let coordinate = if envelope.body.starts_with(b"RELMCRT1") {
            let creation = decode_grouped_creation(envelope.body)
                .map_err(|_| (6, "candidate Memory creation is malformed"))?;
            Some((creation.id, creation.memory_version))
        } else {
            history_change_in_frame(envelope.body)
                .map_err(|_| (6, "candidate Memory revision is malformed"))?
        };
        let wanted = expected
            .next()
            .ok_or((6, "unexpected candidate publication"))?;
        let record = &wanted.revision.record;
        if coordinate != Some((record.id, record.memory_version))
            || envelope.version != wanted.global_version
            || envelope.bound_reservation
            || envelope.transaction_time_ns != Some(wanted.transaction_time_ns)
            || container.transaction_time_ns(envelope.version) != envelope.transaction_time_ns
        {
            return Err((6, "candidate Memory global binding changed on reopen"));
        }
    }
    if expected.next().is_some() {
        return Err((6, "candidate Memory global binding is missing"));
    }
    Ok(())
}
