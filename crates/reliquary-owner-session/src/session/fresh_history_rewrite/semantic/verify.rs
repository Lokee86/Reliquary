use super::*;

impl SemanticClosure {
    pub(in crate::session::fresh_history_rewrite) fn verify_reopened(
        &self,
        source: &CommonState,
        reopened: &CommonState,
        source_owner: [u8; 16],
        destination_owner: [u8; 16],
    ) -> Result<(), String> {
        let actual_entities = reopened
            .perception
            .entities()
            .map(|entity| entity.id)
            .chain(reopened.perception.historical_entities())
            .collect::<BTreeSet<_>>();
        if actual_entities != self.entity_ids {
            return Err("independent rewrite reopen changed selected Entity closure".into());
        }
        for id in &self.entity_ids {
            let revisions = entity_revision_count(&source.perception, *id)?;
            for revision in 1..=revisions {
                if source.perception.entity_revision(*id, revision)
                    != reopened.perception.entity_revision(*id, revision)
                {
                    return Err("independent rewrite reopen changed Entity history".into());
                }
            }
            let expected = source.perception.entity_tombstone(*id);
            let actual = reopened.perception.entity_tombstone(*id);
            if expected.map(|value| (value.id, value.revision))
                != actual.map(|value| (value.id, value.revision))
            {
                return Err("independent rewrite reopen changed Entity tombstone".into());
            }
        }

        let actual_relationships = reopened
            .perception
            .relationships()
            .map(|relationship| relationship.id)
            .chain(reopened.perception.historical_relationships())
            .collect::<BTreeSet<_>>();
        if actual_relationships != self.relationship_ids {
            return Err("independent rewrite reopen changed selected Relationship closure".into());
        }
        for id in &self.relationship_ids {
            let revisions = relationship_revision_count(&source.perception, *id)?;
            for revision in 1..=revisions {
                let expected = source
                    .perception
                    .relationship_revision(*id, revision)
                    .ok_or("selected Relationship history is incomplete")?;
                let actual = reopened
                    .perception
                    .relationship_revision(*id, revision)
                    .ok_or("rewritten Relationship history is incomplete")?;
                if actual.id != expected.id
                    || actual.revision != expected.revision
                    || relationship_draft(actual, destination_owner, destination_owner)
                        != relationship_draft(expected, source_owner, destination_owner)
                {
                    return Err("independent rewrite reopen changed Relationship history".into());
                }
            }
            let expected = source.perception.relationship_tombstone(*id);
            let actual = reopened.perception.relationship_tombstone(*id);
            if expected.map(|value| (value.id, value.revision))
                != actual.map(|value| (value.id, value.revision))
            {
                return Err("independent rewrite reopen changed Relationship tombstone".into());
            }
        }

        for current in &self.resolutions {
            for revision in 1..=current.revision {
                if source.perception.mention_revision(current.key, revision)
                    != reopened.perception.mention_revision(current.key, revision)
                {
                    return Err("independent rewrite reopen changed mention history".into());
                }
            }
        }

        let relation_key = |relation: &Relation| {
            (
                relation.source,
                relation.target,
                relation.kind,
                relation.active,
                relation.origin,
            )
        };
        let expected = self
            .relations
            .iter()
            .map(relation_key)
            .collect::<BTreeSet<_>>();
        let actual = reopened
            .graph
            .active_relations()
            .map(relation_key)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err("independent rewrite reopen changed selected Graph closure".into());
        }
        Ok(())
    }
}
