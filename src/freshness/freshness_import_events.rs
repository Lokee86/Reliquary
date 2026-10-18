//! Import-epoch-scoped provisional Freshness event attribution.
use super::{FreshnessStore, JournalEntry, for_each_event};
use crate::MemoryId;
use std::collections::BTreeSet;

/// Physical and semantic boundary for provisional events attributed to one import.
/// The byte coordinates are accepted Container prefixes captured at ImportStart
/// and freeze; activity cuts alone cannot distinguish receipts at the same turn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportEventScope {
    pub owner: [u8; 16],
    pub import_id: [u8; 32],
    pub start_evidence_digest: [u8; 32],
    /// Memories born in this ImportEpoch; only these authorize initial-pass IDs.
    pub born_memories: BTreeSet<MemoryId>,
    /// Full sparse replacement/effect recipient set, including pre-existing Memories.
    pub affected_recipients: BTreeSet<MemoryId>,
    pub source_nodes: BTreeSet<(String, String)>,
    pub start_prefix: u64,
    pub freeze_prefix: u64,
    pub base_cut: u64,
    pub freeze_cut: u64,
}

impl FreshnessStore {
    /// Select exact owner-local event identities physically appended after the
    /// ImportStart prefix and accepted by the finite freeze prefix.
    ///
    /// This is deliberately a separate API from CVAFRS04 settlement ingestion:
    /// the scope must come from the owner's durable epoch. Selection alone does
    /// not authenticate persisted settlement evidence, and legacy receipts do
    /// not acquire an epoch boundary by inference.
    pub(crate) fn import_events_in_scope(
        &self,
        scope: &ImportEventScope,
    ) -> Result<BTreeSet<String>, String> {
        if scope.owner == [0; 16]
            || scope.import_id == [0; 32]
            || scope.start_evidence_digest == [0; 32]
            || scope.freeze_prefix == u64::MAX
            || scope.start_prefix > scope.freeze_prefix
            || scope.freeze_prefix > self.history_limit
            || scope.base_cut > scope.freeze_cut
        {
            return Err("invalid physical import event scope".into());
        }
        if self.owner_uuid != Some(scope.owner) {
            return Err("import event scope owner mismatch".into());
        }
        let before = self.event_ids_before_prefix(scope.start_prefix)?;
        let through_freeze = self.event_ids_before_prefix(scope.freeze_prefix)?;
        if scope.start_prefix == scope.freeze_prefix {
            return Ok(BTreeSet::new());
        }
        let imported_passes: BTreeSet<_> = scope
            .born_memories
            .iter()
            .map(|id| crate::dream_freshness::initial_pass_id(scope.owner, *id))
            .collect();
        let mut selected = BTreeSet::new();
        self.scan_entries_through_prefix(scope.freeze_prefix, |entry| {
            for_each_event(&entry, &mut |event| {
                let JournalEntry::Event {
                    event_id,
                    turn,
                    provenance,
                    effects,
                    semantic,
                    ..
                } = event
                else {
                    return Ok(());
                };
                if before.contains(event_id)
                    || !through_freeze.contains(event_id)
                    || *turn < scope.base_cut
                    || *turn > scope.freeze_cut
                {
                    return Ok(());
                }
                let import_birth = imported_passes.contains(event_id)
                    && provenance.producer_kind == 1
                    && provenance.source_id == *event_id
                    && provenance.origin_owner == scope.owner;
                let import_source = if let Some(evidence) = semantic {
                    if evidence.owner_uuid != scope.owner {
                        false
                    } else if let Some(fragment) = &evidence.fragment {
                        let has_import_source = fragment
                            .sources
                            .iter()
                            .any(|source| scope.source_nodes.contains(source));
                        if has_import_source
                            && fragment
                                .sources
                                .iter()
                                .any(|source| !scope.source_nodes.contains(source))
                        {
                            return Err(
                                "provisional import fragment mixes import and external sources"
                                    .into(),
                            );
                        }
                        has_import_source && !fragment.sources.is_empty()
                    } else {
                        scope
                            .source_nodes
                            .contains(&(evidence.conversation_id.clone(), evidence.node_id.clone()))
                    }
                } else {
                    false
                };
                if import_birth || import_source {
                    if effects
                        .keys()
                        .any(|id| !scope.affected_recipients.contains(id))
                    {
                        return Err("provisional import event crosses the import cohort".into());
                    }
                    selected.insert(event_id.clone());
                }
                Ok(())
            })
        })?;
        Ok(selected)
    }
    /// Read event identities from the accepted physical Container prefix.
    /// This intentionally bypasses `recent`, whose entries may lie beyond a
    /// requested historical prefix.
    pub(crate) fn event_ids_before_prefix(&self, prefix: u64) -> Result<BTreeSet<String>, String> {
        let mut ids = BTreeSet::new();
        self.scan_entries_through_prefix(prefix, |entry| {
            for_each_event(&entry, &mut |event| {
                if let JournalEntry::Event { event_id, .. } = event {
                    ids.insert(event_id.clone());
                }
                Ok(())
            })
        })?;
        Ok(ids)
    }
    fn scan_entries_through_prefix(
        &self,
        prefix: u64,
        mut visitor: impl FnMut(JournalEntry) -> Result<(), String>,
    ) -> Result<(), String> {
        if prefix == u64::MAX || prefix > self.history_limit {
            return Err("Freshness event prefix is outside the accepted history".into());
        }
        let path = self
            .history_path
            .as_ref()
            .ok_or("physical Freshness history is unavailable")?;
        let expected = self.owner_uuid;
        super::stream::scan_entries(path, prefix, |owner, entry| {
            if expected.is_some_and(|id| id != owner) {
                return Err("Freshness durable scan owner mismatch".into());
            }
            visitor(entry)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::freshness::{FreshnessPolicy, FreshnessRecord};
    use std::collections::BTreeMap;

    fn append_accepted(
        container: &mut crate::Container,
        store: &mut FreshnessStore,
        payload: &[u8],
    ) {
        container.append(payload).unwrap();
        container.sync().unwrap();
        store.ingest_appended(payload).unwrap();
    }

    fn admitted(turn: u64) -> FreshnessRecord {
        let mut record = FreshnessRecord::created();
        record.admit(turn).unwrap();
        record
    }

    fn semantic_evidence(
        owner: [u8; 16],
        source: (String, String),
        fragment: Option<super::super::SemanticFragmentEvidence>,
        turn: u64,
    ) -> super::super::SemanticEventEvidence {
        super::super::SemanticEventEvidence {
            owner_uuid: owner,
            fragment,
            conversation_id: source.0,
            node_id: source.1,
            accepted_turn: turn,
            base_policy_digest: crate::freshness::historical_planner::HISTORICAL_POLICY_DIGEST,
            semantic_policy_version: crate::freshness::semantic_policy::SEMANTIC_POLICY_VERSION,
            semantic_policy_digest: crate::freshness::semantic_policy::SemanticPolicy::default()
                .digest()
                .unwrap(),
            compatibility_profile_id: [38; 32],
            source_evidence_digest: [39; 32],
            target_evidence_digest: [40; 32],
        }
    }
    #[test]
    fn scoped_event_attribution_uses_finite_physical_interval_at_same_cut() {
        use crate::Container;

        let root = std::env::current_dir().unwrap().join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!("import-event-scope-{}.rel", uuid::Uuid::new_v4()));
        let owner = [31; 16];
        let imported = MemoryId([32; 32]);
        let existing_recipient = MemoryId([35; 32]);
        let unrelated_born = MemoryId([36; 32]);
        let mut container = Container::create(&path).unwrap();
        let mut store = FreshnessStore::new(Some(owner));
        store.bind_history_path(&path, 0);

        let policy = super::super::encode_policy(owner, FreshnessPolicy::default_v1());
        append_accepted(&mut container, &mut store, &policy);
        for id in [existing_recipient, unrelated_born] {
            let payload = store.encode_initialize(owner, id, 1, admitted(1));
            append_accepted(&mut container, &mut store, &payload);
        }

        let import_source: (String, String) = ("import-conversation".into(), "import-node".into());
        let pre_start_evidence = semantic_evidence(owner, import_source.clone(), None, 100);
        let pre_start_semantic = store
            .encode_semantic_event(
                owner,
                pre_start_evidence,
                BTreeMap::from([(existing_recipient, 3)]),
            )
            .unwrap();
        append_accepted(&mut container, &mut store, &pre_start_semantic);
        let start_prefix = std::fs::metadata(&path).unwrap().len();

        let imported_birth = store.encode_initialize(owner, imported, 100, admitted(100));
        append_accepted(&mut container, &mut store, &imported_birth);

        let unrelated_live_id = crate::dream_freshness::initial_pass_id(owner, unrelated_born);
        let unrelated_live = store.encode_event(
            owner,
            unrelated_live_id,
            100,
            crate::FreshnessEventProof {
                graph_version: 0,
                community_generation: None,
                community_graph_version: None,
            },
            BTreeMap::from([(existing_recipient, 50)]),
            BTreeMap::new(),
        );
        append_accepted(&mut container, &mut store, &unrelated_live);

        let import_event_id = crate::dream_freshness::initial_pass_id(owner, imported);
        let import_event = store.encode_event(
            owner,
            import_event_id.clone(),
            100,
            crate::FreshnessEventProof {
                graph_version: 0,
                community_generation: None,
                community_graph_version: None,
            },
            BTreeMap::from([(existing_recipient, 50)]),
            BTreeMap::new(),
        );
        append_accepted(&mut container, &mut store, &import_event);
        let freeze_prefix = std::fs::metadata(&path).unwrap().len();

        let scope = ImportEventScope {
            owner,
            import_id: [33; 32],
            start_evidence_digest: [34; 32],
            born_memories: BTreeSet::from([imported]),
            affected_recipients: BTreeSet::from([existing_recipient]),
            source_nodes: BTreeSet::from([import_source]),
            start_prefix,
            freeze_prefix,
            base_cut: 100,
            freeze_cut: 100,
        };
        assert_eq!(
            store.import_events_in_scope(&scope).unwrap(),
            BTreeSet::from([import_event_id])
        );
        assert!(
            store
                .import_events_in_scope(&ImportEventScope {
                    freeze_prefix: u64::MAX,
                    ..scope.clone()
                })
                .is_err()
        );
        assert!(
            store
                .import_events_in_scope(&ImportEventScope {
                    freeze_prefix: freeze_prefix + 1,
                    ..scope
                })
                .is_err()
        );

        drop(container);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn scoped_fragment_attribution_rejects_mixed_import_and_external_sources() {
        use crate::Container;

        let root = std::env::current_dir().unwrap().join(".test-tmp");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "import-fragment-scope-{}.rel",
            uuid::Uuid::new_v4()
        ));
        let owner = [41; 16];
        let recipient = MemoryId([42; 32]);
        let import_source = ("import-conversation".into(), "import-node".into());
        let external_source = ("older-conversation".into(), "older-node".into());
        let evidence = semantic_evidence(
            owner,
            import_source.clone(),
            Some(super::super::SemanticFragmentEvidence {
                fragment_id: crate::FragmentId([43; 32]),
                sources: vec![import_source.clone(), external_source],
            }),
            100,
        );
        let mut container = Container::create(&path).unwrap();
        let mut store = FreshnessStore::new(Some(owner));
        store.bind_history_path(&path, 0);
        let policy = super::super::encode_policy(owner, FreshnessPolicy::default_v1());
        append_accepted(&mut container, &mut store, &policy);
        let birth = store.encode_initialize(owner, recipient, 1, admitted(1));
        append_accepted(&mut container, &mut store, &birth);
        let start_prefix = std::fs::metadata(&path).unwrap().len();
        let payload = store
            .encode_semantic_event(owner, evidence, BTreeMap::from([(recipient, 3)]))
            .unwrap();
        append_accepted(&mut container, &mut store, &payload);
        let freeze_prefix = std::fs::metadata(&path).unwrap().len();
        store.bind_history_path(&path, freeze_prefix);
        let scope = ImportEventScope {
            owner,
            import_id: [49; 32],
            start_evidence_digest: [50; 32],
            born_memories: BTreeSet::new(),
            affected_recipients: BTreeSet::from([recipient]),
            source_nodes: BTreeSet::from([import_source]),
            start_prefix,
            freeze_prefix,
            base_cut: 100,
            freeze_cut: 100,
        };
        assert!(store.import_events_in_scope(&scope).is_err());

        drop(container);
        std::fs::remove_file(path).unwrap();
    }
}
