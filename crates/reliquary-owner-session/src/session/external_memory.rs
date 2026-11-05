use super::*;
use crate::records::external_index::{
    ExternalChunkRef, ExternalMessageDescriptor, ExternalSourceError,
};
use crate::records::external_memory::{ExternalMemoryDraft, ExternalMemoryReceipt, new_intent};
use std::collections::BTreeSet;

fn validate_chunk(chunk: &ExternalChunkRef) -> Result<(), ExternalSourceError> {
    chunk.message.validate()?;
    if chunk.start >= chunk.end
        || chunk.end > crate::records::external_index::model::MAX_EXTERNAL_MESSAGE_BYTES as u64
    {
        return Err(ExternalSourceError::InvalidContract);
    }
    Ok(())
}
fn descriptor<'a>(
    manifest: &'a ExternalReferenceSnapshotManifest,
    chunk: &ExternalChunkRef,
) -> Result<&'a ExternalMessageDescriptor, ExternalSourceError> {
    validate_chunk(chunk)?;
    if chunk.message.source_namespace != manifest.namespace {
        return Err(ExternalSourceError::StaleEvidence);
    }
    manifest
        .descriptors
        .iter()
        .find(|d| d.active && d.reference == chunk.message)
        .ok_or(ExternalSourceError::StaleEvidence)
}
fn validate_entries(
    session: &RelSession,
    principal: &str,
    entries: &[ExternalMemoryDraft],
) -> Result<(), ExternalSourceError> {
    if entries.len() > 64 {
        return Err(ExternalSourceError::Bounds);
    }
    let mut mutations = BTreeSet::new();
    let mut namespace = None;
    for entry in entries {
        reliquary_memory_core::validate_memory_draft(&entry.draft)
            .map_err(|_| ExternalSourceError::InvalidContract)?;
        if !mutations.insert(entry.draft.mutation_id.clone())
            || reliquary_memory_core::memory_id(&entry.draft.mutation_id).0 == [0; 32]
        {
            return Err(ExternalSourceError::InvalidContract);
        }
        let name = entry.authority_source.message.source_namespace.as_str();
        if namespace.is_some_and(|n| n != name) {
            return Err(ExternalSourceError::InvalidContract);
        }
        namespace = Some(name);
        let manifest = session
            .external_references
            .get(name, principal)
            .ok_or(ExternalSourceError::RebuildRequired)?
            .manifest();
        let authority = descriptor(manifest, &entry.authority_source)?;
        if authority.role != "user"
            || authority
                .principal_id
                .as_deref()
                .is_some_and(|p| p != principal)
        {
            return Err(ExternalSourceError::Unauthorized);
        }
        let mut earliest = authority.timestamp_ns;
        for source in &entry.content_sources {
            let d = descriptor(manifest, source)?;
            if d.role != "assistant" || d.timestamp_ns > authority.timestamp_ns {
                return Err(ExternalSourceError::InvalidContract);
            }
            earliest = earliest.min(d.timestamp_ns);
        }
        for source in &entry.grounding_sources {
            let d = descriptor(manifest, source)?;
            if d.timestamp_ns > authority.timestamp_ns {
                return Err(ExternalSourceError::InvalidContract);
            }
        }
        if entry.draft.source_time_ns != Some(earliest) {
            return Err(ExternalSourceError::InvalidContract);
        }
    }
    Ok(())
}

impl RelSession {
    pub fn external_memory_receipt(
        &self,
        principal: &str,
        window: [u8; 32],
    ) -> Option<&ExternalMemoryReceipt> {
        self.external_memories.get(principal, window)
    }
    pub fn external_memory_provenance(
        &self,
        id: MemoryId,
    ) -> Option<crate::records::ExternalMemoryProvenance> {
        self.external_memories.provenance(&self.common.memories, id)
    }
}

impl RelWriter {
    pub fn resume_external_memory_publication(
        &mut self,
        principal: &str,
        window: [u8; 32],
    ) -> Result<Option<ExternalMemoryReceipt>, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL owner session is stale; reopen before external Memory publication".into(),
            ));
        }
        if self
            .session
            .external_memories
            .pending(principal, window)
            .is_none()
        {
            return Ok(None);
        }
        match self.session.external_memories.finish(
            &mut self.container,
            &mut self.session.common.memories,
            &mut self.session.freshness,
            principal,
            window,
        ) {
            Ok(receipt) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                if let Err(error) = self
                    .session
                    .common
                    .refresh_dream_index(receipt.memory_ids())
                {
                    self.session.write_poisoned = true;
                    return Err(error);
                }
                Ok(Some(receipt))
            }
            Err(error) => {
                self.session.write_poisoned = true;
                Err(OwnerSessionError::Record(error.to_string()))
            }
        }
    }

    pub fn publish_external_memory(
        &mut self,
        principal: &str,
        window: [u8; 32],
        entries: Vec<ExternalMemoryDraft>,
    ) -> Result<ExternalMemoryReceipt, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL owner session is stale; reopen before external Memory publication".into(),
            ));
        }
        validate_entries(&self.session, principal, &entries)
            .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
        let intent = new_intent(self.session.owner_id, principal, window, &entries)
            .map_err(|e| OwnerSessionError::Record(e.to_string()))?;
        if let Some(receipt) = self.session.external_memories.get(principal, window) {
            if !self
                .session
                .external_memories
                .intent_matches(principal, window, intent.digest())
            {
                return Err(OwnerSessionError::Record(
                    "external Memory window conflicts with durable receipt".into(),
                ));
            }
            let _ = receipt;
            return self.finish_external_memory(principal, window);
        }
        if !self
            .session
            .external_memories
            .intent_matches(principal, window, intent.digest())
        {
            if let Err(error) = self.session.external_memories.install_intent(
                &mut self.container,
                intent,
                self.session.archive.rel_turn_count(),
            ) {
                self.session.write_poisoned = true;
                return Err(OwnerSessionError::Record(error.to_string()));
            }
            self.session.writer_lease_proof = self.container.writer_lease_proof();
        }
        self.finish_external_memory(principal, window)
    }

    fn finish_external_memory(
        &mut self,
        principal: &str,
        window: [u8; 32],
    ) -> Result<ExternalMemoryReceipt, OwnerSessionError> {
        match self.session.external_memories.finish(
            &mut self.container,
            &mut self.session.common.memories,
            &mut self.session.freshness,
            principal,
            window,
        ) {
            Ok(receipt) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                if let Err(error) = self
                    .session
                    .common
                    .refresh_dream_index(receipt.memory_ids())
                {
                    self.session.write_poisoned = true;
                    return Err(error);
                }
                Ok(receipt)
            }
            Err(error) => {
                self.session.write_poisoned = true;
                Err(OwnerSessionError::Record(error.to_string()))
            }
        }
    }
}

#[cfg(test)]
#[path = "external_memory_tests.rs"]
mod tests;
