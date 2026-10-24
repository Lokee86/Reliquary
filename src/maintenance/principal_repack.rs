use crate::ObjectRef;
use crate::archive_codec::{ArchiveRecord, decode_record, encode_node};
use crate::owner_lifecycle::relocation::relocate_payload;
use crate::owner_lifecycle::{OwnerError, ReliquaryOwner};
use crate::turn_ingest_codec::encode_ingested_turn;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrincipalBackfillRepackResult {
    pub rewritten_user_turns: usize,
    pub preserved_user_turns: usize,
    pub copied_chunks: usize,
    pub source_file_bytes: u64,
    pub output_file_bytes: u64,
}

impl ReliquaryOwner {
    /// Repack a single-principal REL while filling missing principal IDs on user turns.
    ///
    /// Existing principal IDs are preserved when they match the supplied principal ID.
    /// A conflicting user-turn principal fails closed. Non-user turns are never assigned one.
    pub(crate) fn repack_missing_user_principal(
        &mut self,
        output_path: impl AsRef<Path>,
        principal_id: &str,
    ) -> Result<PrincipalBackfillRepackResult, OwnerError> {
        if !crate::entity_principal_identity::is_phy_principal_id(principal_id) {
            return Err(OwnerError::Repack(
                "principal backfill requires a canonical phy-UUID".into(),
            ));
        }
        self.validate_historical_principal_repack(principal_id)?;
        let output_path = output_path.as_ref();
        self.container.sync()?;
        let source_file_bytes = fs::metadata(self.container.path())
            .map_err(|error| OwnerError::Repack(error.to_string()))?
            .len();
        let payloads = self.container.object_payloads()?;
        let mut output = self.container.create_empty_like(output_path)?;
        let output_guard = output.read_source()?;
        let mut relocated = BTreeMap::<ObjectRef, ObjectRef>::new();
        let mut rewritten_user_turns = 0usize;
        let mut preserved_user_turns = 0usize;
        let mut copied_chunks = 0usize;

        let copy_result = (|| {
            for (source_ref, payload) in payloads {
                let (payload, rewritten, preserved) =
                    rewrite_missing_user_principal(&payload, principal_id)?;
                rewritten_user_turns += usize::from(rewritten);
                preserved_user_turns += usize::from(preserved);
                let payload = relocate_payload(&payload, &relocated).map_err(OwnerError::Repack)?;
                let destination_ref = output.append(&payload)?;
                relocated.insert(source_ref, destination_ref);
                copied_chunks += 1;
            }
            output.sync()?;
            Ok::<(), OwnerError>(())
        })();

        if let Err(error) = copy_result {
            drop(output);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
        let reopened =
            match crate::maintenance::reconciliation_owner_access::open_reconciliation_container(
                output,
            ) {
                Ok(reopened) => reopened,
                Err(error) => {
                    if output_guard.validate_path_identity().is_ok() {
                        let _ = fs::remove_file(output_path);
                    }
                    return Err(error);
                }
            };
        if let Err(error) = self.validate_historical_import_copy(&reopened) {
            drop(reopened);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(error);
        }
        if reopened.owner_uuid() != self.owner_uuid()
            || reopened.latest_global_version() != self.latest_global_version()
            || reopened.archive.archive_version() != self.archive.archive_version()
            || !self.archive.activity_history_matches(&reopened.archive)
            || reopened.memories.memory_version() != self.memories.memory_version()
            || reopened.entities.entity_version() != self.entities.entity_version()
            || reopened.graph.graph_version() != self.graph.graph_version()
            || reopened.archive.stats() != self.archive.stats()
        {
            drop(reopened);
            if output_guard.validate_path_identity().is_ok() {
                let _ = fs::remove_file(output_path);
            }
            return Err(OwnerError::Repack(
                "principal backfill changed owner, semantic versions or activity history".into(),
            ));
        }

        for source in self.archive.nodes.iter() {
            let Some(actual) = reopened
                .archive
                .nodes
                .get(&source.conversation_id, &source.id)
            else {
                drop(reopened);
                if output_guard.validate_path_identity().is_ok() {
                    let _ = fs::remove_file(output_path);
                }
                return Err(OwnerError::Repack(
                    "principal backfill lost an Archive node".into(),
                ));
            };
            let mut expected = source.clone();
            if expected.role == "user" {
                match expected.principal_id.as_deref() {
                    None => expected.principal_id = Some(principal_id.to_owned()),
                    Some(existing) if existing != principal_id => {
                        drop(reopened);
                        if output_guard.validate_path_identity().is_ok() {
                            let _ = fs::remove_file(output_path);
                        }
                        return Err(OwnerError::Repack(
                            "principal backfill encountered a conflicting user principal".into(),
                        ));
                    }
                    Some(_) => {}
                }
            }
            if actual != &expected {
                drop(reopened);
                if output_guard.validate_path_identity().is_ok() {
                    let _ = fs::remove_file(output_path);
                }
                return Err(OwnerError::Repack(
                    "principal backfill changed Archive node semantics".into(),
                ));
            }
        }

        let output_file_bytes = fs::metadata(output_path)
            .map_err(|error| OwnerError::Repack(error.to_string()))?
            .len();
        Ok(PrincipalBackfillRepackResult {
            rewritten_user_turns,
            preserved_user_turns,
            copied_chunks,
            source_file_bytes,
            output_file_bytes,
        })
    }
}

pub(crate) fn rewrite_missing_user_principal(
    payload: &[u8],
    principal_id: &str,
) -> Result<(Vec<u8>, bool, bool), OwnerError> {
    if let Some(envelope) = crate::container::publication_envelope::decode(payload)? {
        use crate::archive_codec::publication::{self, ArchivePublication};
        if let Some(mut publication) = publication::decode(envelope.body, envelope.global_version)?
        {
            let node = match &mut publication {
                ArchivePublication::Record {
                    record: ArchiveRecord::Node(node),
                    ..
                } => Some(node),
                ArchivePublication::Record {
                    record: ArchiveRecord::IngestedTurn(turn),
                    ..
                } => Some(&mut turn.node),
                _ => None,
            };
            if let Some(node) = node.filter(|node| node.role == "user") {
                match node.principal_id.as_deref() {
                    None => node.principal_id = Some(principal_id.to_owned()),
                    Some(existing) if existing == principal_id => {
                        return Ok((payload.to_vec(), false, true));
                    }
                    Some(_) => {
                        return Err(OwnerError::Repack(
                            "principal backfill encountered a conflicting user principal".into(),
                        ));
                    }
                }
                let body = publication::encode(&publication)?;
                let bytes = crate::container::publication_envelope::encode(
                    crate::container::publication_envelope::PublicationEnvelope {
                        body: &body,
                        ..envelope
                    },
                )?;
                return Ok((bytes, true, false));
            }
            return Ok((payload.to_vec(), false, false));
        }
    }
    match decode_record(payload)? {
        ArchiveRecord::Node(mut node) if node.role == "user" => {
            match node.principal_id.as_deref() {
                None => {
                    node.principal_id = Some(principal_id.to_owned());
                    Ok((encode_node(&node)?, true, false))
                }
                Some(existing) if existing == principal_id => Ok((payload.to_vec(), false, true)),
                Some(_) => Err(OwnerError::Repack(
                    "principal backfill encountered a conflicting user principal".into(),
                )),
            }
        }
        ArchiveRecord::IngestedTurn(mut turn) if turn.node.role == "user" => {
            match turn.node.principal_id.as_deref() {
                None => {
                    turn.node.principal_id = Some(principal_id.to_owned());
                    Ok((encode_ingested_turn(&turn)?, true, false))
                }
                Some(existing) if existing == principal_id => Ok((payload.to_vec(), false, true)),
                Some(_) => Err(OwnerError::Repack(
                    "principal backfill encountered a conflicting user principal".into(),
                )),
            }
        }
        _ => Ok((payload.to_vec(), false, false)),
    }
}
