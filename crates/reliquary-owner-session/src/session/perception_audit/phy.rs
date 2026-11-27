use super::*;

fn native_cut(cut: &PerceptionSourceCut) -> PhyAuditCut {
    PhyAuditCut {
        owner_id: cut.owner_id,
        history_generation: cut.history_generation,
        memory: cut.memory,
        graph: cut.graph,
        settlements: cut.settlements,
    }
}

macro_rules! phy_perception_reads {
    ($session:ty) => {
        impl $session {
            fn check_phy_perception_cut(
                &self,
                cut: &PerceptionSourceCut,
            ) -> Result<(), OwnerSessionError> {
                if !native_cut(cut).is_at_or_before(self.phy_audit_cut())
                    || cut.entity > self.common.perception.version()
                    || cut.observation > self.observation_publication_version()
                    || cut.source_removal != 0
                    || cut.freshness != 0
                    || cut.rel_turn != 0
                    || cut.freshness_event.is_some()
                {
                    return Err(OwnerSessionError::ManagementStale);
                }
                Ok(())
            }

            pub fn perception_source_metadata(
                &self,
                cut: &PerceptionSourceCut,
                ids: &[MemoryId],
            ) -> Result<Vec<PerceptionMemoryMetadata>, OwnerSessionError> {
                self.check_phy_perception_cut(cut)?;
                if ids.len() > MAX_CAPTURE_PAGE {
                    return Err(OwnerSessionError::ManagementLimit);
                }
                let read = self.phy_audit_read(native_cut(cut))?;
                Ok(ids
                    .iter()
                    .filter_map(|id| {
                        let record = read.current(*id)?;
                        Some(PerceptionMemoryMetadata {
                            source: PerceptionMemorySource {
                                id: record.id,
                                revision: record.revision,
                                body_id: record.body_id,
                            },
                            // Shared evidence callers consume the source fields.
                            // PHY never evaluates or persists a Freshness grade.
                            grade: PerceptionSourceGrade::Fresh,
                        })
                    })
                    .collect())
            }

            pub fn perception_memory_evidence(
                &self,
                cut: &PerceptionSourceCut,
                ids: &[MemoryId],
            ) -> Result<Vec<PerceptionMemoryEvidence>, OwnerSessionError> {
                self.perception_source_metadata(cut, ids)?
                    .into_iter()
                    .map(|metadata| {
                        let (record, body) = self.common.memories.revision_projection(
                            metadata.source.id,
                            Some(metadata.source.revision),
                        )?;
                        let MemoryBody { title, content } = body
                            .ok_or(OwnerSessionError::ManagementEvidence)?
                            .into_owned();
                        Ok(PerceptionMemoryEvidence {
                            metadata,
                            title,
                            content,
                            source_lineage: perception_lineage(record),
                        })
                    })
                    .collect()
            }

            pub fn perception_participant_evidence(
                &self,
                cut: &PerceptionSourceCut,
                source: PerceptionMemorySource,
            ) -> Result<Vec<PerceptionParticipantEvidence>, OwnerSessionError> {
                self.check_phy_perception_cut(cut)?;
                let read = self.phy_audit_read(native_cut(cut))?;
                if !read.current(source.id).is_some_and(|record| {
                    record.revision == source.revision && record.body_id == source.body_id
                }) {
                    return Ok(Vec::new());
                }
                let (_, body) = self
                    .common
                    .memories
                    .revision_projection(source.id, Some(source.revision))?;
                let body = body.ok_or(OwnerSessionError::ManagementEvidence)?;
                let perception = &self.common.perception;
                let mut seen = BTreeSet::new();
                Ok(perception
                    .resolved_mentions_in_memory(source.id.0)
                    .filter_map(|(id, mention)| {
                        let entity = perception.entity_at_cut(id, cut.entity)?;
                        if perception.entity(id)?.revision != entity.revision
                            || entity.kind == PRINCIPAL_ENTITY_KIND
                            || entity.kind.eq_ignore_ascii_case("generic")
                            || seen.contains(&id)
                            || (mention.retained_ordinal.is_none()
                                && perception.mention_at_cut(mention.key, cut.entity)?.revision
                                    != mention.revision)
                        {
                            return None;
                        }
                        let text = text_field(&body, mention.key.field);
                        let surface = text
                            .get(mention.key.start_byte as usize..mention.key.end_byte as usize)?;
                        if !super::super::identity_evidence::quote_names_entity(
                            surface,
                            &entity.canonical_name,
                            &entity.aliases,
                        ) {
                            return None;
                        }
                        seen.insert(id);
                        Some(PerceptionParticipantEvidence {
                            entity_id: id,
                            entity_revision: entity.revision,
                            canonical_name: entity.canonical_name.clone(),
                            mention: mention.key,
                            mention_revision: mention.revision,
                            retained_ordinal: mention.retained_ordinal,
                        })
                    })
                    .take(MAX_PARTICIPANTS)
                    .collect())
            }

            pub fn perception_lexical_candidates(
                &self,
                query: &str,
                limit: usize,
            ) -> Vec<MemoryId> {
                self.phy_audit_lexical_candidates(self.phy_audit_cut(), query, limit)
                    .unwrap_or_default()
            }

            pub fn perception_graph_links(
                &self,
                cut: &PerceptionSourceCut,
                source: MemoryId,
            ) -> Result<Vec<(MemoryId, Relation)>, OwnerSessionError> {
                self.check_phy_perception_cut(cut)?;
                self.phy_audit_graph_links(native_cut(cut), source)
            }

            pub fn perception_graph_neighbors(&self, source: MemoryId) -> Vec<MemoryId> {
                self.phy_audit_graph_links(self.phy_audit_cut(), source)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect()
            }

            pub fn perception_relationship_sources(&self, entity: EntityId) -> Vec<MemoryId> {
                relationship_sources(&self.common.perception, self.owner_id, entity, |_| true)
            }

            pub fn has_equivalent_manual_relationship(&self, draft: &ObservationDraft) -> bool {
                equivalent_manual_relationship(
                    &self.common.perception,
                    self.owner_id,
                    draft,
                    |_| true,
                )
            }

            /// Page bounded graph, mention, and derived-source changes independently
            /// from the Memory revisions which determine routine-audit cadence.
            pub fn phy_perception_changes_page(
                &self,
                previous: &PerceptionSourceCut,
                through: &PerceptionSourceCut,
                limit: usize,
            ) -> Result<PerceptionSourceChanges, OwnerSessionError> {
                self.check_phy_perception_cut(previous)?;
                self.check_phy_perception_cut(through)?;
                if !native_cut(previous).is_at_or_before(native_cut(through))
                    || previous.entity > through.entity
                    || previous.observation > through.observation
                {
                    return Err(OwnerSessionError::ManagementStale);
                }
                let common = &self.common;
                let limit = limit.clamp(1, MAX_CAPTURE_PAGE);
                let mut memories = BTreeSet::new();
                let mut graph_through = through.graph;
                let mut last_graph_version = previous.graph;
                for (index, edge) in common
                    .graph
                    .relation_changes_after(previous.graph, through.graph, usize::MAX)
                    .enumerate()
                {
                    // A clocked Graph publication is atomic. Include every
                    // endpoint at its version before advancing the cursor.
                    if index >= limit && edge.graph_version != last_graph_version {
                        graph_through = last_graph_version;
                        break;
                    }
                    last_graph_version = edge.graph_version;
                    for endpoint in [edge.source, edge.target] {
                        if let NodeRef::Memory(id) = endpoint {
                            memories.insert(MemoryId(id));
                        }
                    }
                }
                let entity = common
                    .perception
                    .evidence_changes_after(previous.entity, through.entity, limit + 1)
                    .collect::<Vec<_>>();
                for &(_, change) in entity.iter().take(limit) {
                    match change {
                        EntityEvidenceChange::Mention(key) => {
                            memories.insert(MemoryId(key.memory_id));
                        }
                        EntityEvidenceChange::Entity(id) => {
                            memories.extend(
                                common
                                    .perception
                                    .resolved_mentions_for_entity(id)
                                    .take(MAX_NEIGHBORS)
                                    .map(|mention| MemoryId(mention.key.memory_id)),
                            );
                        }
                    }
                }
                let records = common.phy_observations.store.records();
                let start = records.partition_point(|(receipt, _)| {
                    receipt.publication_version <= previous.observation
                });
                let end = records.partition_point(|(receipt, _)| {
                    receipt.publication_version <= through.observation
                });
                let mut derived = BTreeSet::new();
                let mut observation_through = through.observation;
                for (index, (receipt, _)) in records[start..end].iter().enumerate() {
                    if index == limit {
                        observation_through = receipt.publication_version - 1;
                        break;
                    }
                    // Include every current revision published by this native batch
                    // before moving the cursor across its indivisible record.
                    for &(id, revision) in &receipt.revisions {
                        if self.observation(id).is_some_and(|row| {
                            row.standing == ObservationStanding::Current
                                && row.revision.revision == revision
                        }) {
                            derived.insert(id);
                        }
                    }
                }
                Ok(PerceptionSourceChanges {
                    memories: memories.into_iter().collect(),
                    derived: derived.into_iter().collect(),
                    ego_memories: Vec::new(),
                    memory_through: through.memory,
                    graph_through,
                    entity_through: if entity.len() > limit {
                        entity[limit - 1].0
                    } else {
                        through.entity
                    },
                    observation_through,
                    freshness_through: 0,
                })
            }
        }
    };
}

phy_perception_reads!(PhySession);
phy_perception_reads!(PhyWriter);
