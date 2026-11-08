use super::*;
use super::{entity, memory, mention, relationship};

impl RelWriter {
    pub fn withdraw_entity_mention(
        &mut self,
        request: WithdrawEntityMentionRequest,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL writer requires reopen before mention withdrawal".into(),
            ));
        }
        let result = mention::withdraw(&mut self.session.common, &mut self.container, request);
        finish(
            &mut self.session.writer_lease_proof,
            &mut self.session.write_poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn retire_memory(
        &mut self,
        request: MemoryRetirementRequest,
    ) -> Result<TerminalTombstone, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL writer requires reopen before Memory retirement".into(),
            ));
        }
        let result = memory::retire(&mut self.session.common, &mut self.container, request);
        finish(
            &mut self.session.writer_lease_proof,
            &mut self.session.write_poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn retire_entity(
        &mut self,
        request: EntityRetirementRequest,
    ) -> Result<EntityTombstone, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL writer requires reopen before Entity retirement".into(),
            ));
        }
        let result = entity::retire(&mut self.session.common, &mut self.container, request);
        finish(
            &mut self.session.writer_lease_proof,
            &mut self.session.write_poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn create_relationship(
        &mut self,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, OwnerSessionError> {
        self.ensure_write_ready()?;
        self.create_relationship_after_fence(request)
    }

    pub fn create_native_import_relationship(
        &mut self,
        import_id: &str,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, OwnerSessionError> {
        self.validate_native_import_memory(import_id, request.source_memory_id)?;
        self.create_relationship_after_fence(request)
    }

    fn create_relationship_after_fence(
        &mut self,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, OwnerSessionError> {
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL writer requires reopen before Relationship publication".into(),
            ));
        }
        let result = relationship::create(&mut self.session.common, &mut self.container, request);
        finish(
            &mut self.session.writer_lease_proof,
            &mut self.session.write_poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn retire_relationship(
        &mut self,
        request: RelationshipRetirementRequest,
    ) -> Result<RelationshipTombstone, OwnerSessionError> {
        self.ensure_write_ready()?;
        let before = self.container.writer_lease_proof();
        if before.is_none() || before != self.session.writer_lease_proof {
            return Err(OwnerSessionError::Record(
                "REL writer requires reopen before Relationship retirement".into(),
            ));
        }
        let result = relationship::retire(&mut self.session.common, &mut self.container, request);
        finish(
            &mut self.session.writer_lease_proof,
            &mut self.session.write_poisoned,
            before,
            &self.container,
            result,
        )
    }
}

impl PhyWriter {
    fn check_lifecycle_lease(
        &self,
        label: &str,
    ) -> Result<Option<WriterLeaseProof>, OwnerSessionError> {
        let before = self.container.writer_lease_proof();
        if self.poisoned || before.is_none() || before != self.writer_lease_proof {
            return Err(OwnerSessionError::Record(format!(
                "PHY writer requires reopen before {label}"
            )));
        }
        Ok(before)
    }

    pub fn withdraw_entity_mention(
        &mut self,
        request: WithdrawEntityMentionRequest,
    ) -> Result<MemoryEntityResolution, OwnerSessionError> {
        let before = self.check_lifecycle_lease("mention withdrawal")?;
        let result = mention::withdraw(&mut self.common, &mut self.container, request);
        finish(
            &mut self.writer_lease_proof,
            &mut self.poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn retire_memory(
        &mut self,
        request: MemoryRetirementRequest,
    ) -> Result<TerminalTombstone, OwnerSessionError> {
        let before = self.check_lifecycle_lease("Memory retirement")?;
        let result = memory::retire(&mut self.common, &mut self.container, request);
        finish(
            &mut self.writer_lease_proof,
            &mut self.poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn retire_entity(
        &mut self,
        request: EntityRetirementRequest,
    ) -> Result<EntityTombstone, OwnerSessionError> {
        let before = self.check_lifecycle_lease("Entity retirement")?;
        let result = entity::retire(&mut self.common, &mut self.container, request);
        finish(
            &mut self.writer_lease_proof,
            &mut self.poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn create_relationship(
        &mut self,
        request: CreateRelationshipRequest,
    ) -> Result<Relationship, OwnerSessionError> {
        let before = self.check_lifecycle_lease("Relationship publication")?;
        let result = relationship::create(&mut self.common, &mut self.container, request);
        finish(
            &mut self.writer_lease_proof,
            &mut self.poisoned,
            before,
            &self.container,
            result,
        )
    }

    pub fn retire_relationship(
        &mut self,
        request: RelationshipRetirementRequest,
    ) -> Result<RelationshipTombstone, OwnerSessionError> {
        let before = self.check_lifecycle_lease("Relationship retirement")?;
        let result = relationship::retire(&mut self.common, &mut self.container, request);
        finish(
            &mut self.writer_lease_proof,
            &mut self.poisoned,
            before,
            &self.container,
            result,
        )
    }
}

fn finish<T>(
    lease: &mut Option<WriterLeaseProof>,
    poisoned: &mut bool,
    before: Option<WriterLeaseProof>,
    container: &Container,
    result: Result<T, OwnerSessionError>,
) -> Result<T, OwnerSessionError> {
    let after = container.writer_lease_proof();
    if result.is_ok() {
        *lease = after;
    } else if before != after
        || matches!(
            &result,
            Err(OwnerSessionError::Memory(MemoryError::Container(_)))
                | Err(OwnerSessionError::Perception(PerceptionError::Container(_)))
        )
    {
        *poisoned = true;
    }
    result
}
