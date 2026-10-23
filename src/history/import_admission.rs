//! Canonical corpus and complete native grouped-publication/provenance evidence.
use crate::history::manifest_encoding::Value;
use crate::owner_lifecycle::OwnerError;
use crate::{EpisodeOrigin, MemoryId, freshness::FreshnessRecord};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) use crate::history::source_proof::{node_binding, required_nodes, source_key};
pub(crate) fn corpus_digest(
    cva: &ImportAdmissionReadParts<'_>,
    through: u64,
) -> Result<[u8; 32], OwnerError> {
    let nodes = required_nodes(&cva.archive, through)?;
    let access = &cva.access;
    crate::history::source_proof::corpus_digest_with_content(
        &access,
        &cva.archive,
        nodes.into_values().collect(),
        "import source content proof mismatch",
    )
    .map_err(OwnerError::from)
}
pub(crate) fn scoped_corpus_digest(
    cva: &ImportAdmissionReadParts<'_>,
    source_nodes: &BTreeSet<(String, String)>,
) -> Result<[u8; 32], OwnerError> {
    epoch::scoped_corpus_digest(cva, source_nodes)
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalMemoryInput {
    pub body_id: crate::MemoryBodyId,
    pub cohort_key: [u8; 32],
    pub availability: Option<u64>,
    pub credit_eligible: bool,
    pub necessary_sources: BTreeSet<crate::freshness::historical_planner::SourceKey>,
}
pub(crate) struct AdmissionProof {
    pub memory_inputs: BTreeMap<MemoryId, HistoricalMemoryInput>,
    pub initial: BTreeMap<MemoryId, FreshnessRecord>,
    pub memories: Vec<Value>,
    pub cohorts: Vec<Value>,
}
pub(crate) fn admission_proof(
    cva: &ImportAdmissionReadParts<'_>,
) -> Result<AdmissionProof, OwnerError> {
    admission_proof_legacy(cva)
}

pub(crate) fn admission_proof_scoped(
    cva: &ImportAdmissionReadParts<'_>,
    scope: Option<&crate::imports::ImportEpochScope>,
) -> Result<AdmissionProof, OwnerError> {
    match scope {
        Some(scope) => epoch::admission_proof_for_scope(cva, scope),
        None => admission_proof_legacy(cva),
    }
}

fn admission_proof_legacy(
    cva: &ImportAdmissionReadParts<'_>,
) -> Result<AdmissionProof, OwnerError> {
    let owner = cva
        .owner_uuid()
        .ok_or_else(|| fail("missing import owner"))?;
    let ids: BTreeSet<_> = cva.memory_ids().into_iter().collect();
    let mut birth_episodes = BTreeMap::new();
    let mut completed = BTreeSet::new();
    let mut publications = BTreeMap::<[u8; 32], BTreeSet<[u8; 32]>>::new();
    let scan = cva
        .access
        .snapshot()
        .map_err(|error| OwnerError::Container(error.0))?;
    {
        let mut ingest_frame =
            |frame: crate::owner_lifecycle::HistoryFrame| -> Result<(), OwnerError> {
                let payload = frame.payload.as_slice();
                {
                    if let Some(completion) =
                        crate::insomnia::completion::decode_completion(payload)
                            .map_err(|e| fail(e))?
                    {
                        let episode = cva
                            .episode(completion.episode_id)
                            .ok_or_else(|| fail("import completion lacks Episode"))?;
                        if episode.origin != EpisodeOrigin::Import {
                            return Err(fail("history includes a non-import completion"));
                        }
                        completed.insert(episode.id.0);
                        publications
                            .entry(episode.id.0)
                            .or_default()
                            .insert(Sha256::digest(payload).into());
                        for id in completion.freshness_birth_memory_ids {
                            let record = completion
                                .records
                                .iter()
                                .find(|r| r.id == id && r.revision == 1)
                                .ok_or_else(|| fail("import birth lacks first revision"))?;
                            if cva.memories.first_mutation_id(id)
                                != Some(record.mutation_id.as_str())
                                || record.source_episode_id != Some(episode.id)
                            {
                                return Err(fail("import publication conflicts with Memory birth"));
                            }
                            if birth_episodes
                                .insert(id, episode.id)
                                .is_some_and(|old| old != episode.id)
                            {
                                return Err(fail("conflicting import birth cohort"));
                            }
                        }
                    }
                    Ok(())
                }
            };
        let mut cursor = scan
            .cursor()
            .map_err(|error| OwnerError::Container(error.0))?;
        (|| {
            while let Some(frame) = cursor
                .next_frame()
                .map_err(|error| OwnerError::Container(error.0))?
            {
                ingest_frame(frame)?;
            }
            Ok::<(), OwnerError>(())
        })()
    }?;
    if ids.is_empty() || birth_episodes.keys().any(|id| !ids.contains(id)) {
        return Err(fail("incomplete native import birth cohort"));
    }
    let mut covered = BTreeSet::new();
    let mut episode_keys = BTreeMap::new();
    let mut episode_proofs = BTreeMap::new();
    for episode in cva.episodes() {
        if episode.origin != EpisodeOrigin::Import || !completed.contains(&episode.id.0) {
            return Err(fail("import epoch has incomplete or non-import Episodes"));
        }
        let end = cva
            .archive
            .nodes
            .get(&episode.conversation_id, &episode.end_node_id)
            .ok_or_else(|| fail("missing Episode endpoint"))?;
        if end.timestamp_ns != episode.source_through_ns {
            return Err(fail(
                "Episode source-through timestamp conflicts with endpoint",
            ));
        }
        let turns = cva.episode_turns(episode.id)?;
        if turns
            .first()
            .is_none_or(|t| t.node_id != episode.start_node_id)
            || turns
                .last()
                .is_none_or(|t| t.node_id != episode.end_node_id)
        {
            return Err(fail("Episode path endpoint mismatch"));
        }
        let mut keys = BTreeSet::new();
        let mut path = Vec::new();
        for turn in turns {
            let node = cva
                .archive
                .nodes
                .get(&episode.conversation_id, &turn.node_id)
                .ok_or_else(|| fail("missing Episode path Node"))?;
            if node.content_id != crate::archive_store::hash_content(turn.content.as_bytes()) {
                return Err(fail("Episode retained content binding conflict"));
            }
            path.push(node_binding(node));
            if let Some(position) =
                cva.activity_position_for_turn(&episode.conversation_id, &turn.node_id)
            {
                covered.insert(position);
                keys.insert((episode.conversation_id.clone(), turn.node_id));
            }
        }
        episode_keys.insert(episode.id.0, keys);
        episode_proofs.insert(
            episode.id.0,
            Value::map([
                ("episode_id", Value::bytes(episode.id.0)),
                ("conversation_id", Value::text(&episode.conversation_id)),
                ("start_node_id", Value::text(&episode.start_node_id)),
                ("end_node_id", Value::text(&episode.end_node_id)),
                (
                    "source_through_ns",
                    Value::Signed(episode.source_through_ns),
                ),
                ("path", Value::Array(path)),
                (
                    "publication_payload_digests",
                    Value::Array(
                        publications[&episode.id.0]
                            .iter()
                            .map(Value::bytes)
                            .collect(),
                    ),
                ),
            ]),
        );
    }
    if covered != (1..=cva.rel_turn_count()).collect() {
        return Err(fail("import has unprocessed Archive tail/branch"));
    }
    let mut members = BTreeMap::new();
    let mut cohort_origins = BTreeMap::new();
    let mut cohort_members = BTreeMap::<[u8; 32], Vec<MemoryId>>::new();
    let mut cohort_positions = BTreeMap::<[u8; 32], Option<u64>>::new();
    for id in ids {
        let memory = cva.memory(id)?;
        let first = cva.memory_revision(id, 1)?;
        let grouped = birth_episodes.get(&id).copied();
        if memory.source_episode_id != first.source_episode_id
            || grouped.is_some_and(|episode| memory.source_episode_id != Some(episode))
        {
            return Err(fail("import cohort origin rewritten"));
        }
        let episode = memory
            .source_episode_id
            .map(|episode| {
                cva.episode(episode)
                    .cloned()
                    .ok_or_else(|| fail("missing owner Episode"))
            })
            .transpose()?;
        let (cohort_key, origin) = if let Some(episode) = grouped {
            (
                Value::map([
                    ("owner_uuid", Value::bytes(owner)),
                    ("episode_id", Value::bytes(episode.0)),
                ])
                .digest("reliquary:historical-episode-cohort:v1")?,
                episode_proofs[&episode.0].clone(),
            )
        } else {
            if !cva.freshness_is_publication_birth(id)
                || cva.freshness_publication_cohort(id)? != BTreeSet::from([id])
                || cva.memories.first_mutation_id(id) != Some(first.mutation_id.as_str())
            {
                return Err(fail("unverified direct singleton publication"));
            }
            let key = Value::map([
                ("owner_uuid", Value::bytes(owner)),
                ("memory_id", Value::bytes(id.0)),
            ])
            .digest("reliquary:historical-singleton-cohort:v1")?;
            (
                key,
                Value::map([
                    ("memory_id", Value::bytes(id.0)),
                    ("first_mutation_id", Value::text(&first.mutation_id)),
                    (
                        "first_body_id",
                        Value::bytes(
                            crate::memory_model::memory_body_id(&first.title, &first.content).0,
                        ),
                    ),
                    ("first_revision", Value::Unsigned(first.revision)),
                    ("global_version", Value::Unsigned(first.global_version)),
                    ("publication", Value::text("verified-native-singleton-v1")),
                ]),
            )
        };
        if cohort_origins
            .insert(cohort_key, origin.clone())
            .is_some_and(|old| old != origin)
        {
            return Err(fail("conflicting publication cohort proof"));
        }
        if let Some(source) = &memory.source_ref {
            if Some(&source.owner_id) != cva.owner_id().as_ref()
                || Some(source.source_episode_id) != memory.source_episode_id
                || Some(&source.source_node_id) != memory.source_node_id.as_ref()
                || source.content_source_conversation_id != memory.content_source_conversation_id
                || source.content_source_node_id != memory.content_source_node_id
                || source.grounding_source_conversation_id
                    != memory.grounding_source_conversation_id
                || source.grounding_source_node_id != memory.grounding_source_node_id
            {
                return Err(fail("foreign or conflicting structured import provenance"));
            }
            let node = cva
                .archive
                .nodes
                .get(
                    &episode
                        .as_ref()
                        .ok_or_else(|| fail("structured source lacks Episode"))?
                        .conversation_id,
                    &source.source_node_id,
                )
                .ok_or_else(|| fail("missing structured source"))?;
            if source.principal_id != node.principal_id {
                return Err(fail("structured source principal conflict"));
            }
        }
        let mut necessary = match &episode {
            Some(episode) => episode_keys
                .get(&episode.id.0)
                .cloned()
                .ok_or_else(|| fail("direct publication Episode is outside completed corpus"))?,
            None => BTreeSet::new(),
        };
        if let Some(node) = &memory.source_node_id {
            necessary.insert((
                episode
                    .as_ref()
                    .ok_or_else(|| fail("declared source node lacks Episode namespace"))?
                    .conversation_id
                    .clone(),
                node.clone(),
            ));
        }
        for (conversation, node) in [
            (
                &memory.content_source_conversation_id,
                &memory.content_source_node_id,
            ),
            (
                &memory.grounding_source_conversation_id,
                &memory.grounding_source_node_id,
            ),
        ] {
            match (conversation, node) {
                (Some(c), Some(n)) => {
                    necessary.insert((c.clone(), n.clone()));
                }
                (None, None) => {}
                _ => return Err(fail("partial import provenance pair")),
            }
        }
        let mut latest = None;
        for (conversation, node) in &necessary {
            let position = cva
                .activity_position_for_turn(conversation, node)
                .ok_or_else(|| {
                    fail("declared import source lacks owner-local accepted activity")
                })?;
            latest = Some(latest.unwrap_or(0).max(position));
        }
        let position = cohort_positions.entry(cohort_key).or_insert(Some(0));
        *position = match (*position, latest) {
            (Some(a), Some(b)) => Some(a.max(b)),
            _ => None,
        };
        cohort_members.entry(cohort_key).or_default().push(id);
        members.insert(id, (memory, necessary, cohort_key));
    }
    let mut cohorts = Vec::new();
    for (key, ids) in &cohort_members {
        cohorts.push((
            *key,
            Value::map([
                ("cohort_key", Value::bytes(key)),
                ("origin", cohort_origins[key].clone()),
                (
                    "members",
                    Value::Array(ids.iter().map(|id| Value::bytes(id.0)).collect()),
                ),
                (
                    "availability",
                    cohort_positions[key].map_or(Value::Null, Value::Unsigned),
                ),
            ]),
        ));
    }
    cohorts.sort_by_key(|(key, _)| *key);
    let mut initial = BTreeMap::new();
    let mut memories = Vec::new();
    let mut memory_inputs = BTreeMap::new();
    for (id, (memory, necessary, cohort_key)) in members {
        let available = cohort_positions[&cohort_key];
        let eligible = !memory.archived
            && memory.superseded_by.is_none()
            && matches!(memory.lifecycle_state.as_str(), "knowledge" | "canonical");
        let status = if available.is_none() {
            "unsupported"
        } else if eligible {
            "supported"
        } else {
            "credit-excluded"
        };
        if let Some(at) = available {
            let mut record = FreshnessRecord::created();
            if eligible {
                record.admit(at).map_err(|e| fail(&e.to_string()))?;
            }
            initial.insert(id, record);
        }
        let body = cva.memory_body_id(id)?;
        if crate::memory_model::memory_body_id(&memory.title, &memory.content) != body {
            return Err(fail("historical BodyId content conflict"));
        }
        memory_inputs.insert(
            id,
            HistoricalMemoryInput {
                body_id: body,
                cohort_key,
                availability: available,
                credit_eligible: eligible,
                necessary_sources: necessary
                    .iter()
                    .map(|(c, n)| crate::freshness::historical_planner::SourceKey {
                        conversation_id: c.clone(),
                        node_id: n.clone(),
                    })
                    .collect(),
            },
        );
        let current = cva
            .freshness_record(id)
            .ok_or_else(|| fail("missing provisional scoring record"))?;
        memories.push(Value::map([
            ("memory_id", Value::bytes(id.0)),
            ("body_id", Value::bytes(body.0)),
            ("revision", Value::Unsigned(memory.revision)),
            ("lifecycle", Value::text(&memory.lifecycle_state)),
            ("archived", Value::Bool(memory.archived)),
            (
                "superseded_by",
                memory
                    .superseded_by
                    .map_or(Value::Null, |id| Value::bytes(id.0)),
            ),
            (
                "necessary_sources",
                Value::Array(necessary.iter().map(|(c, n)| source_key(c, n)).collect()),
            ),
            (
                "availability",
                available.map_or(Value::Null, Value::Unsigned),
            ),
            ("cohort_key", Value::bytes(cohort_key)),
            ("status", Value::text(status)),
            (
                "reason",
                Value::text(if available.is_none() {
                    "cohort-missing-usable-provenance"
                } else if eligible {
                    "verified-active-native-publication"
                } else {
                    "final-lifecycle-excluded"
                }),
            ),
            ("mutation_id", Value::text(&memory.mutation_id)),
            (
                "source_episode_id",
                memory
                    .source_episode_id
                    .map_or(Value::Null, |id| Value::bytes(id.0)),
            ),
            (
                "scoring_proof_digest",
                crate::history::community_proof::record_value(current)
                    .digest("reliquary:historical-provisional-record:v1")
                    .map(Value::bytes)?,
            ),
        ]));
    }
    Ok(AdmissionProof {
        memory_inputs,
        initial,
        memories,
        cohorts: cohorts.into_iter().map(|(_, proof)| proof).collect(),
    })
}

#[path = "import_admission_epoch.rs"]
mod epoch;

pub(crate) struct ImportAdmissionReadParts<'a> {
    pub(crate) access: crate::owner_lifecycle::HistoryReadAccess<'a>,
    pub(crate) archive: &'a crate::Archive,
    pub(crate) memories: &'a crate::memory_store::MemoryStore,
    pub(crate) freshness: &'a crate::freshness_storage::FreshnessStore,
    pub(crate) dream: &'a crate::freshness_dream_journal::DreamFreshnessJournal,
}
impl ImportAdmissionReadParts<'_> {
    fn owner_uuid(&self) -> Option<[u8; 16]> {
        self.access.owner_uuid()
    }
    fn owner_id(&self) -> Option<String> {
        self.access.owner_id()
    }
    fn memory_ids(&self) -> Vec<MemoryId> {
        self.memories.current_ids()
    }
    fn memory(&self, id: MemoryId) -> Result<crate::Memory, crate::MemoryError> {
        self.access.current_memory(self.memories, id)
    }
    fn memory_revision(
        &self,
        id: MemoryId,
        revision: u64,
    ) -> Result<crate::Memory, crate::MemoryError> {
        self.access.memory_revision(self.memories, id, revision)
    }
    fn memory_body_id(&self, id: MemoryId) -> Result<crate::MemoryBodyId, crate::MemoryError> {
        self.memories.current_body_id(id)
    }
    fn memory_lifecycle_state(
        &self,
        id: MemoryId,
    ) -> Result<crate::owner_lifecycle::LifecycleState, crate::MemoryError> {
        self.memories.lifecycle_state(id)
    }
    fn memory_retention_descriptor(
        &self,
        id: MemoryId,
    ) -> Result<crate::MemoryRetentionDescriptor, crate::MemoryError> {
        self.memories.retention_descriptor(id)
    }
    fn episode(&self, id: crate::EpisodeId) -> Option<&crate::Episode> {
        self.archive.episode(id)
    }
    fn episodes(&self) -> Vec<crate::Episode> {
        self.archive.episodes()
    }
    fn episode_turns(
        &self,
        id: crate::EpisodeId,
    ) -> Result<Vec<crate::ResolvedTurn>, crate::ArchiveError> {
        self.access.episode_turns(self.archive, id)
    }
    fn rel_turn_count(&self) -> u64 {
        self.archive.rel_turn_count()
    }
    fn activity_position_for_turn(&self, c: &str, n: &str) -> Option<u64> {
        self.archive.activity_position_for_turn(c, n)
    }
    fn freshness_record(&self, id: MemoryId) -> Option<FreshnessRecord> {
        self.freshness
            .current_rewrite_record(id, self.rel_turn_count())
    }
    fn freshness_is_publication_birth(&self, id: MemoryId) -> bool {
        self.dream.is_publication_birth(id)
    }
    fn freshness_publication_cohort(&self, id: MemoryId) -> Result<BTreeSet<MemoryId>, OwnerError> {
        let source = self
            .access
            .snapshot()
            .map_err(|e| OwnerError::Container(e.0))?;
        super::publication_birth_cohort(&source, id)
    }
    fn historical_import_base(
        &self,
        scope: &super::ImportEpochScope,
    ) -> Result<BTreeMap<MemoryId, FreshnessRecord>, OwnerError> {
        let source = self
            .access
            .snapshot()
            .map_err(|e| OwnerError::Container(e.0))?;
        let external = crate::owner_lifecycle::import_birth_facts::external_birth_facts(
            &source,
            scope.start_offset,
        );
        super::historical_freshness_base(
            &source,
            self.memories,
            super::HistoricalFreshnessBaseCut {
                owner_uuid: scope.start.owner_uuid,
                prefix_end: scope.start_offset,
                activity_cut: scope.base_cut,
                global_cut: scope.start.pre.global_version,
            },
            external,
        )
        .map_err(|error| match error {
            super::HistoricalFreshnessBaseError::Freshness(message) => {
                OwnerError::Freshness(message)
            }
            super::HistoricalFreshnessBaseError::Memory(error) => error.into(),
            super::HistoricalFreshnessBaseError::Source(error) => OwnerError::Container(error.0),
        })
    }
}
fn fail(message: &str) -> OwnerError {
    OwnerError::Freshness(message.into())
}
