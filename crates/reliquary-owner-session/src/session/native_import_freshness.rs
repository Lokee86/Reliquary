//! Owner-authenticated inputs and durable publication for historical Freshness.

use super::*;
use crate::records::import_settlement::source_prefix_digest;
use reliquary_freshness_store::{FreshnessReplayEntry, HistoricalFreshnessSnapshot};
use reliquary_graph::NodeRef;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

impl RelSession {
    pub fn has_native_import_historical_freshness(&self, import_id: &str) -> bool {
        self.freshness.historical_snapshot(import_id).is_some()
    }
}

impl RelWriter {
    pub fn native_import_freshness_input(
        &self,
        import_id: &str,
    ) -> Result<NativeImportFreshnessInput, OwnerSessionError> {
        self.ensure_import_scope(import_id)?;
        let start = self
            .session
            .imports
            .open_epoch()
            .filter(|start| start.import_id == import_id)
            .ok_or_else(|| {
                OwnerSessionError::Record("native import epoch is unavailable".into())
            })?;
        let import_digest = super::decode_import_key(&start.import_key)
            .ok_or_else(|| OwnerSessionError::Record("invalid native import key".into()))?;
        let receipt = self
            .session
            .archive
            .import_receipt(&import_digest)
            .ok_or_else(|| {
                OwnerSessionError::Record("native import receipt is unavailable".into())
            })?;

        let mapped = super::work::mapped_source_nodes(receipt);
        let positions: BTreeMap<_, _> = self
            .session
            .archive
            .accepted_turn_sources()
            .into_iter()
            .filter_map(|source| {
                Some((
                    (source.conversation_id, source.node_id),
                    source.activity_position?,
                ))
            })
            .collect();
        let source_positions: Vec<_> = mapped
            .iter()
            .filter_map(|(conversation, node)| {
                positions
                    .get(&(conversation.to_string(), node.to_string()))
                    .copied()
            })
            .collect();
        let base_cut = source_positions
            .iter()
            .min()
            .copied()
            .and_then(|position| position.checked_sub(1))
            .ok_or_else(|| {
                OwnerSessionError::Record("native import has no accepted source activity".into())
            })?;
        let settlement_cut = self.session.archive.rel_turn_count();

        let cohort: BTreeSet<_> = self
            .native_import_memory_ids(import_id)?
            .into_iter()
            .collect();

        let replay = self.session.freshness.replay_entries();
        let mut births = BTreeMap::new();
        let mut admissions = BTreeMap::new();
        for entry in &replay {
            match entry {
                FreshnessReplayEntry::Birth {
                    memory_id,
                    accepted_turn,
                    admitted_at_birth,
                } => {
                    births.insert(*memory_id, (*accepted_turn, *admitted_at_birth));
                }
                FreshnessReplayEntry::Admission {
                    memory_id,
                    accepted_turn,
                } => {
                    admissions.insert(*memory_id, *accepted_turn);
                }
                _ => {}
            }
        }

        let mut initial_records = self
            .session
            .freshness
            .records_through_turn(base_cut)
            .map_err(OwnerSessionError::Record)?
            .into_iter()
            .map(|(id, record)| (MemoryId(id.0), record))
            .collect::<BTreeMap<_, _>>();
        for (id, (birth, admitted_at_birth)) in &births {
            if *birth <= base_cut || *birth > settlement_cut {
                continue;
            }
            let mut record = FreshnessRecord::created();
            if *admitted_at_birth {
                record
                    .admit(*birth)
                    .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
            }
            initial_records.insert(MemoryId(id.0), record);
        }
        for (id, admission) in &admissions {
            if *admission > base_cut && *admission <= settlement_cut {
                initial_records
                    .get_mut(&MemoryId(id.0))
                    .ok_or_else(|| {
                        OwnerSessionError::Record(
                            "historical Freshness admission lacks its birth".into(),
                        )
                    })?
                    .admit(*admission)
                    .map_err(|error| OwnerSessionError::Record(error.to_string()))?;
            }
        }

        let policy = self.session.freshness.policy();
        let dream_event_ids: BTreeSet<_> = cohort
            .iter()
            .map(|id| dream_event_id(self.session.owner_id, *id))
            .collect();
        let mut retained_events = Vec::new();
        for event in self
            .session
            .freshness
            .accepted_events()
            .into_iter()
            .filter(|event| event.accepted_turn > base_cut && event.accepted_turn <= settlement_cut)
        {
            if dream_event_ids.contains(&event.event_id) {
                continue;
            }
            let event_id: [u8; 32] = Sha256::digest(
                [
                    b"reliquary:retained-freshness:v1\0".as_slice(),
                    event.event_id.as_bytes(),
                ]
                .concat(),
            )
            .into();
            retained_events.extend(event.effects.into_iter().map(|(recipient, amount)| {
                NativeImportRetainedEvent {
                    recipient: MemoryId(recipient.0),
                    position: event.accepted_turn,
                    event_id,
                    amount,
                }
            }));
        }

        let mut historical_events = Vec::new();
        for settlement in self
            .session
            .common
            .dreams
            .initial_settlements()
            .filter(|settlement| cohort.contains(&settlement.source))
        {
            let position = births
                .get(&FreshnessMemoryId(settlement.source.0))
                .map(|(turn, _)| *turn)
                .ok_or_else(|| {
                    OwnerSessionError::Record(
                        "import Dream source lacks its durable Freshness birth".into(),
                    )
                })?;
            historical_events.push(NativeImportHistoricalEvent {
                source: settlement.source,
                position,
                roots: settlement
                    .credit_roots
                    .iter()
                    .copied()
                    .map(|id| (id, policy.linkage_principal))
                    .collect(),
                credit_excluded: cohort.clone(),
            });
        }
        if historical_events.len() != cohort.len() {
            return Err(OwnerSessionError::Record(
                "historical import requires settled first-cycle Dream evidence for every imported Memory"
                    .into(),
            ));
        }
        historical_events.sort_by_key(|event| (event.position, event.source));

        let authorized: BTreeSet<_> = births.keys().map(|id| MemoryId(id.0)).collect();
        let mut availability = BTreeMap::new();
        for id in &authorized {
            let birth = births[&FreshnessMemoryId(id.0)].0;
            let value = match self.session.common.memories.history(*id) {
                Ok(history)
                    if history.tombstone.is_none()
                        && history.revisions.last().is_some_and(|revision| {
                            !revision.record.archived
                                && revision.record.superseded_by.is_none()
                                && matches!(
                                    revision.record.lifecycle_state.as_str(),
                                    "knowledge" | "canonical"
                                )
                        })
                        && birth > 0 =>
                {
                    NativeImportMemoryAvailability::Supported { position: birth }
                }
                Ok(_) => NativeImportMemoryAvailability::CreditExcluded,
                Err(_) => NativeImportMemoryAvailability::Unsupported,
            };
            availability.insert(*id, value);
        }

        let mut adjacency = BTreeMap::<MemoryId, BTreeSet<MemoryId>>::new();
        for id in &authorized {
            for relation in self.session.common.graph.outgoing(NodeRef::Memory(id.0)) {
                if !relation.active {
                    continue;
                }
                let NodeRef::Memory(other) = relation.target else {
                    continue;
                };
                let other = MemoryId(other);
                if authorized.contains(&other) {
                    adjacency.entry(*id).or_default().insert(other);
                    adjacency.entry(other).or_default().insert(*id);
                }
            }
        }
        let mut memberships = BTreeMap::new();
        if let Some(snapshot) = self.session.common.communities.current() {
            for community in &snapshot.communities {
                for member in &community.members {
                    let id = MemoryId(*member);
                    if authorized.contains(&id) {
                        memberships.insert(id, community.id);
                    }
                }
            }
        }

        let prefix = source_prefix_digest(&self.container).map_err(OwnerSessionError::Record)?;
        let mut digest = Sha256::new();
        digest.update(b"reliquary:native-import-freshness-input:v1\0");
        digest.update(self.session.owner_id);
        digest.update((import_id.len() as u64).to_le_bytes());
        digest.update(import_id.as_bytes());
        digest.update(prefix.as_bytes());
        let evidence_digest = digest.finalize().into();
        Ok(NativeImportFreshnessInput {
            owner_id: self.session.owner_id,
            import_id: import_id.to_owned(),
            import_digest,
            base_cut,
            settlement_cut,
            evidence_digest,
            initial_records,
            historical_events,
            retained_events,
            adjacency,
            memberships,
            authorized,
            availability,
        })
    }

    pub fn publish_native_import_historical_freshness(
        &mut self,
        import_id: &str,
        evidence_digest: [u8; 32],
        records: BTreeMap<MemoryId, FreshnessRecord>,
    ) -> Result<bool, OwnerSessionError> {
        if let Some(existing) = self.session.freshness.historical_snapshot(import_id) {
            let expected: BTreeMap<_, _> = existing
                .records
                .iter()
                .map(|(id, record)| (MemoryId(id.0), *record))
                .collect();
            return if existing.evidence_digest == evidence_digest && expected == records {
                Ok(false)
            } else {
                Err(OwnerSessionError::Record(
                    "conflicting historical Freshness publication retry".into(),
                ))
            };
        }
        let input = self.native_import_freshness_input(import_id)?;
        if input.evidence_digest != evidence_digest {
            return Err(OwnerSessionError::Record(
                "historical Freshness owner evidence changed before publication".into(),
            ));
        }
        let snapshot = HistoricalFreshnessSnapshot {
            owner_uuid: self.session.owner_id,
            import_id: import_id.to_owned(),
            base_cut: input.base_cut,
            settlement_cut: input.settlement_cut,
            journal_entries_included: 0,
            evidence_digest,
            records: records
                .into_iter()
                .map(|(id, record)| (FreshnessMemoryId(id.0), record))
                .collect(),
        };
        let result = self
            .session
            .freshness
            .append_historical_snapshot(&mut self.container, snapshot)
            .map_err(OwnerSessionError::Record);
        match result {
            Ok(value) => {
                self.session.writer_lease_proof = self.container.writer_lease_proof();
                Ok(value)
            }
            Err(error) => {
                self.session.write_poisoned = true;
                Err(error)
            }
        }
    }
}

fn dream_event_id(owner: [u8; 16], source: MemoryId) -> String {
    format!("dream-initial:{}:{}", hex(&owner), hex(&source.0))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
