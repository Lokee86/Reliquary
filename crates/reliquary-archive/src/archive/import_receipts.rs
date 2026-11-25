use super::*;
use crate::{ImportedConversation, ImportedMessage};
use std::borrow::Cow;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) enum ImportReceiptSlot {
    Resident(Box<ImportReceipt>),
    Leased(Box<LeasedImportReceipt>),
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct LeasedImportReceipt {
    object: ObjectRef,
    frame_digest: [u8; 32],
    owner_uuid: [u8; 16],
    source_namespace: String,
    source_format: String,
    source_digest: [u8; 32],
    receipt_digest: [u8; 32],
    conversations: BTreeMap<String, Option<String>>,
}

/// Accepted receipt identity available without reading its source mappings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ImportReceiptIdentity<'a> {
    pub owner_uuid: [u8; 16],
    pub source_namespace: &'a str,
    pub source_format: &'a str,
    pub source_digest: [u8; 32],
    pub receipt_digest: [u8; 32],
}

impl<'a> From<&'a ImportReceipt> for ImportReceiptIdentity<'a> {
    fn from(receipt: &'a ImportReceipt) -> Self {
        Self {
            owner_uuid: receipt.owner_uuid,
            source_namespace: &receipt.source_namespace,
            source_format: &receipt.source_format,
            source_digest: receipt.source_digest,
            receipt_digest: receipt.receipt_digest,
        }
    }
}

/// An accepted receipt and the source mapping for one native turn.
pub struct ImportedTurnReference<'a> {
    receipt: Cow<'a, ImportReceipt>,
    conversation: usize,
    message: usize,
}

impl ImportedTurnReference<'_> {
    pub fn get(&self) -> (&ImportReceipt, &ImportedConversation, &ImportedMessage) {
        let conversation = &self.receipt.conversations[self.conversation];
        (
            &self.receipt,
            conversation,
            &conversation.messages[self.message],
        )
    }
}

impl ImportReceiptSlot {
    fn identity(&self) -> ImportReceiptIdentity<'_> {
        match self {
            Self::Resident(receipt) => receipt.as_ref().into(),
            Self::Leased(receipt) => ImportReceiptIdentity {
                owner_uuid: receipt.owner_uuid,
                source_namespace: &receipt.source_namespace,
                source_format: &receipt.source_format,
                source_digest: receipt.source_digest,
                receipt_digest: receipt.receipt_digest,
            },
        }
    }

    fn contains(&self, conversation: &str) -> bool {
        match self {
            Self::Resident(receipt) => receipt
                .conversations
                .iter()
                .any(|row| row.conversation_id == conversation),
            Self::Leased(receipt) => receipt.conversations.contains_key(conversation),
        }
    }

    pub(super) fn title(&self, conversation: &str) -> Option<&str> {
        match self {
            Self::Resident(receipt) => receipt
                .conversations
                .iter()
                .find(|row| row.conversation_id == conversation)
                .and_then(|row| row.title.as_deref()),
            Self::Leased(receipt) => receipt.conversations.get(conversation)?.as_deref(),
        }
    }
}

impl Archive {
    pub(super) fn receipt_slot(
        &self,
        receipt: Cow<'_, ImportReceipt>,
        source: Option<(ObjectRef, &[u8])>,
    ) -> Result<ImportReceiptSlot, ArchiveError> {
        if self.reader.is_none() {
            return Ok(ImportReceiptSlot::Resident(Box::new(receipt.into_owned())));
        }
        let (object, payload) =
            source.ok_or(ArchiveError::CorruptRecord("import receipt has no frame"))?;
        Ok(ImportReceiptSlot::Leased(Box::new(LeasedImportReceipt {
            object,
            frame_digest: Sha256::digest(payload).into(),
            owner_uuid: receipt.owner_uuid,
            source_namespace: receipt.source_namespace.clone(),
            source_format: receipt.source_format.clone(),
            source_digest: receipt.source_digest,
            receipt_digest: receipt.receipt_digest,
            conversations: receipt
                .conversations
                .iter()
                .map(|row| (row.conversation_id.clone(), row.title.clone()))
                .collect(),
        })))
    }

    pub fn import_receipt(
        &self,
        import_key: &[u8; 32],
    ) -> Result<Option<Cow<'_, ImportReceipt>>, ArchiveError> {
        self.import_receipts
            .get(import_key)
            .map(|slot| self.read_import_receipt(import_key, slot))
            .transpose()
    }

    pub fn import_receipts(&self) -> Result<Vec<ImportReceipt>, ArchiveError> {
        self.import_receipts
            .iter()
            .map(|(key, slot)| self.read_import_receipt(key, slot).map(Cow::into_owned))
            .collect()
    }

    pub fn import_receipt_count(&self) -> usize {
        self.import_receipts.len()
    }

    pub fn import_receipt_identity(
        &self,
        import_key: &[u8; 32],
    ) -> Option<ImportReceiptIdentity<'_>> {
        self.import_receipts
            .get(import_key)
            .map(ImportReceiptSlot::identity)
    }

    pub fn import_receipt_covers_conversation(&self, conversation: &str) -> bool {
        self.import_receipts
            .values()
            .any(|slot| slot.contains(conversation))
    }

    /// Return one accepted receipt witness for an exact native node.
    pub fn imported_turn_reference(
        &self,
        conversation: &str,
        node: &str,
    ) -> Result<Option<ImportedTurnReference<'_>>, ArchiveError> {
        for (key, slot) in &self.import_receipts {
            if slot.contains(conversation) {
                let receipt = self.read_import_receipt(key, slot)?;
                if let Some((conversation, row)) = receipt
                    .conversations
                    .iter()
                    .enumerate()
                    .find(|(_, row)| row.conversation_id == conversation)
                    && let Some(message) = row
                        .messages
                        .iter()
                        .position(|message| message.node_id.as_deref() == Some(node))
                {
                    return Ok(Some(ImportedTurnReference {
                        receipt,
                        conversation,
                        message,
                    }));
                }
            }
        }
        Ok(None)
    }

    pub(super) fn read_import_receipt<'a>(
        &self,
        key: &[u8; 32],
        slot: &'a ImportReceiptSlot,
    ) -> Result<Cow<'a, ImportReceipt>, ArchiveError> {
        match slot {
            ImportReceiptSlot::Resident(receipt) => Ok(Cow::Borrowed(receipt.as_ref())),
            ImportReceiptSlot::Leased(backing) => {
                let reader = self.reader.as_ref().ok_or(ArchiveError::CorruptRecord(
                    "import receipt read has no owning lease",
                ))?;
                let bytes = reader.read_frame(backing.object, backing.frame_digest)?;
                crate::import_receipt_in_frame(&bytes)
                    .and_then(|receipt| {
                        receipt
                            .filter(|receipt| {
                                receipt.import_key == *key
                                    && ImportReceiptIdentity::from(receipt) == slot.identity()
                                    && receipt.conversations.len() == backing.conversations.len()
                                    && receipt.conversations.iter().all(|row| {
                                        backing.conversations.get(&row.conversation_id)
                                            == Some(&row.title)
                                    })
                            })
                            .map(Cow::Owned)
                            .ok_or(ArchiveError::CorruptRecord(
                                "import receipt backing mismatch",
                            ))
                    })
                    .inspect_err(|_| reader.fence_writes())
            }
        }
    }
}
