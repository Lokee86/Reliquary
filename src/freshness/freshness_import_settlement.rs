//! Ordered sparse current semantic Historical Freshness settlements.
use super::{FreshnessStore, JournalEntry, for_each_event};
use crate::{
    MemoryId,
    freshness::{FreshnessPolicy, FreshnessRecord},
};
use std::collections::{BTreeMap, BTreeSet};

/// Immutable structurally validated binding facts; this is not an accepted
/// settlement and cannot install records or replace independent event checking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SettlementBinding {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub global_version: u64,
    pub archive_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalImportFreshnessScope {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub base_cut: u64,
    pub settlement_cut: u64,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportScopeIdentity {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub base_cut: u64,
    pub born_memories: BTreeSet<MemoryId>,
    pub affected_recipients: BTreeSet<MemoryId>,
    pub source_nodes: BTreeSet<(String, String)>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HistoricalImportFreshnessSettlement {
    pub causal: Option<crate::freshness::historical_causal_basis::HistoricalCausalBasis>,
    /// Transient construction scope; accepted on-disk authority is semantic only.
    pub epoch: Option<super::ImportEventScope>,
    pub semantic: Option<super::semantic_settlement::ImportSemanticScope>,
    pub epoch_prefix_digest: [u8; 32],
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    pub corpus_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub input_digest: [u8; 32],
    pub cohort: BTreeSet<MemoryId>,
    pub base_cut: u64,
    pub settlement_cut: u64,
    pub global_version: u64,
    pub input_bytes: u64,
    pub archive_version: u64,
    pub graph_version: u64,
    pub memory_graph_version: u64,
    pub records: BTreeMap<MemoryId, FreshnessRecord>,
    pub superseded: BTreeSet<String>,
}

impl HistoricalImportFreshnessSettlement {
    pub(crate) fn encode(&self) -> Vec<u8> {
        self.try_encode().expect("validated semantic settlement")
    }
    pub(crate) fn try_encode(&self) -> Result<Vec<u8>, String> {
        let scope = self
            .semantic
            .as_ref()
            .ok_or("accepted settlement lacks semantic authority")?;
        let basis = self
            .causal
            .as_ref()
            .ok_or("accepted settlement lacks causal numeric authority")?;
        super::compact_settlement::encode(&self.as_semantic(scope.clone()), basis)
    }
    #[cfg(test)]
    pub(crate) fn decode(
        payload: &[u8],
        leaves: &BTreeMap<String, super::AcceptedImportEventEvidence>,
    ) -> Result<Self, String> {
        let (value, basis) = super::compact_settlement::decode(payload)?.resolve(leaves)?;
        let mut settlement = Self::from_semantic(value);
        settlement.causal = Some(basis);
        Ok(settlement)
    }
    pub(crate) fn decode_binding(payload: &[u8]) -> Result<SettlementBinding, String> {
        Ok(super::compact_settlement::decode(payload)?.binding())
    }

    pub(crate) fn as_semantic(
        &self,
        scope: super::semantic_settlement::ImportSemanticScope,
    ) -> super::semantic_settlement::SemanticImportFreshnessSettlement {
        super::semantic_settlement::SemanticImportFreshnessSettlement {
            scope,
            corpus_digest: self.corpus_digest,
            manifest_digest: self.manifest_digest,
            input_digest: self.input_digest,
            global_version: self.global_version,
            archive_version: self.archive_version,
            graph_version: self.graph_version,
            memory_graph_version: self.memory_graph_version,
            cohort: self.cohort.clone(),
            records: self.records.clone(),
        }
    }
    pub(crate) fn from_semantic(
        value: super::semantic_settlement::SemanticImportFreshnessSettlement,
    ) -> Self {
        let scope = value.scope;
        Self {
            causal: None,
            owner: scope.owner,
            import_id: scope.import_id,
            start_evidence_digest: scope.start_evidence_digest,
            base_cut: scope.base_cut,
            settlement_cut: scope.settlement_cut,
            superseded: scope.superseded.clone(),
            epoch: None,
            epoch_prefix_digest: [0; 32],
            input_bytes: 0,
            semantic: Some(scope),
            corpus_digest: value.corpus_digest,
            manifest_digest: value.manifest_digest,
            input_digest: value.input_digest,
            global_version: value.global_version,
            archive_version: value.archive_version,
            graph_version: value.graph_version,
            memory_graph_version: value.memory_graph_version,
            cohort: value.cohort,
            records: value.records,
        }
    }
    pub(crate) fn scope_identity(&self) -> Option<ImportScopeIdentity> {
        if let Some(scope) = &self.semantic {
            return Some(ImportScopeIdentity {
                owner: scope.owner,
                import_id: scope.import_id,
                start_evidence_digest: scope.start_evidence_digest,
                base_cut: scope.base_cut,
                born_memories: scope.born_memories.clone(),
                affected_recipients: scope.affected_recipients.clone(),
                source_nodes: scope.source_nodes.clone(),
            });
        }
        self.epoch.as_ref().map(|scope| ImportScopeIdentity {
            owner: scope.owner,
            import_id: scope.import_id,
            start_evidence_digest: scope.start_evidence_digest,
            base_cut: scope.base_cut,
            born_memories: scope.born_memories.clone(),
            affected_recipients: scope.affected_recipients.clone(),
            source_nodes: scope.source_nodes.clone(),
        })
    }
    pub(crate) fn is_scoped(&self) -> bool {
        self.semantic.is_some() || self.epoch.is_some()
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_import_scope(
        scope: HistoricalImportFreshnessScope,
        corpus_digest: [u8; 32],
        manifest_digest: [u8; 32],
        input_digest: [u8; 32],
        cohort: BTreeSet<MemoryId>,
        global_version: u64,
        input_bytes: u64,
        archive_version: u64,
        graph_version: u64,
        memory_graph_version: u64,
        records: BTreeMap<MemoryId, FreshnessRecord>,
        superseded: BTreeSet<String>,
    ) -> Self {
        Self {
            causal: None,
            epoch: None,
            semantic: None,
            epoch_prefix_digest: [0; 32],
            owner: scope.owner,
            import_id: scope.import_id,
            start_evidence_digest: scope.start_evidence_digest,
            corpus_digest,
            manifest_digest,
            input_digest,
            cohort,
            base_cut: scope.base_cut,
            settlement_cut: scope.settlement_cut,
            global_version,
            input_bytes,
            archive_version,
            graph_version,
            memory_graph_version,
            records,
            superseded,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if let Some(scope) = &self.semantic {
            if self.epoch.is_some()
                || self.epoch_prefix_digest != [0; 32]
                || self.input_bytes != 0
                || scope.owner != self.owner
                || scope.import_id != self.import_id
                || scope.start_evidence_digest != self.start_evidence_digest
                || scope.base_cut != self.base_cut
                || scope.settlement_cut != self.settlement_cut
                || scope.superseded != self.superseded
            {
                return Err(
                    "semantic settlement contains conflicting compatibility evidence".into(),
                );
            }
            self.as_semantic(scope.clone()).validate()?;
            if let Some(basis) = &self.causal
                && basis.reconstruct(self.settlement_cut)? != self.records
            {
                return Err("causal basis does not reproduce accepted settlement".into());
            }
            return Ok(());
        }
        if (self.epoch.is_none()
            && (self.settlement_cut == 0
                || self.base_cut >= self.settlement_cut
                || self.records.is_empty()
                || self.cohort.is_empty()))
            || self.base_cut > self.settlement_cut
            || self.cohort.len() > u32::MAX as usize
            || self.records.keys().any(|id| !self.cohort.contains(id))
            || self.records.len() > u32::MAX as usize
            || self.superseded.len() > u32::MAX as usize
            || self.import_id == [0; 32]
            || self.start_evidence_digest == [0; 32]
            || self.manifest_digest == [0; 32]
            || self.memory_graph_version > self.graph_version
        {
            return Err("invalid import settlement identity/cohort".into());
        }
        if let Some(scope) = &self.epoch {
            if scope.owner != self.owner
                || scope.import_id != self.import_id
                || scope.start_evidence_digest != self.start_evidence_digest
                || scope.base_cut != self.base_cut
                || scope.freeze_cut != self.settlement_cut
                || scope.start_prefix >= scope.freeze_prefix
                || scope.freeze_prefix != self.input_bytes
                || scope.freeze_prefix == u64::MAX
                || self.input_digest == [0; 32]
                || self.epoch_prefix_digest == [0; 32]
                || !scope.born_memories.is_subset(&self.cohort)
                || !self.cohort.is_subset(&scope.affected_recipients)
                || self.records.keys().copied().collect::<BTreeSet<_>>() != self.cohort
            {
                return Err("invalid authenticated import settlement scope".into());
            }
        }
        for record in self.records.values() {
            if record.admitted_at_turn.is_none() {
                // The owner manifest may replace provisional admission for a
                // supported but final credit-excluded Memory. Its sole valid
                // unadmitted projection is the canonical creation record.
                if *record != FreshnessRecord::created() {
                    return Err("invalid unadmitted import replacement record".into());
                }
                continue;
            }
            let mut settled = *record;
            settled
                .settle_with_policy(self.settlement_cut, &FreshnessPolicy::default_v1())
                .map_err(|e| e.to_string())?;
            if settled != *record
                || record
                    .admitted_at_turn
                    .is_some_and(|at| at == 0 || at > self.settlement_cut)
            {
                return Err("import records are not canonical at their verified cut".into());
            }
        }
        Ok(())
    }
}

impl FreshnessStore {
    pub(crate) fn settlement_absorbs_event(
        &self,
        settlement: &HistoricalImportFreshnessSettlement,
        event: &JournalEntry,
    ) -> Result<bool, String> {
        let JournalEntry::Event { event_id, .. } = event else {
            return Ok(false);
        };
        if let Some(scope) = &settlement.semantic {
            let Some(expected) = scope.accepted_events.get(event_id) else {
                return Ok(false);
            };
            if super::AcceptedImportEventEvidence::from_event(scope.owner, event)? != *expected {
                return Err("accepted semantic import event changed".into());
            }
            return Ok(true);
        }
        Err("accepted settlement lacks semantic authority".into())
    }
    pub(crate) fn import_settlements(&self) -> &[HistoricalImportFreshnessSettlement] {
        &self.import_settlements
    }
    pub(crate) fn import_settlement(
        &self,
        import_id: [u8; 32],
    ) -> Option<&HistoricalImportFreshnessSettlement> {
        self.import_settlements
            .iter()
            .find(|settlement| settlement.import_id == import_id)
    }
    pub(crate) fn latest_import_settlement(&self) -> Option<&HistoricalImportFreshnessSettlement> {
        self.import_settlements.last()
    }
    pub(crate) fn provisional_import_events(
        &self,
        affected: &BTreeSet<MemoryId>,
        base_cut: u64,
        settlement_cut: u64,
    ) -> Result<BTreeSet<String>, String> {
        let mut ids = BTreeSet::new();
        self.scan_entries(|entry| {
            for_each_event(&entry, &mut |event| {
                if let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    effects,
                    ..
                } = event
                {
                    if *turn > base_cut
                        && *turn <= settlement_cut
                        && provenance.producer_kind == 1
                        && effects.keys().any(|id| affected.contains(id))
                    {
                        if effects.keys().any(|id| !affected.contains(id)) {
                            return Err(
                                "provisional event crosses the replacement/untouched boundary"
                                    .into(),
                            );
                        }
                        ids.insert(event_id.clone());
                    }
                }
                Ok(())
            })
        })?;
        Ok(ids)
    }
    pub(crate) fn ingest_import_settlement(&mut self, payload: &[u8]) -> Result<(), String> {
        let pending = super::compact_settlement::decode(payload)?;
        let binding = pending.binding();
        if self.owner_uuid != Some(binding.owner)
            || self.policy != FreshnessPolicy::default_v1()
            || !self.policy_persisted
        {
            return Err("import settlement owner/base policy mismatch".into());
        }
        let mut leaves = BTreeMap::new();
        self.scan_entries(|entry| {
            for_each_event(&entry, &mut |event| {
                if let JournalEntry::Event { event_id, .. } = event
                    && pending.event_references().contains_key(event_id)
                {
                    let leaf =
                        super::AcceptedImportEventEvidence::from_event(binding.owner, event)?;
                    if leaves
                        .insert(event_id.clone(), leaf.clone())
                        .is_some_and(|old| old != leaf)
                    {
                        return Err("conflicting retained settlement event leaves".into());
                    }
                }
                Ok(())
            })
        })?;
        let (value, basis) = pending.resolve(&leaves)?;
        let mut settlement = HistoricalImportFreshnessSettlement::from_semantic(value);
        settlement.causal = Some(basis);
        if let Some(scope) = &settlement.semantic {
            self.validate_semantic_import_evidence(scope)?;
        }
        if let Some(old) = self.import_settlement(settlement.import_id) {
            return if old == &settlement {
                Ok(())
            } else {
                Err("ImportManifestConflict: import already settled".into())
            };
        }
        if self.import_settlements.last().is_some_and(|previous| {
            (!settlement.is_scoped()
                && (previous.settlement_cut, previous.import_id)
                    >= (settlement.settlement_cut, settlement.import_id))
                || settlement.base_cut < previous.settlement_cut
                || (settlement.epoch.is_some()
                    && previous.input_bytes >= settlement.epoch.as_ref().unwrap().start_prefix)
        }) {
            return Err("import settlements are not in accepted epoch order".into());
        }
        if settlement.cohort.iter().any(|id| {
            !self
                .births
                .get(id)
                .is_some_and(|(birth, _)| *birth <= settlement.settlement_cut)
        }) {
            return Err("import settlement cohort lacks accepted birth proof".into());
        }
        self.records.extend(settlement.records.clone());
        self.import_settlements.push(settlement);
        Ok(())
    }
}
