use crate::imports::ImportReceipt;
use crate::{Archive, ArchiveError, Container};
pub(crate) const IMPORT_MAGIC: &[u8; 8] = b"CVAIMPT1";

pub(crate) fn encode(receipt: &ImportReceipt) -> Result<Vec<u8>, ArchiveError> {
    let mut out = IMPORT_MAGIC.to_vec();
    out.extend(
        serde_json::to_vec(receipt).map_err(|_| ArchiveError::InvalidField("import receipt"))?,
    );
    Ok(out)
}
pub(crate) fn decode(payload: &[u8]) -> Result<ImportReceipt, ArchiveError> {
    let receipt: ImportReceipt = serde_json::from_slice(&payload[8..])
        .map_err(|_| ArchiveError::CorruptRecord("import receipt"))?;
    if receipt.schema_version != 1
        || receipt.import_key.len() != 64
        || !receipt.import_key.bytes().all(|b| b.is_ascii_hexdigit())
        || receipt.source_namespace.is_empty()
        || receipt.source_format.is_empty()
    {
        return Err(ArchiveError::CorruptRecord("import receipt identity"));
    }
    Ok(receipt)
}
impl Archive {
    pub(crate) fn put_import_receipt(
        &mut self,
        container: &mut Container,
        receipt: ImportReceipt,
    ) -> Result<bool, ArchiveError> {
        self.validate_import_receipt(&receipt)?;
        if let Some(existing) = self.import_receipts.get(&receipt.import_key) {
            return if existing == &receipt {
                Ok(false)
            } else {
                Err(ArchiveError::CorruptRecord("conflicting import receipt"))
            };
        }
        let record = container.append(&encode(&receipt)?)?;
        self.publish_record(container, record)?;
        self.import_receipts
            .insert(receipt.import_key.clone(), receipt);
        Ok(true)
    }
    pub(crate) fn validate_import_revisions(
        &self,
        receipt: &ImportReceipt,
    ) -> Result<(), ArchiveError> {
        let mut incoming = std::collections::BTreeMap::new();
        for conversation in &receipt.conversations {
            for message in &conversation.messages {
                let key = (
                    conversation.source_id.as_str(),
                    message.source_id.as_str(),
                    message.revision.as_str(),
                );
                if incoming.insert(key, message.fingerprint.as_str()).is_some() {
                    return Err(ArchiveError::InvalidField(
                        "duplicate import source revision",
                    ));
                }
            }
        }
        for prior in self
            .import_receipts
            .values()
            .filter(|p| p.source_namespace == receipt.source_namespace)
        {
            for conversation in &prior.conversations {
                for message in &conversation.messages {
                    let key = (
                        conversation.source_id.as_str(),
                        message.source_id.as_str(),
                        message.revision.as_str(),
                    );
                    if incoming
                        .get(&key)
                        .is_some_and(|fingerprint| *fingerprint != message.fingerprint)
                    {
                        return Err(ArchiveError::InvalidField(
                            "conflicting import source revision",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn validate_import_receipt(
        &self,
        receipt: &ImportReceipt,
    ) -> Result<(), ArchiveError> {
        self.validate_import_revisions(receipt)?;
        for conversation in &receipt.conversations {
            for message in &conversation.messages {
                if message.deleted != message.node_id.is_none() {
                    return Err(ArchiveError::CorruptRecord("import deletion marker"));
                }
                if let Some(node_id) = &message.node_id {
                    self.require_node(
                        &conversation.conversation_id,
                        node_id,
                        ArchiveError::MissingNode,
                    )?;
                    let attached = self.files_for_source(&conversation.conversation_id, node_id);
                    for artifact in &message.artifacts {
                        if !attached.iter().any(|f| {
                            f.id.0
                                .iter()
                                .map(|b| format!("{b:02x}"))
                                .collect::<String>()
                                == artifact.file_id
                                && f.content_id
                                    .0
                                    .iter()
                                    .map(|b| format!("{b:02x}"))
                                    .collect::<String>()
                                    == artifact.sha256
                        }) {
                            return Err(ArchiveError::MissingFile);
                        }
                    }
                }
            }
            for branch in &conversation.branches {
                if let Some(leaf) = &branch.leaf_node_id {
                    let native = self
                        .branches
                        .get(&conversation.conversation_id, &branch.branch_id)
                        .ok_or(ArchiveError::MissingBranch)?;
                    if !self
                        .branch_nodes(&conversation.conversation_id, &native.leaf_node_id)?
                        .iter()
                        .any(|node| &node.id == leaf)
                    {
                        return Err(ArchiveError::CorruptRecord("import branch receipt"));
                    }
                }
            }
        }
        Ok(())
    }
}
