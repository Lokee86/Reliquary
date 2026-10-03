use crate::archive_codec::{ArchiveRecord, decode_record, encode_node};
use crate::storage_reclamation::relocate_payload;
use crate::turn_ingest_codec::encode_ingested_turn;
use crate::{Cva, CvaError, ObjectRef};
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

impl Cva {
    /// Repack a single-principal REL while filling missing principal IDs on user turns.
    ///
    /// Existing principal IDs are preserved when they match the supplied principal ID.
    /// A conflicting user-turn principal fails closed. Non-user turns are never assigned one.
    pub fn repack_missing_user_principal(
        &mut self,
        output_path: impl AsRef<Path>,
        principal_id: &str,
    ) -> Result<PrincipalBackfillRepackResult, CvaError> {
        if !crate::entity_principal::is_phy_principal_id(principal_id) {
            return Err(CvaError::Repack(
                "principal backfill requires a canonical phy-UUID".into(),
            ));
        }
        let output_path = output_path.as_ref();
        self.container.sync()?;
        let source_file_bytes = fs::metadata(self.container.path())
            .map_err(|error| CvaError::Repack(error.to_string()))?
            .len();
        let payloads = self.container.object_payloads()?;
        let mut output = self.container.create_empty_like(output_path)?;
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
                let payload = relocate_payload(&payload, &relocated).map_err(CvaError::Repack)?;
                let destination_ref = output.append(&payload)?;
                relocated.insert(source_ref, destination_ref);
                copied_chunks += 1;
            }
            output.sync()?;
            Ok::<(), CvaError>(())
        })();

        if let Err(error) = copy_result {
            drop(output);
            let _ = fs::remove_file(output_path);
            return Err(error);
        }
        drop(output);

        let reopened = match Cva::open(output_path) {
            Ok(reopened) => reopened,
            Err(error) => {
                let _ = fs::remove_file(output_path);
                return Err(error);
            }
        };
        if reopened.owner_uuid() != self.owner_uuid()
            || reopened.latest_global_version() != self.latest_global_version()
            || reopened.archive_version() != self.archive_version()
            || !self.archive.activity_history_matches(&reopened.archive)
            || reopened.memory_version() != self.memory_version()
            || reopened.entity_version() != self.entity_version()
            || reopened.graph_version() != self.graph_version()
            || reopened.stats() != self.stats()
        {
            drop(reopened);
            let _ = fs::remove_file(output_path);
            return Err(CvaError::Repack(
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
                let _ = fs::remove_file(output_path);
                return Err(CvaError::Repack(
                    "principal backfill lost an Archive node".into(),
                ));
            };
            let mut expected = source.clone();
            if expected.role == "user" {
                match expected.principal_id.as_deref() {
                    None => expected.principal_id = Some(principal_id.to_owned()),
                    Some(existing) if existing != principal_id => {
                        drop(reopened);
                        let _ = fs::remove_file(output_path);
                        return Err(CvaError::Repack(
                            "principal backfill encountered a conflicting user principal".into(),
                        ));
                    }
                    Some(_) => {}
                }
            }
            if actual != &expected {
                drop(reopened);
                let _ = fs::remove_file(output_path);
                return Err(CvaError::Repack(
                    "principal backfill changed Archive node semantics".into(),
                ));
            }
        }

        let output_file_bytes = fs::metadata(output_path)
            .map_err(|error| CvaError::Repack(error.to_string()))?
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

fn rewrite_missing_user_principal(
    payload: &[u8],
    principal_id: &str,
) -> Result<(Vec<u8>, bool, bool), CvaError> {
    match decode_record(payload)? {
        ArchiveRecord::Node(mut node) if node.role == "user" => {
            match node.principal_id.as_deref() {
                None => {
                    node.principal_id = Some(principal_id.to_owned());
                    Ok((encode_node(&node)?, true, false))
                }
                Some(existing) if existing == principal_id => Ok((payload.to_vec(), false, true)),
                Some(_) => Err(CvaError::Repack(
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
                Some(_) => Err(CvaError::Repack(
                    "principal backfill encountered a conflicting user principal".into(),
                )),
            }
        }
        _ => Ok((payload.to_vec(), false, false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntityDraft;
    use std::path::PathBuf;

    const PRINCIPAL: &str = "phy-00000000-0000-0000-0000-000000000123";

    #[test]
    fn repack_backfills_only_user_turns_and_preserves_entity_state() {
        let source_path = temp_path("principal-repack-source");
        let output_path = temp_path("principal-repack-output");
        let mut source = Cva::create_project(&source_path).unwrap();
        source
            .append_node("u1".into(), "c1".into(), None, "user".into(), 10, "hello")
            .unwrap();
        source
            .append_node(
                "a1".into(),
                "c1".into(),
                Some("u1".into()),
                "assistant".into(),
                11,
                "hi",
            )
            .unwrap();
        let (entity, _) = source
            .publish_entity(
                None,
                0,
                EntityDraft {
                    canonical_name: "SQLite".into(),
                    aliases: Vec::new(),
                    kind: "technology".into(),
                    summary: "Database".into(),
                    mutation_id: "principal-repack-sqlite".into(),
                    created_at_ns: 1,
                    updated_at_ns: 1,
                },
            )
            .unwrap();
        let owner = source.owner_uuid();
        let versions = (
            source.latest_global_version(),
            source.archive_version(),
            source.memory_version(),
            source.entity_version(),
            source.graph_version(),
        );

        let result = source
            .repack_missing_user_principal(&output_path, PRINCIPAL)
            .unwrap();
        assert_eq!(result.rewritten_user_turns, 1);
        assert_eq!(result.preserved_user_turns, 0);

        let reopened = Cva::open(&output_path).unwrap();
        let user = reopened.archive.nodes.get("c1", "u1").unwrap();
        let assistant = reopened.archive.nodes.get("c1", "a1").unwrap();
        assert_eq!(user.principal_id.as_deref(), Some(PRINCIPAL));
        assert_eq!(assistant.principal_id, None);
        assert_eq!(reopened.owner_uuid(), owner);
        assert_eq!(
            (
                reopened.latest_global_version(),
                reopened.archive_version(),
                reopened.memory_version(),
                reopened.entity_version(),
                reopened.graph_version(),
            ),
            versions
        );
        assert_eq!(reopened.entity(entity.id).unwrap().canonical_name, "SQLite");

        drop(reopened);
        drop(source);
        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn repack_rejects_conflicting_existing_user_principal() {
        let source_path = temp_path("principal-repack-conflict-source");
        let output_path = temp_path("principal-repack-conflict-output");
        let mut source = Cva::create_project(&source_path).unwrap();
        source
            .append_node_with_principal(
                "u1".into(),
                "c1".into(),
                None,
                "user".into(),
                Some("phy-00000000-0000-0000-0000-000000000999".into()),
                10,
                "hello",
            )
            .unwrap();

        assert!(matches!(
            source.repack_missing_user_principal(&output_path, PRINCIPAL),
            Err(CvaError::Repack(message))
                if message.contains("conflicting user principal")
        ));
        assert!(!output_path.exists());

        drop(source);
        let _ = fs::remove_file(source_path);
    }

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{}.rel", uuid::Uuid::new_v4()))
    }
}
