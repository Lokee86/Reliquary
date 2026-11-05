use super::*;
#[derive(Default)]
pub(crate) struct ExternalMemoryStore {
    pending: BTreeMap<(String, [u8; 32]), IntentWire>,
    pending_turns: BTreeMap<(String, [u8; 32]), u64>,
    receipts: BTreeMap<(String, [u8; 32]), ExternalMemoryReceipt>,
    owner: Option<[u8; 16]>,
}
impl ExternalMemoryStore {
    pub(crate) fn open(container: &Container, memories: &MemoryStore) -> Result<Self, String> {
        if container.identity().file_kind != FileKind::Reliquary {
            return Err("external Memory publication requires REL".into());
        }
        let mut s = Self {
            owner: Some(container.owner_id()),
            ..Self::default()
        };
        let mut seen_external_creations = std::collections::BTreeSet::new();
        let mut accepted_turns = std::collections::BTreeSet::new();
        let mut rel_turn = 0u64;
        for f in container.read_frames().map_err(|e| e.to_string())? {
            let publication =
                reliquary_container::decode_publication(&f.payload).map_err(|e| e.to_string())?;
            let body = publication
                .as_ref()
                .map_or(f.payload.as_slice(), |p| p.body);
            if let Some(reliquary_archive::SourcePublication::Turn {
                conversation_id,
                node_id,
                counts_activity: true,
            }) = reliquary_archive::source_publication(body).map_err(|e| e.to_string())?
                && accepted_turns.insert((conversation_id, node_id))
            {
                rel_turn = rel_turn
                    .checked_add(1)
                    .ok_or("REL activity count overflow")?;
            }
            if body.starts_with(INTENT) {
                let i = IntentWire::decode(body)
                    .map_err(|e| format!("invalid external Memory intent: {e}"))?;
                if i.owner != container.owner_id() {
                    return Err("foreign external Memory intent".into());
                }
                let key = (i.principal.clone(), i.window);
                if let Some(prev) = s.pending.get(&key) {
                    if prev.digest != i.digest {
                        return Err("conflicting external Memory intent".into());
                    }
                }
                s.pending_turns.entry(key.clone()).or_insert(rel_turn);
                s.pending.insert(key, i);
            } else if body.starts_with(b"RELMCRT1") {
                let creation = reliquary_memory_core::decode_grouped_creation(body)
                    .map_err(|e| e.to_string())?;
                let authorized = s.pending.values().any(|intent| {
                    intent.entries.iter().any(|e| {
                        reliquary_memory_core::memory_id(&e.draft.mutation_id) == creation.id
                            && e.draft.mutation_id == creation.mutation_id
                    })
                });
                if authorized {
                    seen_external_creations.insert(creation.id);
                }
            } else if body.starts_with(RECEIPT) {
                let r = decode_receipt(body).map_err(|e| e.to_string())?;
                if r.owner_id != container.owner_id() {
                    return Err("foreign external Memory receipt".into());
                }
                let key = (r.principal.clone(), r.window);
                let intent = s
                    .pending
                    .get(&key)
                    .ok_or("external Memory receipt lacks intent")?;
                let expected_sources: Vec<_> = intent
                    .entries
                    .iter()
                    .map(|e| ExternalMemoryProvenance {
                        authority_source: e.authority.reference.clone(),
                        content_sources: e.content.iter().map(|c| c.reference.clone()).collect(),
                        grounding_sources: e
                            .grounding
                            .iter()
                            .map(|c| c.reference.clone())
                            .collect(),
                    })
                    .collect();
                if intent.digest != r.digest
                    || r.sources != expected_sources
                    || r.memory_ids
                        .iter()
                        .any(|id| !seen_external_creations.contains(id))
                    || !memories_match(intent, memories, &r.memory_ids)
                {
                    return Err("external Memory receipt does not match published Memories".into());
                }
                s.receipts.insert(key, r);
            }
        }
        Ok(s)
    }
    pub(crate) fn get(&self, p: &str, w: [u8; 32]) -> Option<&ExternalMemoryReceipt> {
        self.receipts.get(&(p.to_owned(), w))
    }
    pub(crate) fn pending(&self, p: &str, w: [u8; 32]) -> Option<&IntentWire> {
        self.pending.get(&(p.to_owned(), w))
    }
    pub(crate) fn intent_matches(&self, p: &str, w: [u8; 32], digest: [u8; 32]) -> bool {
        self.pending(p, w).is_some_and(|i| i.digest == digest)
    }
    pub(crate) fn provenance(
        &self,
        memories: &MemoryStore,
        id: MemoryId,
    ) -> Option<ExternalMemoryProvenance> {
        for intent in self.pending.values() {
            for entry in &intent.entries {
                let expected = reliquary_memory_core::memory_id(&entry.draft.mutation_id);
                if expected == id && memory_matches_entry(memories, id, &entry.draft.clone().into())
                {
                    return Some(ExternalMemoryProvenance {
                        authority_source: entry.authority.reference.clone(),
                        content_sources: entry
                            .content
                            .iter()
                            .map(|c| c.reference.clone())
                            .collect(),
                        grounding_sources: entry
                            .grounding
                            .iter()
                            .map(|c| c.reference.clone())
                            .collect(),
                    });
                }
            }
        }
        None
    }
    pub(crate) fn install_intent(
        &mut self,
        c: &mut Container,
        i: IntentWire,
        accepted_turn: u64,
    ) -> Result<(), ExternalSourceError> {
        if self.owner != Some(c.owner_id()) {
            return Err(ExternalSourceError::Unauthorized);
        }
        let k = (i.principal.clone(), i.window);
        if let Some(old) = self.pending.get(&k) {
            if old.digest != i.digest {
                return Err(ExternalSourceError::Conflict);
            }
            return Ok(());
        }
        c.append(&i.encode()?)
            .map_err(|_| ExternalSourceError::Storage)?;
        c.sync().map_err(|_| ExternalSourceError::Storage)?;
        self.pending_turns.insert(k.clone(), accepted_turn);
        self.pending.insert(k, i);
        Ok(())
    }
    pub(crate) fn finish(
        &mut self,
        c: &mut Container,
        memories: &mut MemoryStore,
        freshness: &mut reliquary_freshness_store::FreshnessStore,
        p: &str,
        w: [u8; 32],
    ) -> Result<ExternalMemoryReceipt, ExternalSourceError> {
        let k = (p.to_owned(), w);
        let intent = self
            .pending
            .get(&k)
            .cloned()
            .ok_or(ExternalSourceError::Unavailable)?;
        let accepted_turn = *self
            .pending_turns
            .get(&k)
            .ok_or(ExternalSourceError::Unavailable)?;
        if let Some(r) = self.receipts.get(&k) {
            for id in &r.memory_ids {
                freshness
                    .create_birth(
                        c,
                        reliquary_freshness_store::FreshnessMemoryId(id.0),
                        accepted_turn,
                        false,
                    )
                    .map_err(|_| ExternalSourceError::Storage)?;
            }
            return Ok(r.clone());
        }
        let entries = intent.entries();
        let mut ids = Vec::with_capacity(entries.len());
        for e in entries {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ExternalSourceError::Storage)?
                .as_nanos()
                .min(i64::MAX as u128) as i64;
            let result = memories
                .publish_versioned_creation(c, e.draft, now)
                .map_err(|_| ExternalSourceError::Storage)?;
            ids.push(result.revision.record.id);
        }
        c.sync().map_err(|_| ExternalSourceError::Storage)?;
        if !memories_match(&intent, memories, &ids) {
            return Err(ExternalSourceError::InvalidContract);
        }
        for id in &ids {
            freshness
                .create_birth(
                    c,
                    reliquary_freshness_store::FreshnessMemoryId(id.0),
                    accepted_turn,
                    false,
                )
                .map_err(|_| ExternalSourceError::Storage)?;
        }
        let sources = intent
            .entries
            .iter()
            .map(|e| ExternalMemoryProvenance {
                authority_source: e.authority.reference.clone(),
                content_sources: e.content.iter().map(|c| c.reference.clone()).collect(),
                grounding_sources: e.grounding.iter().map(|c| c.reference.clone()).collect(),
            })
            .collect();
        let receipt = ExternalMemoryReceipt {
            owner_id: intent.owner,
            principal: p.to_owned(),
            window: w,
            memory_ids: ids,
            sources,
            digest: intent.digest,
        };
        let bytes = encode_receipt(&receipt)?;
        c.append(&bytes).map_err(|_| ExternalSourceError::Storage)?;
        c.sync().map_err(|_| ExternalSourceError::Storage)?;
        self.receipts.insert(k, receipt.clone());
        Ok(receipt)
    }
}
